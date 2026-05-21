# vigen

> Vision + Gen — CLI tool for text-only models to see and create images.

A lightweight command-line tool: `vigen see` analyzes images with Google Gemini, `vigen gen` creates images with OpenAI DALL·E. Designed for AI agents and power users who need vision capabilities from the terminal.

## Features

- **Image analysis** — Describe, OCR, identify objects via Google Gemini
- **Image generation** — Create images from text prompts via OpenAI DALL·E
- **Reference-guided generation** — Use an existing image as style reference
- **Multi-endpoint fallback** — Configure backup providers, auto-switch on failure
- **URL image extraction** — Handles providers that return image URLs instead of base64
- **Proxy support** — Global or per-provider HTTP/SOCKS5 proxy
- **Multiple auth modes** — API key or browser-based OAuth
- **Format conversion** — Output as PNG, JPEG, or WebP

## Install

### From crates.io

```bash
cargo install vigen
```

### From source

```bash
git clone https://github.com/trtyr/vigen.git
cd vigen
cargo install --path .
```

Requires Rust 1.80+.

## Quick Start

```bash
# Set up API keys
vigen auth key google <your-gemini-api-key>
vigen auth key gpt <your-openai-api-key>

# Analyze an image
vigen see -i photo.jpg -p "What's in this image?"

# Generate an image
vigen gen -p "a cat wearing a spacesuit" --size 1024x1024 -o ./output

# Generate with style reference
vigen gen -p "cyberpunk cityscape" -r style_reference.png

# Pipe image via stdin
cat photo.png | vigen see -p "Describe this image"
```

## Commands

```
vigen see -i <path> [-p <prompt>] [-v]              Analyze image (Google Gemini)
vigen gen -p <prompt> [--size] [-n] [-o] [-r] [-v]  Generate image (OpenAI DALL·E)
vigen auth key <google|gpt> <key>                    Set API key
vigen auth login --provider <google|gpt>             Browser OAuth login
vigen auth login --provider gpt --device-auth        Device flow auth
vigen auth login --provider gpt --with-api-key       Interactive API key entry
vigen model <google|gpt> <model>                     Switch model
vigen models [--provider <google|gpt>]               List available models
vigen proxy <url>                                    Set proxy
vigen project <project_id>                           Set GCP project ID (Google OAuth)
vigen config show | path | init                      Manage config
```

## Configuration

Config file location: `$XDG_CONFIG_HOME/vigen/config.toml` (defaults to `~/.config/vigen/config.toml`).

```toml
[proxy]
url = "http://127.0.0.1:7890"

[providers.google]
api_keys = ["AIza..."]                          # Multiple keys for rotation
model = "gemini-2.0-flash"
fallback_model = "gemini-1.5-flash"             # Fallback model on same endpoint

[providers.gpt]
api_key = "sk-..."
model = "gpt-image-2"
base_url = "https://api.openai.com"             # Custom base URL for compatible APIs
image_endpoint = "/v1/images/generations"       # Also supports /v1/chat/completions
fallback_model = "dall-e-2"                     # Fallback model on same endpoint

# Fallback endpoints — tried in order when primary fails
[[providers.gpt.fallbacks]]
api_key = "sk-backup-..."
base_url = "https://backup-provider.com"
image_endpoint = "/v1/chat/completions"
model = "custom-model-id"

[[providers.gpt.fallbacks]]
api_key = "sk-another-..."
base_url = "https://another-provider.com"
```

### Fallback Behavior

Two levels of fallback:

1. **Within-endpoint**: primary model → `fallback_model` (same API endpoint)
2. **Across-endpoints**: primary config → `[[providers.gpt.fallbacks]]` list (different API endpoints)

Non-fatal errors (timeouts, 5xx, rate limits) continue to the next fallback. Fatal errors (401, 403) stop immediately.

Each fallback endpoint can optionally override `api_key`, `base_url`, `image_endpoint`, and `model`. Fields omitted in a fallback inherit from the primary `[providers.gpt]` config.

### Proxy

Set globally or per-provider:

```toml
[proxy]
url = "http://127.0.0.1:7890"    # Global proxy

[providers.gpt]
proxy = "socks5://127.0.0.1:1080"  # Override for this provider
```

Supports HTTP and SOCKS5 proxies.

## Gen Command

```bash
# Basic generation
vigen gen -p "A golden retriever in a sunflower field"

# Sizes: 1024x1024 (square), 1024x1536 (portrait), 1536x1024 (landscape)
vigen gen -p "Mountain landscape" --size 1536x1024

# Multiple images
vigen gen -p "Abstract ocean waves" --n 3

# Output format: png (default), jpg, webp
vigen gen -p "Sunset photo" --format jpg

# Style reference: analyze image with Gemini, merge into prompt
vigen gen -p "Cyberpunk city" -r reference.png

# Save to specific directory
vigen gen -p "Watercolor forest" -o ~/Pictures/
```

## See Command

```bash
# Default: describe image in detail
vigen see -i photo.jpg

# OCR / text extraction
vigen see -i screenshot.png -p "Extract all error messages"

# Diagram analysis
vigen see -i flowchart.png -p "Explain this flowchart step by step"

# Object identification
vigen see -i room.jpg -p "List every object you can identify"

# Via stdin
cat image.png | vigen see -p "What is this?"
```

## Skills

Vigen ships with skill files that teach AI coding assistants how to use `vigen see` and `vigen gen`. These are plain Markdown files — any tool that supports skill/knowledge files can use them.

### OpenCode

Copy the skill directories to your OpenCode skills directory:

```bash
cp -r skills/vigen-gen ~/.config/opencode/skills/
cp -r skills/vigen-see ~/.config/opencode/skills/
```

### Other Tools

The `skills/` directory contains two self-contained Markdown files:

- `skills/vigen-gen/SKILL.md` — Image generation instructions
- `skills/vigen-see/SKILL.md` — Image analysis instructions

For any AI tool that accepts knowledge files or custom instructions, simply point it at these files or copy their contents into your tool's configuration. The files contain trigger words, usage examples, and prompt writing tips.

## License

MIT
