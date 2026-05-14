mod config;
mod error;
mod pkce;
mod providers;

use std::io::Read;
use std::io::Write;
use std::path::Path;

use clap::{Parser, Subcommand};
use config::ProviderType;
use base64::Engine;

#[derive(Parser)]
#[command(
    name = "vigen",
    about = "Gemini vision + GPT image generation",
    long_about = "A lightweight helper that gives text-only models access to Google Gemini (vision) and OpenAI GPT (image generation)."
)]
struct Cli {
    /// Show debug output
    #[arg(short, long, global = true, default_value = "false")]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze an image with Google Gemini
    See {
        /// Path to the image file (reads from stdin if not provided)
        #[arg(short, long)]
        image: Option<String>,

        /// Text prompt for Gemini (default: "Describe this image in detail")
        #[arg(short, long, default_value = "Describe this image in detail")]
        prompt: String,
    },

    /// Generate an image with OpenAI GPT (DALL·E)
    Gen {
        /// Text prompt describing the image to generate
        #[arg(short, long)]
        prompt: String,

        /// Reference image to use as style guide (analyzed via Gemini)
        #[arg(short, long)]
        reference: Option<String>,

        /// Image size (1024x1024, 1024x1536, 1536x1024)
        #[arg(long, default_value = "1024x1024")]
        size: String,

        /// Number of images to generate (1-4)
        #[arg(long, default_value = "1")]
        n: u8,

        /// Directory to save generated images
        #[arg(short, long, default_value = ".")]
        output: String,

        /// Output format (png, jpg, webp)
        #[arg(long, default_value = "png")]
        format: String,

        /// Write raw image bytes to stdout (progress goes to stderr)
        #[arg(long, default_value = "false")]
        stdout: bool,
    },

