# ---- Stage 1: Planner (cargo-chef) ----
FROM rust:latest AS chef
RUN apt-get update && apt-get install -y pkg-config libssl-dev musl-tools && rm -rf /var/lib/apt/lists/*
RUN rustup target add x86_64-unknown-linux-musl
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
FROM alpine:latest AS runtime
RUN apk add --no-cache ca-certificates
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/fnine /usr/local/bin/fnine
COPY --from=builder /app/static /app/static

# Data directory via volume mount
VOLUME ["/app/data"]

EXPOSE 3000
ENV FNINE_HOST=0.0.0.0
ENV FNINE_PORT=3000
ENV FNINE_DATA_DIR=/app/data

CMD ["/usr/local/bin/fnine"]
