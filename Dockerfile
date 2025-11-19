# 1) Prepare dependency build (for fast incremental builds)
FROM rust:slim AS chef
RUN cargo install cargo-chef --version 0.1.72 --locked
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# 2) Build stage (cached dependencies)
FROM rust:slim AS builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --version 0.1.72 --locked

WORKDIR /app
COPY --from=chef /app/recipe.json recipe.json
# Build dependencies only (will be nicely cached)
RUN cargo chef cook --release --recipe-path recipe.json

# Now build your app
COPY . .
RUN cargo build --release --bin maratus

# 3) Small runtime image
FROM debian:bookworm-slim AS runtime
RUN useradd -m appuser
WORKDIR /app

# Copy the compiled binary only
COPY --from=builder /app/target/release/maratus /usr/local/bin/maratus

USER appuser
EXPOSE 8080
CMD ["maratus"]