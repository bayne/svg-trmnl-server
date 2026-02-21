# syntax=docker/dockerfile:1

# ---------------------------------------------------------------------------
# Stage 1 – Build
# ---------------------------------------------------------------------------
FROM rust:1.85-bookworm AS builder

WORKDIR /build

# Cache dependencies separately from source so layer rebuilds are fast.
# Copy manifests first; the dummy main forces Cargo to compile deps.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo 'fn main(){}' > src/main.rs \
    && cargo build --release \
    && rm -rf src

# Now copy real source and rebuild only the application crate.
COPY src ./src
# Touch main.rs so Cargo detects the change even though mtime may match.
RUN touch src/main.rs && cargo build --release

# ---------------------------------------------------------------------------
# Stage 2 – Runtime
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# resvg / tiny-skia need these shared libraries at runtime.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        libfontconfig1 \
    && rm -rf /var/lib/apt/lists/*

# Non-root user for least-privilege operation.
RUN useradd --no-create-home --shell /bin/false appuser

WORKDIR /app

# Copy the compiled binary.
COPY --from=builder /build/target/release/svg-trmnl-server /app/svg-trmnl-server

# Copy static assets that the server needs at runtime.
# The config volume is mounted at /app/config in the container; these are
# the remaining read-only assets baked into the image.
COPY fonts     ./fonts
COPY templates ./templates

# A default (example) config is included for reference; operators should
# mount their own config.toml via a ConfigMap / volume.
COPY config/config.example.toml ./config/config.example.toml

RUN chown -R appuser:appuser /app
USER appuser

EXPOSE 9080

ENTRYPOINT ["/app/svg-trmnl-server"]
# Pass --listen and --config-path as CMD so they can be overridden at runtime.
CMD ["--listen=0.0.0.0:9080", "--config-path=/app/config/config.toml"]
