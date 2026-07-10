# syntax=docker/dockerfile:1.7
# Runtime image — built in CI; optional locally if you have Docker only.

FROM rust:bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY config ./config
RUN cargo build --release -p simplineage-cli \
    && strip target/release/simplineage

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 --shell /usr/sbin/nologin simplineage
COPY --from=builder /app/target/release/simplineage /usr/local/bin/simplineage
COPY config /etc/simplineage/config
USER simplineage
WORKDIR /home/simplineage
ENV SIMPLINEAGE_STORAGE__DATA_DIR=/home/simplineage/.simplineage
ENV RUST_LOG=info
ENTRYPOINT ["simplineage"]
CMD ["hello"]
