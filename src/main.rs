mod config;
mod error;
mod pkce;
mod providers;

use std::path::Path;

use clap::{Parser, Subcommand};
use config::ProviderType;
use base64::Engine;

#[derive(Parser)]
#[command(
    name = "vigen",
    about = "Vision + Gen — AI visual assistant",
    long_about = "A lightweight helper that gives text-only models access to vision AI (and later, image generation)."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze an image with a vision model
    See {
        /// Path to the image file (png, jpg, webp, gif, bmp)
        #[arg(short, long)]
        image: String,

        /// Text prompt for the vision model
        #[arg(short, long, default_value = "Describe this image in detail")]
        prompt: String,
    },

    /// Generate an image from a text prompt
    Gen {
        /// Text prompt describing the image to generate
        #[arg(short, long)]
        prompt: String,

        /// Image size (1024x1024, 1024x1536, 1536x1024)
        #[arg(long, default_value = "1024x1024")]
        size: String,

        /// Number of images to generate (1-4)
        #[arg(long, default_value = "1")]
        n: u8,

        /// Directory to save generated images
        #[arg(short, long, default_value = ".")]
        output: String,
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
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), anyhow::Error> {
    match cli.command {
        Commands::See {
            image,
            prompt,
        } => {
            let cfg = config::VigenConfig::load()?;
            let image_data = std::fs::read(&image)
                .map_err(|e| anyhow::anyhow!("cannot read image '{image}': {e}"))?;
            let mime = detect_mime(&image)?;
            let result = providers::analyze_image(&cfg, &image_data, mime, &prompt).await?;
            println!("{result}");
        }
        Commands::Gen {
            prompt,
            size,
            n,
            output,
        } => {
            let cfg = config::VigenConfig::load()?;
            let images = providers::generate_image(&cfg, &prompt, &size, n).await?;
            for (i, b64) in images.iter().enumerate() {
                let data = base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .map_err(|e| anyhow::anyhow!("invalid base64 in image response: {e}"))?;
                let filename = if images.len() > 1 {
                    format!("vigen_gen_{:02}.png", i + 1)
                } else {
                    "vigen_gen.png".to_string()
                };
                let path = std::path::Path::new(&output).join(&filename);
                std::fs::write(&path, &data)
                    .map_err(|e| anyhow::anyhow!("cannot write {}: {e}", path.display()))?;
                println!("{}", path.display());
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
