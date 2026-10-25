# --- web UI (embedded into the server binary) ---
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

# --- rust binary ---
FROM rust:1.98-bookworm AS build

WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY server ./server
COPY --from=web /src/client/web/dist ./client/web/dist

RUN cargo build --release -p nimbus-server --no-default-features --features postgres

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates ffmpeg \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /src/target/release/nimbus-server /usr/local/bin/nimbus-server

ENV ADDRESS=0.0.0.0:8080 \
    UPLOAD_DIR=/var/lib/nimbus

EXPOSE 8080
ENTRYPOINT ["nimbus-server"]
