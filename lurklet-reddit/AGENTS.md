# lurklet-reddit

An SDK for `lurlet-api`, abstracting upstream Reddit.

## Ownership

- Connecting directly to Reddit.

## Conventions

- Use contract testing using captured/live results to allow `lurklet-reddit` to be tested offline.
- Keep `lurklet-reddit-cli` in-sync with added/updated `lurklet-reddit` methods. Use `lurklet-reddit-cli` for manual validation and to capture real-results.
- Backends are considered swappable (only the mobile backend currently exists) - maintain high cohesion with a backend and low coupling between backends and the `RedditClient`.
- Avoid hardcoding values, use the builder pattern to override reasonable defaults.