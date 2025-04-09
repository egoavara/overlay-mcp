FROM rust:1.86.0-bookworm AS builder

WORKDIR /app

COPY . .

RUN cargo build --release

FROM debian:bookworm

COPY --from=builder /app/target/release/overlay-mcp /usr/local/bin/overlay-mcp

ENTRYPOINT ["/usr/local/bin/overlay-mcp"]
