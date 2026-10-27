# lurklet-api

Supports `lurklet-web` (the SPA frontend) by hosting the frontend's static content and through exposing a REST API.

## Ownership

- A thin wrapper across `lurklet-reddit`.
- Hosts the SPA's static assets when deployed.
- Provides a REST API to the SPA, the only consumer.
- Provides a configuration surface for operators using Clap (CLI flags and environment variables).
- Exposes an OpenAPI spec to consumers.

## Conventions

- Favor accepting `RedditId` directly when the backing `lurklet-reddit` API uses it.