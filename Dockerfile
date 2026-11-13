# syntax=docker/dockerfile:1.27@sha256:4edf897a3ffa55b89f906fc8cc78afdb3f1834cc9c7083565e611a8a7d5fe99e
# Stages: base -> chef -> planner -> builder -> prod   (production image)
#         base -> dev                                  (development image)
# The tag of this image is the toolchain version: it equals rust-toolchain.toml and rust-version (G31 rule (c)).
FROM rust:1.99.1-slim-trixie@sha256:f6c4a35d21dbb3c89d7a8d056c193e94fc09037f2b58dd858b09c39a47dd02d7 AS base
WORKDIR /app
ENV CARGO_TERM_COLOR=always \
    CARGO_NET_RETRY=10 \
    CARGO_INCREMENTAL=0 \
    SQLX_OFFLINE=true

FROM base AS chef
ARG CARGO_CHEF_VERSION=0.1.78
RUN cargo install cargo-chef --locked --version "${CARGO_CHEF_VERSION}" \
 && rm -rf "${CARGO_HOME}/registry"

# The recipe holds manifests and the lock file only: it changes only when dependencies change.
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Dependency layer: rebuilt only when a manifest or Cargo.lock changes.
RUN cargo chef cook --release --locked --package service --recipe-path recipe.json
# Application layer. `.sqlx/` and `migrations/` arrive with this COPY.
COPY . .
RUN cargo build --release --locked --package service --bin service \
 && install -D target/release/service /out/service

FROM base AS dev
SHELL ["/bin/bash", "-o", "pipefail", "-c"]
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl git jq make xz-utils \
 && rm -rf /var/lib/apt/lists/*
RUN rustup component add clippy rustfmt llvm-tools-preview
COPY make/tool-versions.mk /tmp/tooling/make/tool-versions.mk
COPY scripts/install-tools.sh scripts/install-bats.sh /tmp/tooling/scripts/
RUN --mount=type=secret,id=github_token,env=GITHUB_TOKEN \
    /tmp/tooling/scripts/install-tools.sh --with-watchexec \
 && /tmp/tooling/scripts/install-bats.sh --prefix /opt/bats \
 && rm -rf /tmp/tooling "${CARGO_HOME}/registry"
# Any uid may use the image: cargo state, the home directory and the build directory are world-writable
# mount points outside /app, so nothing is created in a bind-mounted checkout. The home directory has no sticky
# bit: the common-utils feature of .devcontainer/ appends as root to the .bashrc it gave the user there, which
# fs.protected_regular (set on hosted runners) refuses in a sticky world-writable directory owned by root.
RUN mkdir -p /cargo /home/dev /cargo-target \
 && chmod 1777 /cargo /cargo-target \
 && chmod 0777 /home/dev
ENV CARGO_INCREMENTAL=1 \
    CARGO_HOME=/cargo \
    CARGO_TARGET_DIR=/cargo-target \
    HOME=/home/dev \
    BATS_LIB_PATH=/opt/bats/lib \
    PATH=/opt/bats/core/bin:/usr/local/cargo/bin:${PATH}
EXPOSE 8081
CMD ["watchexec", "--restart", "--stop-signal", "SIGTERM", "--watch", "crates", "--watch", "migrations", "--watch", "Cargo.toml", "--watch", "Cargo.lock", "--", "cargo", "run", "--locked", "-p", "service", "--", "serve"]

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:cc66d222f0a7ec996b78a99fe76a969bc80b539a5016ad866c14d1949bc6d0ab AS prod
LABEL org.opencontainers.image.licenses="CC0-1.0"
COPY --from=builder /out/service /app/service
ENV APP_ENV=prod \
    APP_HOST=0.0.0.0 \
    APP_PORT=8081
USER 65532:65532
EXPOSE 8081
STOPSIGNAL SIGTERM
# distroless has no shell, curl or wget: the binary probes itself (exec form is mandatory).
HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=3 CMD ["/app/service", "healthcheck"]
ENTRYPOINT ["/app/service"]
CMD ["serve"]
