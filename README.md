# vigen

> Vision + Gen — 给纯文本模型插上眼睛和画笔。

一个轻量 CLI 工具：`vigen see` 用 Google Gemini 识图，`vigen gen` 用 OpenAI 生图。

## 安装

```bash
cargo install --path .
```

要求 Rust 1.80+。

## 快速开始

```bash
# 设置 API 密钥
vigen auth key google <your-gemini-api-key>
vigen auth key gpt <your-openai-api-key>

# 识图
vigen see -i photo.jpg -p "这张图里有什么？"

# 生图
vigen gen -p "a cat wearing a spacesuit" --size 1024x1024 -o ./output
```

## 所有命令

```
vigen see -i <文件> [-p <提示词>]      识图（Google Gemini）
vigen gen -p <提示词> [--size] [-n] [-o]  生图（OpenAI）
vigen auth login --provider <google|gpt>  浏览器 OAuth 登录
vigen auth key <google|gpt> <key>         直接设置 API key
vigen model <google|gpt> <model>          切换模型
vigen models [--provider <google|gpt>]    列出可用模型
vigen proxy <url>                         设置代理
vigen project <project_id>                设置 GCP 项目 ID（Google OAuth 需要）
vigen config show|path|init              管理配置文件
```

## 配置

配置文件在 `~/.config/vigen/config.toml`：

```toml
[proxy]
url = "http://127.0.0.1:7890"  # 可选

[providers.google]
api_key = "sk-xxx"
model = "gemini-2.0-flash"
fallback_model = "gemini-1.5-flash"  # 可选，主模型失败时自动切换

[providers.gpt]
api_key = "sk-xxx"
model = "gpt-image-2"
```

## 设计原则

- **Google 识图，GPT 生图** — 每个命令对应一个提供者，没有 `--provider` 参数。
- **零配置 OAuth** — 用 `vigen auth login` 浏览器登录，不需要手动申请 client ID。
- **代理支持** — 全局或按提供者设置 HTTP/SOCKS5 代理。
