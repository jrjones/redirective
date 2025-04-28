# Stage 1: build
FROM rust:1.74-slim AS builder
WORKDIR /usr/src/app
# Install dependencies for building
RUN apt-get update && apt-get install -y pkg-config libssl-dev git ca-certificates && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY links.yaml redirective.toml ./
RUN cargo build --release
# Stage 2: runtime
FROM gcr.io/distroless/cc
WORKDIR /app
# Copy binary and config files
COPY --from=builder /usr/src/app/target/release/redirective /usr/local/bin/redirective
COPY --from=builder /usr/src/app/links.yaml ./
COPY --from=builder /usr/src/app/redirective.toml ./
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/redirective"]