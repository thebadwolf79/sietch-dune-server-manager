FROM rust:1.82-bookworm AS builder
WORKDIR /src
COPY Cargo.toml Cargo.toml
COPY src src
RUN cargo build --release

FROM debian:bookworm-slim
RUN useradd --create-home --uid 10001 manager \
  && apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/*
COPY --from=builder /src/target/release/dune-manager-api /usr/local/bin/dune-manager-api
USER manager
EXPOSE 8787
ENTRYPOINT ["/usr/local/bin/dune-manager-api"]
