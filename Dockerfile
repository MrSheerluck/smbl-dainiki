FROM rust:1.99-bookworm AS builder

WORKDIR /app

RUN cargo install wasm-pack --version 0.15.0 --locked \
    && cargo install topcoat-cli --version 0.10.0 --locked

COPY . .

RUN wasm-pack build crates/client --target web --out-dir ../../web/client/pkg
RUN cargo build --release -p dainiki-web
RUN topcoat asset bundle -p dainiki-web --release

FROM debian:bookworm-slim

ARG DAINIKI_ASSET_VERSION=dev
ENV DAINIKI_ASSET_VERSION=$DAINIKI_ASSET_VERSION

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

RUN groupadd --system --gid 10001 dainiki \
    && useradd --system --uid 10001 --gid dainiki --home-dir /app \
        --shell /usr/sbin/nologin dainiki

COPY --from=builder /app/target/release/dainiki-web /usr/local/bin/dainiki-web
COPY --from=builder /app/target/release/assets /usr/local/bin/assets
COPY --from=builder /app/web/client /app/web/client

ENV HOST=0.0.0.0
ENV PORT=3000

EXPOSE 3000

HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=5 \
    CMD curl -fsS http://127.0.0.1:3000/api/health || exit 1

USER 10001:10001

CMD ["dainiki-web"]
