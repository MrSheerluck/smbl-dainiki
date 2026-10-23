FROM rust:1.99-bookworm AS builder

WORKDIR /app

RUN cargo install wasm-pack --version 0.15.0 --locked

COPY . .

RUN wasm-pack build crates/client --target web --out-dir ../../web/client/pkg
RUN cargo build --release -p dainiki-web

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/dainiki-web /usr/local/bin/dainiki-web
COPY --from=builder /app/web/client /app/web/client

ENV HOST=0.0.0.0
ENV PORT=3000

EXPOSE 3000

HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=5 \
    CMD curl -fsS http://127.0.0.1:3000/api/health || exit 1

CMD ["dainiki-web"]
