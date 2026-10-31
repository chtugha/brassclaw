# Multi-stage Dockerfile for the BrassClaw agent (cloud deployment).
#
# Uses cargo-chef for dependency caching — only rebuilds deps when
# Cargo.toml/Cargo.lock change, not on every source edit.
#
# Debian-based build + runtime. The embedded Postgres C code has
# threading issues when statically linked against musl, so we use glibc.
#
# Build:
#   docker build --platform linux/amd64 --target runtime -t brassclaw:latest .
#
# Run:
#   docker run --env-file .env -p 3000:3000 brassclaw:latest

# Stage 1: Install cargo-chef
FROM rust:1.96-bookworm AS chef

RUN apt-get update \
    && apt-get install -y --no-install-recommends python3 \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install --locked cargo-chef@0.1.78

WORKDIR /app

# Stage 2: Generate the dependency recipe (changes only when Cargo.toml/lock change)
FROM chef AS planner

COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY vendor/ vendor/
COPY src/ src/
COPY tests/ tests/
COPY migrations/ migrations/
COPY *.md ./
COPY docs/ docs/
COPY scripts/ scripts/

RUN python3 -m unittest discover -s scripts/ci -p test_normalize_cargo_chef_recipe.py \
    && cargo chef prepare --recipe-path recipe.json \
    && python3 scripts/ci/normalize_cargo_chef_recipe.py recipe.json

# Stage 3: Build dependencies (cached unless Cargo.toml/lock change)
FROM chef AS deps

# Preserve the package panic strategy: host and task failure boundaries need
# unwinding in containers as well as in the cargo-dist release binaries.
ENV CARGO_PROFILE_DIST_CODEGEN_UNITS=1

COPY --from=planner /app/recipe.json recipe.json
# Path patches outside the workspace are not included in the recipe skeleton.
COPY --from=planner /app/vendor/ vendor/
RUN cargo chef cook --locked --profile dist --recipe-path recipe.json

# Stage 4: Build the actual binary (only recompiles brassclaw source)
FROM deps AS builder

COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY vendor/ vendor/
COPY src/ src/
COPY tests/ tests/
COPY migrations/ migrations/
COPY *.md ./
COPY docs/ docs/
COPY scripts/ scripts/

RUN cargo build --locked --profile dist -p brassclaw -p brassclaw_monty_host --bins

# Stage 5a: Shared runtime base
FROM debian:bookworm-slim AS runtime-base

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/dist/brassclaw /usr/local/bin/brassclaw
COPY --from=builder /app/target/dist/monty_worker /usr/local/bin/monty_worker
COPY --from=builder /app/migrations /app/migrations

# Non-root user
ENV HOME=/home/brassclaw
RUN useradd -m -d /home/brassclaw -u 1000 brassclaw \
    && mkdir -p /home/brassclaw/.brassclaw \
    && chown -R brassclaw:brassclaw /home/brassclaw
WORKDIR /home/brassclaw

EXPOSE 3000

ENV RUST_LOG=brassclaw=info

ENTRYPOINT ["brassclaw"]

# Stage 5b: Production runtime
FROM runtime-base AS runtime
USER brassclaw
