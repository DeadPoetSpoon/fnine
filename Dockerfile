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

# ---- Stage 1: Planner (cargo-chef binary, no compilation) ----
FROM library/rust:1.96-alpine3.23 AS chef

# Install build dependencies (100% cachable — identical for all platforms)
RUN apk add --no-cache pkgconfig openssl-dev musl-dev wget

# Download pre-built cargo-chef binary instead of compiling from source.
# Saves ~4 minutes on cold builds and makes this layer fully cachable.
ARG CARGO_CHEF_VERSION=0.1.77
RUN ARCH=$(apk --print-arch) && \
    CHEF_ARCH=$(case "${ARCH}" in \
      "x86_64")  echo "x86_64-unknown-linux-musl" ;; \
      "aarch64") echo "aarch64-unknown-linux-musl" ;; \
      *)         echo "unsupported:${ARCH}" ; exit 1 ;; \
    esac) && \
    wget -q "https://github.com/LukeMathWalker/cargo-chef/releases/download/v${CARGO_CHEF_VERSION}/cargo-chef-${CHEF_ARCH}.tar.gz" -O /tmp/chef.tar.gz && \
    tar xzf /tmp/chef.tar.gz -C /usr/local/cargo/bin/ cargo-chef && \
    rm /tmp/chef.tar.gz && \
    chmod +x /usr/local/cargo/bin/cargo-chef && \
    cargo chef --version

WORKDIR /app

# ---- Stage 2: Prepare recipe ----
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ---- Stage 3: Build dependencies (cached by recipe.json) ----
FROM chef AS builder

# ARG declared only in this stage where it's needed, so it doesn't pollute
# the chef stage cache or invalidate layers that don't depend on it.
ARG TARGETPLATFORM
RUN case "${TARGETPLATFORM}" in \
      "linux/amd64")   RUST_TARGET="x86_64-unknown-linux-musl" ;; \
      "linux/arm64")   RUST_TARGET="aarch64-unknown-linux-musl" ;; \
      *)               echo "ERROR: Unsupported platform ${TARGETPLATFORM}" ; exit 1 ;; \
    esac && \
    echo "[fnine] Rust target = ${RUST_TARGET}" && \
    rustup target add "${RUST_TARGET}" && \
    echo "${RUST_TARGET}" > /rust_target

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

COPY --from=builder /app/fnine /usr/local/bin/fnine
COPY --from=builder /app/static /app/static

# Data directory via volume mount
VOLUME ["/app/data"]

EXPOSE 3000
ENV FNINE_HOST=0.0.0.0
ENV FNINE_PORT=3000
ENV FNINE_DATA_DIR=/app/data

CMD ["/usr/local/bin/fnine"]
