# lurklet-web

This is the SPA interface for the lurklet-api.

## Conventions - UI

- User selection is disabled by default. Components that expect user selection should enable it directly using
  `select-text` or `select-all` as relevant.

## OpenAPI Spec

When re-generating the OpenAPI spec is required:

```sh
cargo run --locked --quiet -p lurklet-api -- export-openapi > lurklet-api/openapi-spec.json
  
pnpm generate:api
```
