# Distribution image for `wf`. Two-stage:
#   1. Compile statically-linkable Rust binary with rustls-tls-webpki-roots
#      so we don't need a system CA bundle at runtime.
#   2. Drop the binary + the `skills/` tree onto distroless/cc.
#
# Designed to be COPY'd from in downstream projects. Dockerfiles can pull
# /usr/local/bin/wf and /skills/ via
# `COPY --from=ghcr.io/klh-homes/wealthfolio-cli:vX.Y.Z`).
# It also works as a standalone container:
#   docker run --rm \
#     -e WEALTHFOLIO_BASE_URL=... -e WEALTHFOLIO_PASSWORD=... \
#     ghcr.io/klh-homes/wealthfolio-cli:vX.Y.Z accounts list

ARG RUST_VERSION=1.95.0
ARG DEBIAN_CODENAME=trixie

FROM rust:${RUST_VERSION}-slim-${DEBIAN_CODENAME} AS builder
WORKDIR /build
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
# Pre-fetch deps in a layer separate from the source so subsequent
# source-only edits don't re-download crates.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs \
    && cargo build --release \
    && rm -rf src target/release/deps/wealthfolio_cli* target/release/wf*
COPY . .
RUN cargo build --release && strip target/release/wf

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=builder /build/target/release/wf /usr/local/bin/wf
COPY skills /skills
USER nonroot
ENTRYPOINT ["/usr/local/bin/wf"]
