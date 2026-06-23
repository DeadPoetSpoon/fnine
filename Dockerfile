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
# Build with Chinese mirrors (faster inside China):
#   docker build \
#     --build-arg APK_MIRROR=mirrors.aliyun.com \
#     --build-arg CARGO_MIRROR=sparse+https://mirrors.ustc.edu.cn/crates.io-index/ \
#     -t fnine .
#
# Multi-platform build + push (requires buildx):
#   docker buildx create --use
#   docker buildx build --platform linux/amd64,linux/arm64 -t ghcr.io/user/fnine --push .
# =============================================================================

# ---- Stage 1: Planner (cargo-chef binary, no compilation) ----
FROM library/rust:1.96-alpine3.23 AS chef

# Optionally use a Chinese APK mirror for faster builds inside China.
# Example: --build-arg APK_MIRROR=mirrors.aliyun.com
ARG APK_MIRROR
RUN if [ -n "${APK_MIRROR}" ]; then \
        sed -i "s@dl-cdn.alpinelinux.org@${APK_MIRROR}@g" /etc/apk/repositories ; \
    fi

# Optionally use a Chinese Cargo mirror for faster crate downloads inside China.
# Pass the sparse registry URL, e.g.:
#   --build-arg CARGO_MIRROR=sparse+https://mirrors.ustc.edu.cn/crates.io-index/
ARG CARGO_MIRROR
RUN if [ -n "${CARGO_MIRROR}" ]; then \
        mkdir -p /usr/local/cargo && \
        printf '[source.crates-io]\nreplace-with = "mirror"\n\n[source.mirror]\nregistry = "%s"\n' "${CARGO_MIRROR}" > /usr/local/cargo/config.toml ; \
    fi

# Clean stale APK cache, then install build deps (100% cachable layer).
# 'apk update --no-cache' ensures a fresh index fetch, fixing the
# "v2 database format error" when upstream indexes have changed.
RUN rm -rf /var/cache/apk/* && \
    apk update --no-cache && \
    apk add --no-cache pkgconf openssl-dev musl-dev

# Install cargo-chef (cached after first build).
RUN cargo install cargo-chef --locked

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

# Optionally use a Chinese APK mirror for faster builds inside China.
ARG APK_MIRROR
RUN if [ -n "${APK_MIRROR}" ]; then \
        sed -i "s@dl-cdn.alpinelinux.org@${APK_MIRROR}@g" /etc/apk/repositories ; \
    fi

RUN apk update --no-cache && \
    apk add --no-cache ca-certificates
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