    /// Manage authentication
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },

    /// Set the default model for a provider
    Model {
        /// Provider name (google, gpt)
        provider: String,

        /// Model name to use (e.g. gemini-2.0-flash, gpt-image-2)
        model: String,
    },

    /// List available models for a provider
    Models {
        /// Provider name (google, gpt)
        #[arg(long, default_value = "google")]
        provider: String,
    },

    /// Set global proxy URL (e.g. http://127.0.0.1:7890)
    Proxy {
        url: String,
    },

    /// Set GCP project ID (google only, for OAuth mode)
    Project {
        project_id: String,
    },

    /// Manage the config file itself
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum AuthAction {
    /// Login to an AI provider
    Login {
        /// Provider to login to (google, codex)
        #[arg(long, default_value = "google")]
        provider: String,
    },

    /// Set API key directly
    Key {
        /// Provider name (google, gpt)
        provider: String,

        /// API key
        api_key: String,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Print current configuration
    Show,

    /// Print the path to the config file
    Path,

    /// Write a fresh config template to disk
    Init,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli).await {
        eprintln!("vigen error: {e}");
        if let Some(hint) = error_hint(&e) {
            eprintln!("  {hint}");
        }
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), anyhow::Error> {
    let verbose = cli.verbose;
    if verbose {
        eprintln!("[vigen] config path: {}", config::VigenConfig::config_path().display());
    }

    match cli.command {
        Commands::See {
            image,
            prompt,
        } => {
            let cfg = config::VigenConfig::load()?;
            if verbose {
                let model = cfg.providers.google.as_ref().map(|c| c.model.as_str()).unwrap_or("default");
                eprintln!("[vigen] see: model={model}");
            }
            let (image_data, mime) = if let Some(ref path) = image {
                let data = std::fs::read(path)
                    .map_err(|e| anyhow::anyhow!("cannot read image '{path}': {e}"))?;
                let mime = detect_mime(path)?;
                (data, mime)
            } else {
                let mut buf = Vec::new();
                std::io::stdin()
                    .read_to_end(&mut buf)
                    .map_err(|e| anyhow::anyhow!("cannot read image from stdin: {e}"))?;
                if buf.is_empty() {
                    anyhow::bail!("no image provided (use -i <path> or pipe an image to stdin)");
                }
                let mime = detect_mime_from_bytes(&buf);
                (buf, mime)
            };
            let result = providers::analyze_image(&cfg, &image_data, mime, &prompt).await?;
            println!("{result}");
        }
        Commands::Gen {
            prompt,
            reference,
            size,
            n,
            output,
            format,
            stdout,
        } => {
            let mut cfg = config::VigenConfig::load()?;
            let model = cfg
                .providers
                .gpt
                .as_ref()
                .map(|c| c.model.clone())
                .unwrap_or_default();

            let final_prompt = if let Some(ref ref_path) = reference {
                let ref_data = std::fs::read(ref_path)
                    .map_err(|e| anyhow::anyhow!("cannot read reference image '{ref_path}': {e}"))?;
                let ref_mime = detect_mime(ref_path)?;
                let has_google = cfg.providers.google.is_some();
                if !has_google {
                    anyhow::bail!(
                        "reference image requires Google Gemini configured.\n  Set up: vigen auth key google <your-gemini-api-key>"
                    );
                }
                if !stdout {
                    eprintln!("Analyzing reference image with Gemini...");
                }
                let analysis = providers::analyze_image(
                    &cfg,
                    &ref_data,
                    ref_mime,
                    "Analyze this reference image and describe its visual style, color palette, lighting, composition, mood, and key elements. Write concisely as a style guide for image generation.",
                ).await?;
                if verbose {
                    eprintln!("[vigen] reference analysis ({ref_path}): {analysis}");
                }
                format!("Style reference: {analysis}\n\nGenerate: {prompt}")
            } else {
                prompt.clone()
            };

            if !stdout {
                eprintln!("Generating with {model}...");
            }
            let start = std::time::Instant::now();
            let images = providers::generate_image(&mut cfg, &final_prompt, &size, n).await?;
            if !stdout {
                let elapsed = start.elapsed();
                let n_images = images.len();
                eprintln!("Done in {:.1}s ({n_images} image{})", elapsed.as_secs_f64(), if n_images > 1 { "s" } else { "" });
            }
            let fmt = format.to_lowercase();
            if fmt != "png" && fmt != "jpg" && fmt != "jpeg" && fmt != "webp" {
                anyhow::bail!("unsupported format '{format}' (use png, jpg, or webp)");
            }
            for (i, b64) in images.iter().enumerate() {
                let mut data = base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .map_err(|e| anyhow::anyhow!("invalid base64 in image response: {e}"))?;
                if fmt == "jpg" || fmt == "jpeg" {
                    data = convert_to_jpeg(&data)?;
                } else if fmt == "webp" {
                    data = convert_to_webp(&data)?;
                }
                if stdout {
                    std::io::stdout()
                        .write_all(&data)
                        .map_err(|e| anyhow::anyhow!("cannot write to stdout: {e}"))?;
                } else {
                    let ext = if fmt == "jpeg" { "jpg" } else { fmt.as_str() };
                    let filename = if images.len() > 1 {
                        format!("vigen_gen_{:02}.{ext}", i + 1)
                    } else {
                        format!("vigen_gen.{ext}")
                    };
                    let path = std::path::Path::new(&output).join(&filename);
                    std::fs::write(&path, &data)
                        .map_err(|e| anyhow::anyhow!("cannot write {}: {e}", path.display()))?;
                    eprintln!("{}", path.display());
                }
            }
        }
        Commands::Auth { action } => match action {
            AuthAction::Login { provider } => {
                let pt = ProviderType::parse(&provider)?;
                let mut cfg = config::VigenConfig::load()?;
                let proxy = cfg.proxy.as_ref().map(|p| p.url.clone());
                providers::login(pt, &mut cfg, proxy.as_deref()).await?;
                println!("Login successful!");
            }
            AuthAction::Key { provider, api_key } => {
                let pt = ProviderType::parse(&provider)?;
                let mut cfg = config::VigenConfig::load()?;
                match pt {
                    ProviderType::Google => {
                        let google = cfg.providers.google.get_or_insert_with(|| {
                            config::GoogleConfig {
                                api_key: None,
                                model: "gemini-2.0-flash".into(),
                                fallback_model: None,
                                proxy: None,
                                project: None,
                            }
                        });
                        google.api_key = Some(api_key);
                    }
                    ProviderType::Gpt => {
                        let gpt = cfg.providers.gpt.get_or_insert_with(|| {
                            config::GptConfig {
                                api_key: None,
                                model: "gpt-image-2".into(),
                                base_url: None,
                                image_endpoint: None,
                                fallback_model: None,
                                proxy: None,
                            }
                        });
                        gpt.api_key = Some(api_key);
                    }
                }
                cfg.save()?;
                println!("{provider} api key updated");
            }
        },
        Commands::Model { provider, model } => {
            let pt = ProviderType::parse(&provider)?;
            let mut cfg = config::VigenConfig::load()?;
            match pt {
                ProviderType::Google => {
                    let google = cfg.providers.google.get_or_insert_with(|| {
                        config::GoogleConfig {
                            api_key: None,
                            model: String::new(),
                            fallback_model: None,
                            proxy: None,
                            project: None,
                        }
                    });
                    google.model = model;
                }
                ProviderType::Gpt => {
                    let gpt = cfg.providers.gpt.get_or_insert_with(|| config::GptConfig {
                        api_key: None,
                        model: String::new(),
                        base_url: None,
                        image_endpoint: None,
                        fallback_model: None,
                        proxy: None,
                    });
                    gpt.model = model;
                }
            }
            cfg.save()?;
            println!("{provider} model updated");
        }
        Commands::Models { provider } => {
            let pt = ProviderType::parse(&provider)?;
            let cfg = config::VigenConfig::load()?;
            let models = providers::list_models(pt, &cfg).await?;
            for (name, display) in &models {
                if let Some(d) = display {
                    println!("{name}  ({d})");
                } else {
                    println!("{name}");
                }
            }
            if models.is_empty() {
                println!("No models found");
            }
        }
        Commands::Proxy { url } => {
            let mut cfg = config::VigenConfig::load()?;
            cfg.proxy = Some(config::ProxyConfig { url });
            cfg.save()?;
            println!("proxy updated");
        }
        Commands::Project { project_id } => {
            let mut cfg = config::VigenConfig::load()?;
            let google = cfg.providers.google.get_or_insert_with(|| {
                config::GoogleConfig {
                    api_key: None,
                    model: String::new(),
                    fallback_model: None,
                    proxy: None,
                    project: None,
                }
            });
            google.project = Some(project_id);
            cfg.save()?;
            println!("project updated");
        }
        Commands::Config { action } => match action {
            ConfigAction::Show => {
                let cfg = config::VigenConfig::load()?;
                let text = toml::to_string_pretty(&cfg)?;
                println!("{text}");
            }
            ConfigAction::Path => {
                println!("{}", config::VigenConfig::config_path().display());
            }
            ConfigAction::Init => {
                let path = config::VigenConfig::config_path();
                if path.exists() {
                    anyhow::bail!("config already exists at {}", path.display());
                }
                config::VigenConfig::default().save()?;
                println!("created {}", path.display());
            }
        },
    }
    Ok(())
}

fn detect_mime(path: &str) -> Result<&'static str, anyhow::Error> {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    match ext.to_lowercase().as_str() {
        "png" => Ok("image/png"),
        "jpg" | "jpeg" => Ok("image/jpeg"),
        "webp" => Ok("image/webp"),
        "gif" => Ok("image/gif"),
        "bmp" => Ok("image/bmp"),
        _ => anyhow::bail!("unsupported image format: .{ext} (png, jpg, webp, gif, bmp)"),
    }
}

