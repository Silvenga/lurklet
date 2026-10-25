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

## Conventions

- Do not add code without justification of current or immediate future needs. This abstention applies to defensive coding without evidence the additional protections are required.

## Conventions - Rust

- Favor absolute paths in `use` statements over `super::`, except for test modules where `use super::*;` should be used and re-exporting, where specifying the full path duplicates existing `mod` statements.

## Principals

- Favor cacheability of API endpoints where the path/query are the caching key. 
  - Avoid creating endpoints that accept many unrelated inputs for batch retrieval.
  - Each endpoint should have a single purpose.
  - Minimize the returned data for each endpoint - minimizing requested fields avoids needless coupling and improves forward compatiblity.

## Verification

After relevant code changes:

- Format: `cargo fmt` or `pnpm fmt`
- Lint: `cargo clippy --all-targets` or `pnpm lint`
- Test: `cargo test --all-targets` or `pnpm test`
