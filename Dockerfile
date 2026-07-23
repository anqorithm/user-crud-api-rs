FROM rust:1.78-slim as builder

WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libpq-dev && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock* ./
RUN mkdir src

COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libpq5 curl && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/user_crud_api /usr/local/bin/

EXPOSE 8000

CMD ["user_crud_api"]