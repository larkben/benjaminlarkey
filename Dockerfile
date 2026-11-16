FROM rust:1.89-slim AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY data ./data
RUN cargo build --release

FROM debian:bookworm-slim AS runtime

WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /app/data \
    && chown -R 10001:10001 /app

COPY --from=builder --chown=10001:10001 /app/target/release/benjaminlarkey /usr/local/bin/benjaminlarkey
COPY --from=builder --chown=10001:10001 /app/data /app/data

ENV PORT=8080
EXPOSE 8080
USER 10001:10001

CMD ["/usr/local/bin/benjaminlarkey"]
