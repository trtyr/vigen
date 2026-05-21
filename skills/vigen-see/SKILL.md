---
name: vigen-see
description: |
  Image analysis via Google Gemini vision. Use `vigen see` to send images
  for description, OCR text extraction, object identification, and
  diagram/chart reading.

  LOAD WHEN: User sends a screenshot, photo, diagram, or any image file
  — you physically cannot see images. Instead of explaining your limitation,
  immediately load this skill and analyze the image.

  DO NOT: Tell the user "I can't see images" without loading this skill
  first. The skill exists precisely to fill this capability gap.

  TRIGGERS: image, screenshot, photo, picture, diagram, chart, visual,
  see this, look at this, view this, 图片, 截图, 照片, 看图, 看看这个,
  what's in this image, describe this image, analyze this image, read this,
  extract text from, image recognition, OCR, what does this show, can you see,
  can't see images, cannot view images
---

# Vigen-See — Image Recognition with Google Gemini

Use the `vigen see` command to analyze images. Powered by Google Gemini's
vision model — supports png, jpg, webp, gif, bmp.

## Prerequisites

Before first use, authenticate with Google:

```bash
vigen auth login --provider google
# Or set API key directly:
vigen auth key google YOUR_API_KEY
```

Verify it's ready:

```bash
vigen see --help
```

## Basic Usage

```bash
# Describe an image (default prompt: "Describe this image in detail")
vigen see -i /path/to/image.png

# Custom prompt — be specific about what you want to know
vigen see -i screenshot.png -p "Extract all error messages shown in this screenshot"

# Read text from a screenshot / document photo
vigen see -i receipt.jpg -p "Transcribe all text visible in this image, preserve the layout"

# Analyze a diagram or chart
vigen see -i architecture.png -p "Describe this diagram: what components are shown, how are they connected, what does the flow look like"

# Describe a photo in detail
vigen see -i photo.webp -p "Describe this photo in rich detail — setting, subjects, mood, lighting, composition"

# Identify objects in an image
vigen see -i room.jpg -p "List every object you can identify in this image"

# Pipe image data via stdin
cat photo.png | vigen see -p "What is this a photo of?"

# Verbose mode
vigen see -i diagram.png -p "Explain this flowchart" -v
```

## Prompt Writing Tips

- **Be specific**: "Extract all error messages" beats "What's in this image"
- **Ask for structure**: "List items from top to bottom" or "Describe left to right"
- **Set context**: Tell Gemini what kind of image it is ("This is a terminal screenshot", "This is a UI mockup")
- **Request format**: "Return as numbered list", "Output as JSON with keys: objects, text, colors"