fn detect_mime_from_bytes(data: &[u8]) -> &'static str {
    if data.starts_with(b"\x89PNG") {
        "image/png"
    } else if data.starts_with(b"\xff\xd8\xff") {
        "image/jpeg"
    } else if data.len() >= 12 && &data[8..12] == b"WEBP" {
        "image/webp"
    } else if data.starts_with(b"GIF8") {
        "image/gif"
    } else if data.starts_with(b"BM") {
        "image/bmp"
    } else {
        "image/png"
    }
}

fn convert_to_jpeg(png_bytes: &[u8]) -> Result<Vec<u8>, anyhow::Error> {
    let img = image::load_from_memory(png_bytes)
        .map_err(|e| anyhow::anyhow!("cannot decode image for jpeg conversion: {e}"))?;
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Jpeg)
        .map_err(|e| anyhow::anyhow!("cannot encode jpeg: {e}"))?;
    Ok(buf.into_inner())
}

fn convert_to_webp(png_bytes: &[u8]) -> Result<Vec<u8>, anyhow::Error> {
    let img = image::load_from_memory(png_bytes)
        .map_err(|e| anyhow::anyhow!("cannot decode image for webp conversion: {e}"))?;
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::WebP)
        .map_err(|e| anyhow::anyhow!("cannot encode webp: {e}"))?;
    Ok(buf.into_inner())
}

fn error_hint(err: &anyhow::Error) -> Option<String> {
    let msg = err.to_string();
    if msg.contains("not configured") {
        if msg.contains("google") {
            Some("→ Set up: vigen auth key google <your-gemini-api-key>".into())
        } else if msg.contains("gpt") {
            Some("→ Set up: vigen auth key gpt <your-openai-api-key>".into())
        } else {
            None
        }
    } else if msg.contains("status 401") || msg.contains("status 403") {
        Some("→ Authentication failed. Re-run: vigen auth key <provider> <key>".into())
    } else if msg.contains("status 429") {
        Some("→ Rate limited. Wait and retry, or set a fallback model with: vigen model <provider> <fallback>".into())
    } else if msg.contains("cannot read") || msg.contains("no such file") {
        Some("→ Check the file path. Pipe an image via stdin: cat image.png | vigen see".into())
    } else {
        None
    }
}
