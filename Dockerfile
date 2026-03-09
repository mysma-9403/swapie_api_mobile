# ── Stage 1: Build ────────────────────────────────────────────────────────────
FROM rust:1.83-slim-bookworm AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
# Cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release 2>/dev/null || true
RUN rm -rf src

COPY . .
RUN touch src/main.rs && cargo build --release

# ── Stage 2: Runtime ─────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

RUN useradd -r -s /bin/false swapie

COPY --from=builder /app/target/release/swapie_backend /usr/local/bin/swapie_backend
COPY --from=builder /app/locales /opt/swapie/locales
COPY --from=builder /app/migrations /opt/swapie/migrations

WORKDIR /opt/swapie

RUN chown -R swapie:swapie /opt/swapie
USER swapie

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8080/api/v1/genres || exit 1

CMD ["swapie_backend"]
