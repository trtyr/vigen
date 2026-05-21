# vigen

Vision + Gen — CLI tool for text-only models to access vision AI and image generation.

**Updated:** 2026-05-21
**Branch:** master

## Stack

Rust (edition 2021), tokio, reqwest, clap, serde, base64, sha2, dirs, webbrowser, url, image.

## Structure

```
src/
├── main.rs            # CLI (clap): thin dispatch to providers
├── config.rs          # TOML config: ProviderType, provider structs, load/save
├── error.rs           # VigenError enum (IO, HTTP, API, Config, OAuth)
├── pkce.rs            # shared PKCE helpers (verifier, challenge, port picker)
└── providers/
    ├── mod.rs         # VisionProvider + ImageGenProvider traits, dispatch (analyze, generate, login, list_models)
    ├── google.rs      # GoogleProvider: Gemini vision API + OAuth login + model listing
    ├── gpt.rs         # GptProvider: OpenAI image generation API + OAuth + API key + custom endpoint
    └── http.rs        # shared HTTP retry helper (send_with_retry)
```

Microkernel: each provider module is self-contained (auth + API + config). `mod.rs` is the thin dispatch layer. `pkce.rs` and `http.rs` are the shared utilities.

## Where to look

| Task | Location | Notes |
|------|----------|-------|
| Add CLI command | `src/main.rs` | clap derive, dispatch to providers/mod |
| Change CLI behavior (format, stdin, error hints) | `src/main.rs` | format conversion (image crate), error_hint(), stdin MIME detection |
| Add provider | `src/providers/<name>.rs` + register in `mod.rs` | impl VisionProvider or ImageGenProvider + login functions + config struct |
| Change config schema | `src/config.rs` | add provider config struct, keep ProviderType in config.rs |
| Change Google provider | `src/providers/google.rs` | Gemini vision API, OAuth, model listing |
| Change Gpt provider | `src/providers/gpt.rs` | OpenAI image generation API, OAuth login, API key auth, custom endpoint |
| Add shared PKCE utility | `src/pkce.rs` | used by all providers |
| Error handling | `src/error.rs` | VigenError enum, Display + Error impl |
| HTTP retry logic | `src/providers/http.rs` | send_with_retry (3 attempts, exponential backoff) |

## Architecture

- **Google = 识图, GPT = 生图** — each command maps to a single provider. No `--provider` flags.
- **`VisionProvider` trait** — `async fn analyze_image(&self, image_data: &[u8], mime_type: &str, prompt: &str) -> Result<String>`. Only `GoogleProvider` implements it.
- **`ImageGenProvider` trait** — `async fn generate_image(&mut self, prompt: &str, size: &str, n: u8) -> Result<Vec<String>>`. Only `GptProvider` implements it. Mutable for OAuth token refresh.
- **`ProviderType` enum** — Google / Gpt. `parse(s)` for CLI strings. Lives in config.rs with serde as TOML string.
- **Fallback** — two levels: (1) within-endpoint: main model → fallback_model, (2) across-endpoints: primary → fallbacks list. Each fallback has its own api_key, base_url, image_endpoint, model. Non-fatal errors continue to next; fatal errors short-circuit.
- **Auth modes** — API key (priority) > OAuth Bearer token. Gpt uses Codex client OAuth. Google uses standard Google Cloud OAuth. No user-provided client secrets needed.
- **Proxy** — global `proxy.url` in config, per-provider override. HTTP and SOCKS5 via reqwest.
- **Config** — `$XDG_CONFIG_HOME/vigen/config.toml` (defaults to `~/.config/vigen/config.toml`), TOML. Sections: `[proxy]`, `[providers.google]`, `[providers.gpt]`, `[auth]`.
- **Custom endpoint** — Gpt supports custom `base_url` and `image_endpoint` for third-party OpenAI-compatible APIs. Multiple endpoints via `[[providers.gpt.fallbacks]]` array in config.
- **Retry** — all provider HTTP calls use `send_with_retry` (3 attempts, exponential backoff) for connect/timeout/5xx/429 errors.
- **Gen --reference** — reads reference image → Gemini analyzes style (colors, composition, lighting) → merges into prompt before sending to GPT.
- **Format conversion** — `src/main.rs` handles PNG → JPEG/WebP via the `image` crate. Provider returns base64, main.rs decodes and converts.
- **URL image extraction** — when provider returns markdown image links (`![...](url)`) instead of base64, GptProvider auto-downloads the image and converts to base64. Supports any OpenAI-compatible API that returns URL-based responses.
- **Error hints** — `error_hint()` in `src/main.rs` maps `VigenError` variants to actionable CLI suggestions (e.g. "run `vigen auth key google <key>`").

## Key commands

```
vigen see -i <path> [-p <prompt>] [-v]          # or pipe: cat img.png | vigen see
vigen gen -p <prompt> [--size <s>] [-n <n>] [-o <dir>] [--format png|jpg|webp] [--stdout] [-r <ref>] [-v]
vigen config show | path | init
vigen auth key <google|gpt> <key>
vigen model <google|gpt> <model>
vigen proxy <url>
vigen project <project_id>
vigen models [--provider <google|gpt>]
vigen auth login --provider <google|gpt>
vigen auth login --provider gpt --device-auth
vigen auth login --provider gpt --with-api-key
```

## Commands

```bash
cargo build              # Debug build
cargo run -- <args>      # Run CLI
cargo test               # All tests
cargo clippy             # Lint
```

## Adding a new provider

1. Create `src/providers/<name>.rs` with:
   - Provider struct + `from_config(&VigenConfig)`
   - `impl VisionProvider` or `impl ImageGenProvider`
   - At least one `login_*` function that takes `&mut VigenConfig` and saves auth to config
   - Use `crate::pkce` for OAuth PKCE flows
2. Add `ProviderType` variant in `src/providers/mod.rs`
3. Register in `analyze_image`, `generate_image`, `login`, `list_models` match arms in `mod.rs`
4. Add config struct in `src/config.rs` (add to `ProviderConfigs` and `AuthConfig` if needed)

## Conventions

- No comments unless absolutely necessary (no section separators).
- Use `anyhow` for CLI errors, `VigenError` for library errors.
- Keep clap doc comments — they show in `--help`.
- Compile with `cargo build` (zero warnings required).
- All provider HTTP calls must use `crate::providers::http::send_with_retry`, not bare `client.send()`.
- Wrap `.json().await` and `.text().await` results with `VigenError::http(context, err)` for context-rich errors.
