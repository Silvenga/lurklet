# lurklet-web

This is the SPA interface for the `lurklet-api`.

## Priorities

> Considerations for every added and updated modification to this project.

- A11y: When writing Aria labels, be succinct, and be mindful of the users time.
- Dark/Light modes: Respect the choice of the user by supporting dark and light modes for all features.

## Conventions

- User selection is disabled by default. Components that expect user selection should enable it directly using `select-text` or `select-all` as relevant. Avoid allowing users to select headers and clickable elements.
- Create mocks for all supported endpoints - allowing development without the API server running.

## OpenAPI Spec

When re-generating the OpenAPI spec is required:

```sh
cargo run --locked --quiet -p lurklet-api -- export-openapi > lurklet-api/openapi-spec.json
pnpm generate:api
```
