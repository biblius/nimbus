# syntax=docker/dockerfile:1

# --- embedded web UI ---
FROM node:22-bookworm AS web

WORKDIR /src
COPY package.json package-lock.json ./
COPY client/shared/package.json ./client/shared/
COPY client/web/package.json ./client/web/
COPY client/desktop/package.json ./client/desktop/
RUN npm ci

COPY client/shared ./client/shared
COPY client/web ./client/web
ENV VITE_NIMBUS_URL=/api
RUN npm run build -w @nimbus/web

# --- rust binary (postgres; suitable for k8s / compose) ---
FROM rust:1.98-bookworm AS build

WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY server ./server
COPY --from=web /src/client/web/dist ./client/web/dist

ARG NIMBUS_FEATURES=postgres
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --release -p nimbus-server --no-default-features --features "${NIMBUS_FEATURES}" \
    && strip target/release/nimbus-server \
    && cp target/release/nimbus-server /nimbus-server

# --- runtime ---
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl ffmpeg \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 nimbus \
    && useradd --system --uid 10001 --gid nimbus \
        --home-dir /var/lib/nimbus --shell /usr/sbin/nologin \
        --create-home nimbus

COPY --from=build /nimbus-server /usr/local/bin/nimbus-server

ENV ADDRESS=0.0.0.0:8080 \
    DATA_DIR=/var/lib/nimbus \
    RUST_LOG=info

VOLUME ["/var/lib/nimbus"]
EXPOSE 8080

USER nimbus:nimbus
WORKDIR /var/lib/nimbus

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -fsS -o /dev/null "http://127.0.0.1:8080/_health" || exit 1

ENTRYPOINT ["nimbus-server"]
