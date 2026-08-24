FROM rust:1.90-bookworm AS builder

WORKDIR /app
COPY . .
RUN test -f src/api/media/mod.rs && test -f src/models/media/mod.rs
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates ffmpeg \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/backend /usr/local/bin/vanta-backend
COPY --from=builder /app/migrations ./migrations

ENV VANTA_BIND=""
CMD ["vanta-backend"]
