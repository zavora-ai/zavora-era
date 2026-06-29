# Multi-stage build for the Zavora ERP API (Rust workspace).
# Stage 1: build the release binary. Stage 2: slim runtime image.

FROM rust:1.90-slim-bookworm AS builder
WORKDIR /build

# System deps for building (pdf-extract is pure-Rust; sqlx uses rustls so no
# native openssl needed, but pkg-config/ca-certs help some transitive crates).
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the whole workspace and build the API binary in release mode.
COPY Cargo.toml Cargo.lock ./
COPY zavora-erp-core ./zavora-erp-core
COPY zavora-erp-api ./zavora-erp-api
COPY migrations ./migrations
RUN cargo build --release --bin zavora-erp-api

# Stage 2: minimal runtime.
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /build/target/release/zavora-erp-api /usr/local/bin/zavora-erp-api
# Migrations are embedded at compile time via sqlx::migrate!, so no runtime copy
# is strictly needed, but keep them for reference/debugging.
COPY --from=builder /build/migrations /app/migrations
EXPOSE 8080
CMD ["zavora-erp-api"]
