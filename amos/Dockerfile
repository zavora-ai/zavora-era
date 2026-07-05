# Amos AI — production image.
#
# Amos path-depends on ../../adk-rust (its crate dir is two levels below the
# framework checkout), so the builder recreates that layout by cloning
# adk-rust at a pinned ref. The mcp-erp server (zavora backend) is built in
# the same stage; the runtime bakes Node + Playwright Chromium so the
# showcase browser runs headless inside the container.
#
# Build context: the zavora-era repo root (amos/ is copied from it).

ARG ADK_RUST_REF=fix/gemini-batched-tool-calls
ARG MCP_ERP_REF=feat/zavora-backend
ARG PLAYWRIGHT_MCP_VERSION=0.0.77

# ── Stage 1: build amos + mcp-erp ────────────────────────────────────────────
# adk-rust pins rustc 1.94 (rust-toolchain.toml).
FROM rust:1.94-slim-bookworm AS builder
ARG ADK_RUST_REF
ARG MCP_ERP_REF
WORKDIR /build

RUN apt-get update && apt-get install -y --no-install-recommends \
    git pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# adk-rust at /build/adk-rust — satisfies amos's ../../adk-rust path deps
# when amos sits at /build/zavora-era/amos.
RUN git clone --depth 1 --branch "${ADK_RUST_REF}" \
    https://github.com/zavora-ai/adk-rust.git /build/adk-rust

# mcp-erp with the zavora backend.
RUN git clone --depth 1 --branch "${MCP_ERP_REF}" \
    https://github.com/zavora-ai/mcp-erp.git /build/mcp-erp \
    && cargo build --release --features zavora --manifest-path /build/mcp-erp/Cargo.toml

COPY amos /build/zavora-era/amos
RUN cargo build --release --manifest-path /build/zavora-era/amos/Cargo.toml

# ── Stage 2: runtime with Node + headless Chromium ───────────────────────────
FROM node:20-bookworm-slim AS runtime
ARG PLAYWRIGHT_MCP_VERSION
ENV PLAYWRIGHT_BROWSERS_PATH=/ms-playwright

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Warm the npx cache for the pinned @playwright/mcp, then install the exact
# Chromium build its bundled playwright-core expects (with system deps).
RUN npx --yes "@playwright/mcp@${PLAYWRIGHT_MCP_VERSION}" --version \
    && PW_CLI=$(find /root/.npm/_npx -path '*playwright-core*/cli.js' | head -1) \
    && node "$PW_CLI" install --with-deps chromium \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /build/zavora-era/amos/target/release/amos /usr/local/bin/amos
COPY --from=builder /build/mcp-erp/target/release/mcp-erp /usr/local/bin/mcp-erp
COPY --from=builder /build/mcp-erp/mcp-server.toml /app/mcp-server.toml
COPY amos/mcp.json amos/system.md amos/AGENTS.md /app/
COPY amos/skills /app/skills

# Point every file-based config at the image locations (the compiled-in
# CARGO_MANIFEST_DIR defaults reference the build tree, which doesn't exist here).
ENV AMOS_MCP_JSON=/app/mcp.json \
    AMOS_SYSTEM_MD=/app/system.md \
    AMOS_AGENTS_MD=/app/AGENTS.md \
    AMOS_SKILLS_DIR=/app/skills \
    AMOS_SHOWCASE_DIR=/app/showcase \
    AMOS_MCP_ERP_BIN=/usr/local/bin/mcp-erp \
    AMOS_PLAYWRIGHT_MCP_VERSION=${PLAYWRIGHT_MCP_VERSION} \
    AMOS_BROWSER_CHANNEL=chromium \
    AMOS_BROWSER_HEADLESS=1 \
    AMOS_PORT=8090

EXPOSE 8090
CMD ["amos"]
