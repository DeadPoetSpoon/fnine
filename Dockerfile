# ---- Stage 1: Planner (cargo-chef) ----
FROM library/rust:1.96-alpine3.23 AS chef

# Use domestic Alpine mirror (USTC)
RUN sed -i 's|dl-cdn.alpinelinux.org|mirrors.ustc.edu.cn|g' /etc/apk/repositories

RUN apk add --no-cache pkgconfig openssl-dev musl-dev
RUN rustup target add x86_64-unknown-linux-musl

# Use domestic cargo mirror (rsproxy)
RUN mkdir -p $CARGO_HOME && \
    printf '[source.crates-io]\nreplace-with = "rsproxy"\n[source.rsproxy]\nregistry = "sparse+https://rsproxy.cn/index/"\n' \
    > $CARGO_HOME/config.toml

RUN cargo install cargo-chef
WORKDIR /app

# ---- Stage 2: Prepare recipe ----
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ---- Stage 3: Build dependencies (cached) ----
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json

# ---- Stage 4: Build application ----
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl

# ---- Stage 5: Runtime (minimal Alpine) ----
FROM library/alpine:3.23 AS runtime
RUN apk add --no-cache ca-certificates
WORKDIR /app
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/fnine /usr/local/bin/fnine
COPY --from=builder /app/static /app/static

# Data directory via volume mount
VOLUME ["/app/data"]

EXPOSE 3000
ENV FNINE_HOST=0.0.0.0
ENV FNINE_PORT=3000
ENV FNINE_DATA_DIR=/app/data

CMD ["/usr/local/bin/fnine"]
