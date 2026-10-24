# Lurklet

Lurklet is a lightweight and modern privacy front-end for Reddit.

## Preflight

- Load the `conventions-code-rust` once in a session before modifying rust code.

## Structure

- `lurklet-api/`: Rust API binary, depending on `lurklet-reddit`.
- `lurklet-reddit/`: Reusable Rust library for communicating with Reddit, independent of the API server.

## Logging

Logging should be privacy-aware. The default log level is `info`.

- `error`: Issues that must be fixed by an operator and cannot be automatically recovered.
- `warn`: Issues that an operator would need to know about, but can be automatically recovered.
- `info`: Logging that would be useful to any operator to see.
- `debug`: Logging that would be useful to an operator debugging an issue. May log sensitive information.
- `trace`: Logging that would only be useful to a developer.

No personal information or security-sensitive information should be logged at levels `info` or higher. It is expected
that `info` and higher log levels can be publicly uploaded.

## Verification

After relevant code changes:

- Format: `cargo fmt`
- Lint: `cargo clippy`
- Test: `cargo test`
