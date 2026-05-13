# vigen

Vision + Gen — CLI tool for text-only models to access vision AI.

**Updated:** 2026-05-13
**Branch:** master

## Stack

Rust (edition 2021), tokio, reqwest, clap, serde, base64, sha2, dirs, webbrowser.

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
    └── gpt.rs       # GptProvider: OpenAI image generation API + API key auth
```

Microkernel: each provider module is self-contained (auth + API + config). `mod.rs` is the thin dispatch layer. `pkce.rs` is the only shared utility.

## Where to look

| Task | Location | Notes |
|------|----------|-------|
| Add CLI command | `src/main.rs` | clap derive, dispatch to providers/mod |
| Add provider | `src/providers/<name>.rs` + register in `mod.rs` | impl VisionProvider or ImageGenProvider + login functions + config struct |
| Change config schema | `src/config.rs` | add provider config struct, keep ProviderType in config.rs |
| Change Google provider | `src/providers/google.rs` | Gemini vision API, OAuth, model listing |
| Change Gpt provider | `src/providers/gpt.rs` | OpenAI image generation API, API key auth |
| Add shared PKCE utility | `src/pkce.rs` | used by all providers |
| Error handling | `src/error.rs` | VigenError enum, Display + Error impl |

## Architecture

- **Google = 识图, GPT = 生图** — each command maps to a single provider. No `--provider` flags.
- **`VisionProvider` trait** — `async fn analyze_image(&self, image_data: &[u8], mime_type: &str, prompt: &str) -> Result<String>`. Only `GoogleProvider` implements it.
- **`ImageGenProvider` trait** — `async fn generate_image(&self, prompt: &str, size: &str, n: u8) -> Result<Vec<String>>`. Only `GptProvider` implements it.
- **`ProviderType` enum** — Google / Gpt. `parse(s)` for CLI strings. Lives in config.rs with serde as TOML string.
- **Fallback** — within-provider only: main model → fallback_model. Fatal errors short-circuit.
- **Auth modes** — API key (priority) > OAuth Bearer token. OAuth uses provider-specific public client credentials, no user config needed.
- **Proxy** — global `proxy.url` in config, per-provider override. HTTP and SOCKS5 via reqwest.
- **Config** — XDG `~/.config/vigen/config.toml`, TOML. Sections: `[proxy]`, `[providers.google]`, `[providers.gpt]`, `[auth]`.

## Key commands

```
vigen see -i <path> [-p <prompt>]
vigen gen -p <prompt> [--size <s>] [-n <n>] [-o <dir>]
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
