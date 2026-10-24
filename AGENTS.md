# Lurklet

Lurklet is a lightweight and modern privacy front-end for Reddit.

## Preflight

- Load the `conventions-code-rust` once in a session before modifying rust code.

## Structure

- `lurklet-api/`: Rust API binary, depending on `lurklet-reddit`.
- `lurklet-reddit/`: Reusable Rust library for communicating with Reddit, independent of the API server.

## Verification

After relevant code changes:

- Format: `cargo fmt`
- Lint: `cargo clippy`
- Test: `cargo test`
