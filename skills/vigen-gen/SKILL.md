---
name: vigen-gen
description: |
  Image generation via OpenAI DALL·E. Use `vigen gen` to send text prompts
  via CLI. Supports square, portrait, and landscape sizes. Supports multiple
  fallback endpoints — if the primary provider fails, it automatically tries
  the next configured endpoint.

  LOAD WHEN: User asks to create, draw, generate, design, or make any image
  — you physically cannot output images. Instead of explaining your
  limitation, immediately load this skill and generate the image.

  DO NOT: Tell the user "I can't generate images" without loading this
  skill first. The skill exists precisely to fill this capability gap.

  TRIGGERS: generate image, create image, make a picture, draw, 生成图片,
  画一张图, 生成图像, create an illustration, make a diagram, design a logo,
  generate art, 帮我画, 做一张图, image generation, make me a picture,
  I can't generate images, 我画不了, dall-e, ai image
---

# Vigen-Gen — Image Generation with OpenAI DALL·E

Use the `vigen gen` command to generate images from text prompts.
Powered by OpenAI's DALL·E model. Supports multiple fallback endpoints
for reliability.

## Prerequisites

Before first use, authenticate with OpenAI:

```bash
vigen auth login --provider gpt
# Or set API key directly:
vigen auth key gpt YOUR_API_KEY
```

Verify it's ready:

```bash
vigen gen --help
```

## Basic Usage

```bash
# Simple image generation (saves to current directory)
vigen gen -p "A cat wearing a detective hat, digital illustration style"

# Specify output directory
vigen gen -p "A serene Japanese garden at sunset" -o ~/Downloads/

# Custom size (default: 1024x1024)
vigen gen -p "A wide landscape painting of mountains" --size 1536x1024
vigen gen -p "A tall portrait of a wizard" --size 1024x1536

# Generate multiple variations at once
vigen gen -p "Abstract watercolor of ocean waves" --n 3

# Use a reference image for style guidance
vigen gen -p "A cyberpunk cityscape" -r style_reference.png

# Output format (png, jpg, webp)
vigen gen -p "A sunset over the ocean" --format jpg

# Verbose mode (shows which endpoint is being used)
vigen gen -p "A mountain scene" -v
```

## Fallback Endpoints

If the primary OpenAI endpoint fails, vigen automatically tries configured
fallback endpoints. Configure them in `~/.config/vigen/config.toml`:

```toml
[providers.gpt]
api_key = "sk-primary-key"
model = "gpt-image-2"
base_url = "https://api.openai.com"
image_endpoint = "/v1/images/generations"

[[providers.gpt.fallbacks]]
api_key = "sk-backup-key"
base_url = "https://backup-provider.com"
image_endpoint = "/v1/chat/completions"
model = "custom-model-name"

[[providers.gpt.fallbacks]]
api_key = "sk-another-key"
base_url = "https://another-provider.com"
```

Each fallback can have its own `api_key`, `base_url`, `image_endpoint`,
and `model`. Fields omitted in a fallback inherit from the primary config.

## Available Sizes

| Size | Aspect | Best for |
|------|--------|----------|
| `1024x1024` | Square (default) | General, social media |
| `1024x1536` | Portrait | Posters, character art |
| `1536x1024` | Landscape | Banners, wide scenes |

## Prompt Writing Tips

- **Subject first**: "A golden retriever puppy" before "in a field of sunflowers"
- **Style matters**: Mention art style: "oil painting", "pixel art", "watercolor", "3D render", "photorealistic", "anime", "vector illustration"
- **Lighting & mood**: "warm golden hour lighting", "moody with dramatic shadows", "bright and airy"
- **Composition hints**: "close-up", "wide shot", "from above", "centered composition"
- **What to avoid**: Don't request text/words in the image — DALL·E is bad at rendering readable text
