# Stage 1: build
FROM rust:1.86-slim AS builder
WORKDIR /usr/src/app
# Install dependencies for building
RUN apt-get update && apt-get install -y pkg-config libssl-dev git ca-certificates && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY links.yaml redirective.toml ./
RUN cargo build --release
# Stage 2: runtime (with git for git-sync)
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends git ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
# Copy binary and config files
COPY --from=builder /usr/src/app/target/release/redirective /usr/local/bin/redirective
COPY --from=builder /usr/src/app/links.yaml ./
COPY --from=builder /usr/src/app/redirective.toml ./
# Copy static files into the container
COPY static_html ./static_html
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/redirective"]