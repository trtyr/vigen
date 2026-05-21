<div align="center">

# vigen

**Vision + Gen — 给纯文本模型插上眼睛和画笔。**

[![crates.io](https://img.shields.io/crates/v/vigen.svg)](https://crates.io/crates/vigen)
[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](https://www.rust-lang.org/)

`vigen see` 用 Google Gemini 识图 · `vigen gen` 用 OpenAI DALL·E 生图

为 AI Agent、终端爱好者和所有需要视觉能力的命令行用户打造的 CLI 工具。

[English](README.md) · 中文

</div>

---

## ✨ 它能干什么

大部分 AI 模型看不见也画不了图。vigen 用两个命令解决这个问题：

| 命令 | 干什么 | 怎么干 |
|------|--------|--------|
| `vigen see` | 👁️ 分析图片 | 发给 Google Gemini 做描述、OCR、物体识别 |
| `vigen gen` | 🎨 生成图片 | 发 prompt 给 OpenAI DALL·E 生成图片 |

但它不只是一个 API 包装器 — vigen 为**可靠性**而生：

- **多端点轮询** — 主 provider 挂了？自动试下一个。
- **URL 图片提取** — provider 返回的是 URL 而不是 base64？自动下载转换。
- **风格参考** — 先用 Gemini 分析参考图的风格，再融合到生成 prompt 里。

## 📦 安装

### 从 crates.io 安装

```bash
cargo install vigen
```

### 从源码编译

```bash
git clone https://github.com/trtyr/vigen.git
cd vigen
cargo install --path .
```

> 需要 Rust 1.80+

## 🚀 快速开始

```bash
# 1. 设置 API 密钥
vigen auth key google <你的-Gemini-API-Key>
vigen auth key gpt <你的-OpenAI-API-Key>

# 2. 分析图片
vigen see -i photo.jpg -p "这张图里有什么？"

# 3. 生成图片
vigen gen -p "一只穿着宇航服的猫，数字插画风格" -o ./output
```

### 管道输入

```bash
# 把图片通过管道传给 vigen see
cat screenshot.png | vigen see -p "提取所有错误信息"
```

## 🎨 图片生成

```bash
# 基础用法 — 保存到当前目录
vigen gen -p "金毛犬在向日葵花田里，油画风格"

# 选择宽高比
vigen gen -p "山脉全景" --size 1536x1024      # 横版
vigen gen -p "人物肖像" --size 1024x1536      # 竖版
vigen gen -p "头像" --size 1024x1024          # 正方形（默认）

# 批量生成
vigen gen -p "抽象海浪水彩画" --n 3

# 输出格式：png（默认）、jpg、webp
vigen gen -p "产品照片" --format jpg -o ~/Pictures/

# 用参考图引导风格（先用 Gemini 分析风格，再融合到 prompt）
vigen gen -p "赛博朋克城市夜景" -r style_reference.png
```

#### Prompt 技巧

| ✅ 好的写法 | ❌ 差的写法 |
|------------|-----------|
| "金毛犬幼崽，水彩风格，暖光" | "狗" |
| "赛博朋克城市，湿漉漉的街道上的霓虹倒影，3D 渲染" | "搞得酷一点" |
| "极简 logo，几何图形，黑白" | "帮我公司设计个 logo" |

## 👁️ 图片分析

```bash
# 详细描述图片
vigen see -i photo.jpg

# OCR — 从截图、收据、文档中提取文字
vigen see -i receipt.jpg -p "转录所有文字，保持原始排版"

# 分析流程图 / 图表
vigen see -i flowchart.png -p "逐步解释这个流程图"

# 物体识别
vigen see -i room.jpg -p "列出你能识别的所有物体"

# 读取代码截图
vigen see -i error.png -p "提取错误信息并建议修复方案"
```

## ⚙️ 配置

配置文件位置：`$XDG_CONFIG_HOME/vigen/config.toml`（默认 `~/.config/vigen/config.toml`）

```toml
[proxy]
url = "http://127.0.0.1:7890"                    # 可选全局代理

[providers.google]
api_keys = [                                      # 多个密钥 — 自动轮询
    "AIzaSy...",
    "AIzaSy...",
]
model = "gemini-2.0-flash"
fallback_model = "gemini-1.5-flash"               # 主模型失败时尝试这个

[providers.gpt]
api_key = "sk-..."
model = "gpt-image-2"
base_url = "https://api.openai.com"               # 支持任何 OpenAI 兼容 API
image_endpoint = "/v1/images/generations"         # 也支持 /v1/chat/completions
fallback_model = "dall-e-2"                       # 主模型失败时尝试这个

# 备用端点 — 主端点失败时按顺序尝试
[[providers.gpt.fallbacks]]
api_key = "sk-backup-..."
base_url = "https://backup-provider.com"
image_endpoint = "/v1/chat/completions"
model = "custom-model-id"

[[providers.gpt.fallbacks]]
api_key = "sk-another-..."
base_url = "https://another-provider.com"
```

### 🔄 Fallback 系统

vigen 有两层 fallback 机制，让你的请求不会因为单个 provider 挂掉就失败：

```
第一层：同端点内的模型 fallback
┌─────────────────────┐
│  主模型              │──失败──▶ fallback_model
└─────────────────────┘

第二层：跨端点的 provider fallback
┌─────────────────────┐    ┌──────────────────┐    ┌──────────────────┐
│  主端点              │──▶ │  备用端点 #1       │──▶ │  备用端点 #2       │
└─────────────────────┘    └──────────────────┘    └──────────────────┘
```

- **非致命错误**（超时、5xx、429 等）→ 继续尝试下一个
- **致命错误**（401、403）→ 立即停止（你的密钥可能有问题）

每个备用端点可以单独设置 `api_key`、`base_url`、`image_endpoint` 和 `model`。省略的字段会继承主配置。

### 🌐 代理

```toml
[proxy]
url = "http://127.0.0.1:7890"                    # 全局代理

[providers.gpt]
proxy = "socks5://127.0.0.1:1080"                # 单独覆盖此 provider 的代理
```

支持 HTTP 和 SOCKS5。

## 🛠️ 所有命令

```
vigen see -i <文件> [-p <提示词>] [-v]                分析图片（Google Gemini）
vigen gen -p <提示词> [--size] [-n] [-o] [-r] [-v]    生成图片（OpenAI DALL·E）
vigen auth key <google|gpt> <key>                     直接设置 API 密钥
vigen auth login --provider <google|gpt>              浏览器 OAuth 登录
vigen auth login --provider gpt --device-auth         设备流认证
vigen auth login --provider gpt --with-api-key        交互式输入 API 密钥
vigen model <google|gpt> <model>                      切换默认模型
vigen models [--provider <google|gpt>]                列出可用模型
vigen proxy <url>                                     设置代理
vigen project <project_id>                            设置 GCP 项目 ID（Google OAuth 需要）
vigen config show                                     显示当前配置
vigen config path                                     打印配置文件路径
vigen config init                                     初始化配置文件
```

## 🧠 AI Agent Skills

vigen 附带 skill 文件，教 AI 编程助手如何使用 `vigen see` 和 `vigen gen`。它们是纯 Markdown 文件 — 任何支持 skill/知识文件的工具都能用。

### OpenCode

```bash
cp -r skills/vigen-gen ~/.config/opencode/skills/
cp -r skills/vigen-see ~/.config/opencode/skills/
```

### Cursor / Windsurf / 其他 AI 工具

`skills/` 目录包含两个独立的 Markdown 文件：

| 文件 | 用途 |
|------|------|
| `skills/vigen-gen/SKILL.md` | 图片生成 — 触发词、用法、prompt 技巧 |
| `skills/vigen-see/SKILL.md` | 图片分析 — 触发词、用法、prompt 技巧 |

把它们复制到你工具的知识/规则目录，或者直接粘贴内容到自定义指令中。每个文件都包含触发词、使用示例和最佳实践。

## 📋 环境要求

- **Rust** 1.80+（编译用）
- **Google Gemini** API 密钥（`vigen see` 用）
- **OpenAI** API 密钥或兼容 API（`vigen gen` 用）

## 许可证

[MIT](LICENSE)
