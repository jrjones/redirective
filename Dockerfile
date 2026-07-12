# Stage 1: build
# -slim-bookworm, NOT -slim: the builder's Debian release must match the runtime stage
# below. Bare `rust:1.97-slim` is trixie (glibc 2.41) and links the binary against
# GLIBC_2.39+, which bookworm (glibc 2.36) cannot load — the image builds and pushes
# clean, then the container exits 1 on `version GLIBC_2.39 not found`.
FROM rust:1.97-slim-bookworm AS builder
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
    && apt-get install -y --no-install-recommends git ca-certificates wget openssh-client \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
# Copy binary and config files
COPY --from=builder /usr/src/app/target/release/redirective /usr/local/bin/redirective
COPY --from=builder /usr/src/app/links.yaml ./
COPY --from=builder /usr/src/app/redirective.toml ./
# Copy static files into the container
COPY static_html ./static_html

# Copy entrypoint script that installs SSH deploy key (if provided) and starts the app
COPY scripts/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]