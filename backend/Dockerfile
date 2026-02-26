# ── Stage 1: Plan dependencies ──
FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo chef prepare --recipe-path recipe.json

# ── Stage 2: Build dependencies (cached) + app ──
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Cook dependencies — this layer is cached as long as Cargo.toml/Cargo.lock don't change
RUN cargo chef cook --release --recipe-path recipe.json
# Now copy source and build the actual app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
RUN cargo build --release

# ── Stage 3: Minimal runtime ──
FROM debian:trixie-slim AS runtime
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/geminihydra-backend /usr/local/bin/app
COPY migrations ./migrations
ENV PORT=8081
EXPOSE 8081
HEALTHCHECK --interval=30s --timeout=5s CMD curl -f http://localhost:8081/api/health || exit 1
CMD ["app"]
