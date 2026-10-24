FROM rust:1-slim-trixie AS chef

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

FROM chef AS planner

COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder

COPY --from=planner /app/recipe.json recipe.json

RUN set -xe \
    && cargo chef cook --release --locked --workspace --all-targets --recipe-path recipe.json \
    && cargo chef cook --release --locked --workspace --all-targets --clippy --recipe-path recipe.json \
    && cargo chef cook --release --locked --package lurklet-api --bin lurklet-api --recipe-path recipe.json

COPY . .

RUN set -xe \
    && cargo clippy --release --locked --workspace --all-targets -- -D warnings \
    && cargo test --release --locked --workspace \
    && cargo build --release --locked --package lurklet-api --bin lurklet-api

FROM gcr.io/distroless/cc-debian13:nonroot AS runtime

COPY --from=builder --chmod=0555 /app/target/release/lurklet-api /usr/local/bin/lurklet-api

ENV LURKLET_BIND=0.0.0.0:3000
EXPOSE 3000/tcp
USER 65532:65532
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["/usr/local/bin/lurklet-api", "health-check"]
ENTRYPOINT ["/usr/local/bin/lurklet-api"]
