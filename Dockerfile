# =============================================================================
# Multi-architecture Dockerfile for Fnine
# =============================================================================
#
# Supported platforms:
#   linux/amd64          — x86_64 servers / desktops
#   linux/arm64          — Raspberry Pi 3/4/5 (64-bit OS)
#
# NOTE: linux/arm/v7 (32-bit ARM) is not supported because the official
# Rust Alpine image does not provide an arm/v7 manifest.
#
# Single-platform local build:
#   docker build -t fnine .
#
# Multi-platform build + push (requires buildx):
#   docker buildx create --use
#   docker buildx build --platform linux/amd64,linux/arm64 -t ghcr.io/user/fnine --push .
# =============================================================================

# ---- Stage 1: Planner (cargo-chef) ----
FROM library/rust:1.96-alpine3.23 AS chef

# Map Docker's TARGETPLATFORM to the matching Rust target triple.
# This ARG is injected automatically by Buildx; falls back to the build host arch.
ARG TARGETPLATFORM
RUN echo "[fnine] TARGETPLATFORM = ${TARGETPLATFORM}" && \
    case "${TARGETPLATFORM}" in \
      "linux/amd64")   RUST_TARGET="x86_64-unknown-linux-musl" ;; \
      "linux/arm64")   RUST_TARGET="aarch64-unknown-linux-musl" ;; \
      *)               echo "ERROR: Unsupported platform ${TARGETPLATFORM}" ; exit 1 ;; \
    esac && \
    echo "[fnine] Rust target  = ${RUST_TARGET}" && \
    echo "${RUST_TARGET}" > /rust_target && \
    rustup target add "${RUST_TARGET}"

# Use domestic Alpine mirror (USTC) — uncomment if building in China
# RUN sed -i 's|dl-cdn.alpinelinux.org|mirrors.ustc.edu.cn|g' /etc/apk/repositories

RUN apk add --no-cache pkgconfig openssl-dev musl-dev

# Use domestic cargo mirror (rsproxy) — uncomment if building in China
# RUN mkdir -p $CARGO_HOME && \
#     printf '[source.crates-io]\nreplace-with = "rsproxy"\n[source.rsproxy]\nregistry = "sparse+https://rsproxy.cn/index/"\n' \
#     > $CARGO_HOME/config.toml

RUN cargo install cargo-chef
WORKDIR /app

# ---- Stage 2: Prepare recipe ----
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ---- Stage 3: Build dependencies (cached by recipe.json) ----
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN RUST_TARGET=$(cat /rust_target) && \
    cargo chef cook --release --target "${RUST_TARGET}" --recipe-path recipe.json

# ---- Stage 4: Build application ----
COPY . .
RUN RUST_TARGET=$(cat /rust_target) && \
    cargo build --release --target "${RUST_TARGET}" && \
    cp "/app/target/${RUST_TARGET}/release/fnine" /app/fnine

# ---- Stage 5: Runtime (minimal Alpine) ----
FROM library/alpine:3.23 AS runtime
RUN apk add --no-cache ca-certificates
WORKDIR /app

# Single common path — the builder stage already copies the correct binary to /app/fnine
COPY --from=builder /app/fnine /usr/local/bin/fnine
COPY --from=builder /app/static /app/static

# Data directory via volume mount
VOLUME ["/app/data"]

EXPOSE 3000
ENV FNINE_HOST=0.0.0.0
ENV FNINE_PORT=3000
ENV FNINE_DATA_DIR=/app/data

CMD ["/usr/local/bin/fnine"]
