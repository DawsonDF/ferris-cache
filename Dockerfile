# Stage 1: build
FROM rust:alpine AS builder

ARG TARGETARCH

RUN apk add --no-cache musl-dev

# Map Docker's TARGETARCH to the correct Rust musl target triple
RUN case "$TARGETARCH" in \
      amd64) echo "x86_64-unknown-linux-musl"  > /rust_target ;; \
      arm64) echo "aarch64-unknown-linux-musl" > /rust_target ;; \
      *) echo "Unsupported architecture: $TARGETARCH" >&2 && exit 1 ;; \
    esac && rustup target add "$(cat /rust_target)"

WORKDIR /app

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release --target "$(cat /rust_target)"
RUN rm src/main.rs

# Build real binary
COPY src ./src
RUN touch src/main.rs && cargo build --release --target "$(cat /rust_target)"
RUN cp "target/$(cat /rust_target)/release/ferris-cache" /ferris-cache

# Stage 2: runtime
FROM gcr.io/distroless/static-debian12

COPY --from=builder /ferris-cache /ferris-cache

EXPOSE 7878
ENTRYPOINT ["/ferris-cache"]
