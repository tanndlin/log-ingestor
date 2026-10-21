# Single Dockerfile for every service. The shared `builder` stage compiles the
# whole workspace once; each service is a small runtime stage selected with
# `target:` in docker-compose.yml.

# ---------- builder ----------
FROM rust:1.99 AS builder
WORKDIR /app

# No DB reachable during `docker build`, so sqlx::query! must check against the
# committed .sqlx/ offline cache instead of a live database.
ENV SQLX_OFFLINE=true

COPY Cargo.toml Cargo.lock ./
COPY util ./util
COPY ingestor ./ingestor
COPY http_ingestor ./http_ingestor
COPY protobuf_ingestor ./protobuf_ingestor
COPY tcp_ingestor ./tcp_ingestor
COPY tanlogs-client ./tanlogs-client
COPY writer ./writer


# The cargo registry and target/ live in BuildKit cache mounts, so deps and
# unchanged crates are reused across builds (and across services) instead of
# being recompiled whenever this layer reruns. Building --workspace (not -p)
# unifies features so every service links against the same dep artifacts.
# Mounts aren't part of the image, so the binaries are copied out to /out.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target,sharing=locked \
    cargo build --release --workspace \
    && mkdir -p /out \
    && cp target/release/ingestor /out/

# ---------- runtime ----------
FROM debian:trixie-slim AS runtime
WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates && update-ca-certificates

FROM runtime AS ingestor
COPY --from=builder /out/ingestor /app/ingestor
CMD ["./ingestor"]
