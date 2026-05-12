# vigen

Vision + Gen — CLI tool for text-only models to access vision AI.

**Generated:** 2026-05-12
**Branch:** master (no commits yet)

## Stack

Rust (edition 2021), tokio, reqwest, clap, serde, base64, sha2, dirs, webbrowser.

## Structure

```
src/
├── main.rs        # CLI (clap): see, config, login
├── config.rs      # TOML config at ~/.config/vigen/config.toml
├── error.rs       # VigenError enum (IO, HTTP, API, Config, OAuth)
├── auth.rs        # Google OAuth PKCE (browser login, zero-config)
└── providers/
    ├── mod.rs     # VisionProvider trait
    └── google.rs  # Gemini API via generativelanguage.googleapis.com
```

## Where to look

| Task | Location | Notes |
|------|----------|-------|
| Add CLI command | `src/main.rs` | clap derive macros |
| Add provider | `src/providers/<name>.rs` + `mod.rs` | impl VisionProvider trait |
| Change config schema | `src/config.rs` | structs + TOML serialize/deserialize |
| Change auth flow | `src/auth.rs` | PKCE flow, hardcoded Gemini CLI OAuth creds |
| Error handling | `src/error.rs` | VigenError enum, Display + Error impl |

## Architecture

- **`VisionProvider` trait** — `async fn analyze_image(&self, image_data: &[u8], mime_type: &str, prompt: &str) -> Result<String>`
- **Auth modes** — API key (priority) > OAuth Bearer token. OAuth uses Gemini CLI's public client_id, no user config needed: `vigen login` opens browser.
- **Proxy** — global `proxy.url` in config, per-provider override. HTTP and SOCKS5 via reqwest.
- **Config** — XDG `~/.config/vigen/config.toml`, TOML. Sections: `[proxy]`, `[providers.google]`, `[auth.google]`.

## Key commands

```
vigen see -i <path> [-p <prompt>]
vigen config list-models
vigen config set-key <key>
vigen config set-proxy <url>
vigen config set-model <model>
vigen login
```

## Commands

```bash
cargo build              # Debug build
cargo run -- <args>      # Run CLI
cargo test               # All tests (only auth.rs has real tests)
cargo clippy             # Lint
```

## Adding a new provider

1. Create `src/providers/<name>.rs` impl `VisionProvider`
2. Register in `src/providers/mod.rs`
3. Add `[providers.<name>]` config struct in `config.rs`
4. Add CLI subcommand in `main.rs`

## Conventions

- No comments unless absolutely necessary (no section separators).
- Use `anyhow` for CLI errors, `VigenError` for library errors.
- Keep clap doc comments — they show in `--help`.
- Compile with `cargo build` (zero warnings required).
