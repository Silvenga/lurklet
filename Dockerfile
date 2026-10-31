FROM ghcr.io/pnpm/pnpm:12.17.0 AS frontend-build

WORKDIR /app

COPY pnpm-lock.yaml pnpm-workspace.yaml ./

RUN set -xe \
    && pnpm fetch --frozen-lockfile

COPY --parents \
    package.json \
    lurklet-api/openapi-spec.json \
    lurklet-web/package.json \
    lurklet-web/openapi-ts.config.ts \
    lurklet-web/tsconfig.app.json \
    ./

RUN set -xe \
    && pnpm install --offline --frozen-lockfile

COPY --parents \
    .gitignore \
    tsconfig.json \
    oxlint.config.ts \
    oxfmt.config.ts \
    lurklet-web \
    ./

RUN set -xe \
    && pnpm lint \
    && pnpm test \
    && pnpm publish:web

FROM rust:1-slim-trixie AS backend-chef

RUN set -xe \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
    g++ \
    make \
    cmake \
    git \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

RUN set -xe \
    && cargo install --locked cargo-chef \
    && rustup component add clippy

WORKDIR /app

FROM backend-chef AS backend-planner

COPY --parents \
    Cargo.toml \
    Cargo.lock \
    README.md \
    LICENSE \
    .config \
    lurklet-api \
    lurklet-reddit \
    lurklet-reddit-cli \
    ./

RUN set -xe \
    && cargo chef prepare --recipe-path recipe.json

FROM backend-chef AS backend-builder

COPY --from=backend-planner /app/recipe.json recipe.json

RUN set -xe \
    && cargo chef cook --release --locked --workspace --all-targets --recipe-path recipe.json \
    && cargo chef cook --release --locked --workspace --all-targets --clippy --recipe-path recipe.json \
    && cargo chef cook --release --locked --package lurklet-api --bin lurklet-api --recipe-path recipe.json

COPY --from=backend-planner /app/ ./

RUN set -xe \
    && cargo clippy --release --locked --workspace --all-targets -- -D warnings \
    && cargo test --release --locked --workspace \
    && cargo build --release --locked --package lurklet-api --bin lurklet-api

RUN install -d -m 0755 /cache

FROM gcr.io/distroless/cc-debian13:nonroot AS backend-runtime

COPY --from=backend-builder --chmod=0555 /app/target/release/lurklet-api /usr/local/bin/lurklet-api
COPY --from=frontend-build /app/lurklet-web/dist/ /app/static/
COPY --from=backend-builder --chown=65532:65532 /cache /cache

ENV LURKLET_BIND=0.0.0.0:3000 \
    LURKLET_STATIC_DIR=/app/static \
    LURKLET_STATE_FILE=/cache/state.json

EXPOSE 3000/tcp
VOLUME ["/cache"]
USER 65532:65532
HEALTHCHECK \
    --interval=30s \
    --timeout=5s \
    --start-period=10s \
    --retries=3 \
    CMD ["/usr/local/bin/lurklet-api", "health-check"]
ENTRYPOINT ["/usr/local/bin/lurklet-api"]
