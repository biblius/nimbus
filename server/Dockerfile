FROM rust:1.98-bookworm AS build

WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY server ./server
RUN cargo build --release -p nimbus-server --no-default-features --features postgres,valkey

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates ffmpeg \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /src/target/release/nimbus-server /usr/local/bin/nimbus-server

ENV ADDRESS=0.0.0.0:8080 \
    UPLOAD_DIR=/var/lib/nimbus

EXPOSE 8080
ENTRYPOINT ["nimbus-server"]
