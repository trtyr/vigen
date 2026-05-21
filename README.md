<div align="center">

# vigen

**Vision + Gen — Give your AI agent eyes and a paintbrush.**

[![crates.io](https://img.shields.io/crates/v/vigen.svg)](https://crates.io/crates/vigen)
[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](https://www.rust-lang.org/)

`vigen see` analyzes images with Google Gemini · `vigen gen` creates images with OpenAI DALL·E

A CLI tool built for AI agents, terminal enthusiasts, and anyone who wants vision capabilities without leaving the command line.

</div>

---

## ✨ What It Does

Most AI models can't see or create images. Vigen fixes that with two simple commands:

| Command | What | How |
|---------|------|-----|
| `vigen see` | 👁️ Analyze images | Send to Google Gemini for description, OCR, object detection |
| `vigen gen` | 🎨 Generate images | Send prompts to OpenAI DALL·E for image creation |

But it's not just a wrapper — vigen is built for **reliability**:

- **Multi-endpoint fallback** — Primary provider down? Automatically tries the next one.
- **URL image extraction** — Provider returns a URL instead of base64? Downloaded and converted automatically.
- **Style reference** — Feed an existing image to Gemini for style analysis, then generate something new in that style.

## 📦 Installation

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

> Requires Rust 1.80+

## 🚀 Quick Start

```bash
# 1. Set up your API keys
vigen auth key google <your-gemini-api-key>
vigen auth key gpt <your-openai-api-key>

# 2. Analyze an image
vigen see -i photo.jpg -p "What's in this image?"

# 3. Generate an image
vigen gen -p "a cat wearing a spacesuit, digital illustration" -o ./output
```

### Piping

```bash
# Pipe images into vigen see
cat screenshot.png | vigen see -p "Extract all error messages"
```

## 🎨 Image Generation

```bash
# Basic — saves to current directory
vigen gen -p "A golden retriever in a sunflower field, oil painting style"

# Choose your aspect ratio
vigen gen -p "Mountain panorama" --size 1536x1024      # Landscape
vigen gen -p "Character portrait" --size 1024x1536      # Portrait
vigen gen -p "Profile picture" --size 1024x1024         # Square (default)

# Batch generation
vigen gen -p "Abstract ocean waves" --n 3

# Output format: png (default), jpg, webp
vigen gen -p "Product photo" --format jpg -o ~/Pictures/

# Use a reference image for style guidance (analyzed by Gemini first)
vigen gen -p "Cyberpunk cityscape at night" -r style_reference.png
```

#### Prompt Tips

| ✅ Good | ❌ Bad |
|---------|--------|
| "A golden retriever puppy, watercolor style, warm lighting" | "dog" |
| "Cyberpunk cityscape, neon reflections on wet streets, 3D render" | "make it look cool" |
| "Minimalist logo, geometric shapes, black and white" | "a logo for my company" |

## 👁️ Image Analysis

```bash
# Describe an image in detail
vigen see -i photo.jpg

# OCR — extract text from screenshots, receipts, documents
vigen see -i receipt.jpg -p "Transcribe all text, preserve layout"

# Diagram / chart analysis
vigen see -i flowchart.png -p "Explain this flowchart step by step"

# Object identification
vigen see -i room.jpg -p "List every object you can identify"

# Code screenshot reading
vigen see -i error.png -p "Extract the error message and suggest a fix"
```

## ⚙️ Configuration

Config file: `$XDG_CONFIG_HOME/vigen/config.toml` (defaults to `~/.config/vigen/config.toml`)

```toml
[proxy]
url = "http://127.0.0.1:7890"                    # Optional global proxy

[providers.google]
api_keys = [                                      # Multiple keys — rotated automatically
    "AIzaSy...",
    "AIzaSy...",
]
model = "gemini-2.0-flash"
fallback_model = "gemini-1.5-flash"               # Try this model if primary fails

[providers.gpt]
api_key = "sk-..."
model = "gpt-image-2"
base_url = "https://api.openai.com"               # Works with any OpenAI-compatible API
image_endpoint = "/v1/images/generations"         # Or /v1/chat/completions
fallback_model = "dall-e-2"                       # Try this model if primary fails

# Backup endpoints — tried in order when primary fails
[[providers.gpt.fallbacks]]
api_key = "sk-backup-..."
base_url = "https://backup-provider.com"
image_endpoint = "/v1/chat/completions"
model = "custom-model-id"

[[providers.gpt.fallbacks]]
api_key = "sk-another-..."
base_url = "https://another-provider.com"
```

### 🔄 Fallback System

Vigen has a two-level fallback system so your requests don't die with a single provider outage:

```
Level 1: Within-endpoint fallback
┌─────────────────────┐
│  primary model       │──fail──▶ fallback_model
└─────────────────────┘

Level 2: Across-endpoint fallback
┌─────────────────────┐    ┌──────────────────┐    ┌──────────────────┐
│  primary endpoint    │──▶ │  fallback #1      │──▶ │  fallback #2      │
└─────────────────────┘    └──────────────────┘    └──────────────────┘
```

- **Non-fatal errors** (timeouts, 5xx, 429, etc.) → continue to next
- **Fatal errors** (401, 403) → stop immediately (your key is probably wrong)

Each fallback endpoint can optionally override `api_key`, `base_url`, `image_endpoint`, and `model`. Fields omitted in a fallback inherit from the primary config.

### 🌐 Proxy

```toml
[proxy]
url = "http://127.0.0.1:7890"                    # Global proxy

[providers.gpt]
proxy = "socks5://127.0.0.1:1080"                # Per-provider override
```

Supports HTTP and SOCKS5.

## 🛠️ All Commands

```
vigen see -i <path> [-p <prompt>] [-v]              Analyze image (Google Gemini)
vigen gen -p <prompt> [--size] [-n] [-o] [-r] [-v]  Generate image (OpenAI DALL·E)
vigen auth key <google|gpt> <key>                    Set API key directly
vigen auth login --provider <google|gpt>             Browser OAuth login
vigen auth login --provider gpt --device-auth        Device flow auth
vigen auth login --provider gpt --with-api-key       Interactive API key entry
vigen model <google|gpt> <model>                     Switch default model
vigen models [--provider <google|gpt>]               List available models
vigen proxy <url>                                    Set proxy
vigen project <project_id>                           Set GCP project ID (Google OAuth)
vigen config show                                    Show current config
vigen config path                                    Print config file path
vigen config init                                    Initialize config file
```

## 🧠 AI Agent Skills

Vigen ships with skill files that teach AI coding assistants how to use `vigen see` and `vigen gen`. These are plain Markdown files — any tool that supports skill/knowledge files can use them.

### For OpenCode

```bash
cp -r skills/vigen-gen ~/.config/opencode/skills/
cp -r skills/vigen-see ~/.config/opencode/skills/
```

### For Cursor / Windsurf / Other AI Tools

The `skills/` directory contains two self-contained Markdown files:

| File | Purpose |
|------|---------|
| `skills/vigen-gen/SKILL.md` | Image generation — triggers, usage, prompt tips |
| `skills/vigen-see/SKILL.md` | Image analysis — triggers, usage, prompt tips |

Copy them into your tool's knowledge/rules directory, or paste the contents into your custom instructions. Each file contains trigger words, usage examples, and best practices.

## 📋 Requirements

- **Rust** 1.80+ (for building)
- **Google Gemini** API key for `vigen see`
- **OpenAI** API key (or compatible) for `vigen gen`

## License

[MIT](LICENSE)
