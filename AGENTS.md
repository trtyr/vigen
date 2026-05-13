# vigen

Vision + Gen — CLI tool for text-only models to access vision AI.

**Updated:** 2026-05-12
**Branch:** master (no commits yet)

## Stack

Rust (edition 2021), tokio, reqwest, clap, serde, base64, sha2, dirs, webbrowser.

## Structure

```
src/
├── main.rs            # CLI (clap): thin dispatch to providers
├── config.rs          # TOML config: ProviderType, DefaultsConfig, provider structs, load/save
├── error.rs           # VigenError enum (IO, HTTP, API, Config, OAuth)
├── pkce.rs            # shared PKCE helpers (verifier, challenge, port picker)
└── providers/
    ├── mod.rs         # VisionProvider trait + ProviderType + dispatch (analyze, login, list_models)
    ├── google.rs      # GoogleProvider: config → Gemini API + OAuth login
    └── gpt.rs       # GptProvider: config → OpenAI API + 3-way auth
```

Microkernel: each provider module is self-contained (auth + API + config). `mod.rs` is the thin dispatch layer. `pkce.rs` is the only shared utility.

## Where to look

| Task | Location | Notes |
|------|----------|-------|
| Add CLI command | `src/main.rs` | clap derive, dispatch to providers/mod |
| Add provider | `src/providers/<name>.rs` + register in `mod.rs` | impl VisionProvider + login functions + config struct |
| Change config schema | `src/config.rs` | add provider config struct, keep ProviderType in config.rs |
| Change Google provider | `src/providers/google.rs` | API calls, OAuth, model listing |
| Change Gpt provider | `src/providers/gpt.rs` | API calls, PKCE browser, device code, API key |
| Add shared PKCE utility | `src/pkce.rs` | used by all providers |
| Error handling | `src/error.rs` | VigenError enum, Display + Error impl |

## Architecture

- **`VisionProvider` trait** — `async fn analyze_image(&self, image_data: &[u8], mime_type: &str, prompt: &str) -> Result<String>`
- **`ProviderType` enum** — Google / Gpt. `parse(s)` for CLI strings. Lives in config.rs with serde as TOML string.
- **`DefaultsConfig`** — `defaults.vision` (primary), `defaults.vision_fallback` (auto-failover). Each provider config has `model` + `fallback_model`. Fallback chain: primary/model → primary/fallback_model → vision_fallback/model → vision_fallback/fallback_model. `--provider` on CLI overrides and disables cross-provider fallback.
- **Auth modes** — API key (priority) > OAuth Bearer token. OAuth uses provider-specific public client credentials, no user config needed.
- **Provider dispatch** — `providers::analyze_image(pt, config, ...)`, `providers::login(pt, config, ...)`, `providers::list_models(pt, config)` in mod.rs route to the correct provider.
- **Proxy** — global `proxy.url` in config, per-provider override. HTTP and SOCKS5 via reqwest.
- **Config** — XDG `~/.config/vigen/config.toml`, TOML. Sections: `[defaults]`, `[proxy]`, `[providers.google]`, `[providers.gpt]`, `[auth]`.

## Key commands

```
vigen see -i <path> [-p <prompt>] [--provider <google|gpt>]
vigen config show | path | init
vigen auth key <google|gpt> <key>
vigen model <google|gpt> <model>
vigen proxy <url>
vigen project <project_id>
vigen models [--provider <google|gpt>]
vigen auth login --provider <google|gpt>
vigen auth login --provider gpt --device-auth
vigen auth login --provider gpt --with-api-key
vigen auth key <google|gpt> <key>
vigen model <google|gpt> <model>
vigen proxy <url>
vigen project <project_id>
vigen models [--provider <google|gpt>]
vigen auth login --provider <google|gpt>
vigen auth login --provider gpt --device-auth
vigen auth login --provider gpt --with-api-key
```
vigen see -i <path> [-p <prompt>] [--provider <google|gpt>]
vigen config show | path | init
vigen config set-key <google|gpt> <key>
vigen config set-model <google|gpt> <model>
vigen config set-proxy <url>
vigen config set-project <project_id>
vigen config list-models [--provider <google|gpt>]
vigen login --provider <google|gpt>
vigen login --provider gpt --device-auth
vigen login --provider gpt --with-api-key
vigen config list-models [--provider <google|gpt>]
vigen login --provider <google|gpt>
vigen login --provider gpt --device-auth
vigen login --provider gpt --with-api-key
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
   - `impl VisionProvider` with `analyze_image()`
   - At least one `login_*` function that takes `&mut VigenConfig` and saves auth to config
   - Use `crate::pkce` for OAuth PKCE flows
2. Add `ProviderType` variant in `src/providers/mod.rs`
3. Register in `analyze_image`, `login`, `list_models` match arms in `mod.rs`
4. Add config struct in `src/config.rs` (add to `ProviderConfigs` and `AuthConfig` if needed)

## Conventions

- No comments unless absolutely necessary (no section separators).
- Use `anyhow` for CLI errors, `VigenError` for library errors.
- Keep clap doc comments — they show in `--help`.
- Compile with `cargo build` (zero warnings required).
