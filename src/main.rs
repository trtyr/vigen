mod auth;
mod config;
mod error;
mod providers;

use std::path::Path;

use clap::{Parser, Subcommand};
use providers::VisionProvider;

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

    /// Manage vigen configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Login to Google via browser OAuth
    Login,
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Print current configuration
    Show,

    /// Print the path to the config file
    Path,

    /// Write a fresh config template to disk
    Init,

    /// Set Google API key
    SetKey {
        api_key: String,
    },

    /// Set global proxy URL (e.g. http://127.0.0.1:7890)
    SetProxy {
        url: String,
    },

    /// Set Google model
    SetModel {
        model: String,
    },

    /// Set GCP project ID (for OAuth mode)
    SetProject {
        project_id: String,
    },

    /// List available Gemini models
    ListModels,
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
        Commands::See { image, prompt } => cmd_see(&image, &prompt).await?,
        Commands::Config { action } => cmd_config(action).await?,
        Commands::Login => cmd_login().await?,
    }
    Ok(())
}

async fn cmd_see(image_path: &str, prompt: &str) -> Result<(), anyhow::Error> {
    let cfg = config::VigenConfig::load()?;

    let image_data = std::fs::read(image_path)
        .map_err(|e| anyhow::anyhow!("cannot read image '{}': {e}", image_path))?;

    let mime = detect_mime(image_path)?;

            let provider = providers::google::GoogleProvider::from_config(&cfg)?;

    let result = provider.analyze_image(&image_data, mime, prompt).await?;

    println!("{result}");
    Ok(())
}

async fn cmd_config(action: ConfigAction) -> Result<(), anyhow::Error> {
    let path = config::VigenConfig::config_path();

    match action {
        ConfigAction::Show => {
            let cfg = config::VigenConfig::load()?;
            let text = toml::to_string_pretty(&cfg)?;
            println!("{text}");
        }
        ConfigAction::Path => {
            println!("{}", path.display());
        }
        ConfigAction::Init => {
            if path.exists() {
                anyhow::bail!("config already exists at {}", path.display());
            }
            config::VigenConfig::default().save()?;
            println!("created {}", path.display());
        }
        ConfigAction::SetKey { api_key } => {
            let mut cfg = config::VigenConfig::load()?;
            let google = cfg.providers.google.get_or_insert_with(|| config::GoogleConfig {
                api_key: None,
                model: "gemini-2.0-flash".into(),
                proxy: None,
                project: None,
            });
            google.api_key = Some(api_key);
            cfg.save()?;
            println!("api key updated");
        }
        ConfigAction::SetProxy { url } => {
            let mut cfg = config::VigenConfig::load()?;
            cfg.proxy = Some(config::ProxyConfig { url });
            cfg.save()?;
            println!("proxy updated");
        }
        ConfigAction::SetModel { model } => {
            let mut cfg = config::VigenConfig::load()?;
            let google = cfg.providers.google.get_or_insert_with(|| config::GoogleConfig {
                api_key: None,
                model: String::new(),
                proxy: None,
                project: None,
            });
            google.model = model;
            cfg.save()?;
            println!("model updated");
        }
        ConfigAction::SetProject { project_id } => {
            let mut cfg = config::VigenConfig::load()?;
            let google = cfg.providers.google.get_or_insert_with(|| config::GoogleConfig {
                api_key: None,
                model: String::new(),
                proxy: None,
                project: None,
            });
            google.project = Some(project_id);
            cfg.save()?;
            println!("project updated");
        }
        ConfigAction::ListModels => {
            let cfg = config::VigenConfig::load()?;
    let provider = providers::google::GoogleProvider::from_config(&cfg)?;
            let models = provider.list_models().await?;
            for m in models {
                let name = m.name.strip_prefix("models/").unwrap_or(&m.name);
                let label = m
                    .display_name
                    .as_deref()
                    .unwrap_or(name);
                let vision = m
                    .supported_generation_methods
                    .contains(&"generateContent".to_string());
                let token_info = match (m.input_token_limit, m.output_token_limit) {
                    (Some(i), Some(o)) => format!(" in:{i} out:{o}"),
                    _ => String::new(),
                };
                let marker = if vision { " [vision]" } else { "" };
                println!("{name}");
                println!("  {label}{marker}{token_info}");
                if let Some(ref desc) = m.description {
                    println!("  {desc}");
                }
                println!();
            }
        }
    }
    Ok(())
}

async fn cmd_login() -> Result<(), anyhow::Error> {
    let mut cfg = config::VigenConfig::load()?;

    let proxy = cfg.proxy.as_ref().map(|p| p.url.as_str());

    let result = auth::google_login(proxy).await?;

    let auth_cfg = cfg.auth.get_or_insert_with(config::AuthConfig::default);
    auth_cfg.google = Some(result);
    cfg.save()?;

    println!("Login successful! Tokens saved.");
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
        "gif" => Ok("image/gif"),
        "webp" => Ok("image/webp"),
        "bmp" => Ok("image/bmp"),
        other => anyhow::bail!("unsupported image format: .{other}"),
    }
}
