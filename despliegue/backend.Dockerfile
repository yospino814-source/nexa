# Backend Rust. Contexto de build: ../backend
FROM rust:1-bookworm AS build
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked && cp target/release/nexa-backend /nexa-backend

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates libssl3 curl \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --no-create-home nexa
COPY --from=build /nexa-backend /usr/local/bin/nexa-backend
USER nexa
ENV PORT=8080
EXPOSE 8080
CMD ["nexa-backend"]
