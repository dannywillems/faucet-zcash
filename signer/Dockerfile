# Multi-stage build for the faucet signer. Build context is the repo root so
# the workspace (faucet-core path dependency) is available.
FROM rust:1.96-bookworm AS build
WORKDIR /app
COPY . .
RUN cargo build --release -p faucet-signer

FROM debian:bookworm-slim AS runtime
# hadolint ignore=DL3008
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/faucet-signer /usr/local/bin/faucet-signer
ENV SIGNER_BIND=0.0.0.0:8080
EXPOSE 8080
USER nobody
HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8080/health || exit 1
ENTRYPOINT ["faucet-signer"]
