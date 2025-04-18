# Cross-compile the app for gnu to create a dynamically-linked binary.
FROM --platform=$BUILDPLATFORM rust:1.86.0-bookworm AS builder

ARG TARGETPLATFORM

# Determine Rust target based on TARGETPLATFORM
RUN case "$TARGETPLATFORM" in \
      "linux/amd64") echo x86_64-unknown-linux-gnu > /rust_target.txt ;; \
      "linux/arm64") echo aarch64-unknown-linux-gnu > /rust_target.txt ;; \
      "linux/arm/v7") echo armv7-unknown-linux-gnueabihf > /rust_target.txt ;; \
      "linux/arm/v6") echo arm-unknown-linux-gnueabihf > /rust_target.txt ;; \
      *) exit 1 ;; \
    esac

# Read the target from the file
RUN export RUST_TARGET=$(cat /rust_target.txt) && \
    echo "Selected Rust target: $RUST_TARGET" && \
    rustup target add $RUST_TARGET

# Install necessary build tools including cross-compilers
RUN apt-get update && apt-get install -y --no-install-recommends \
    # Base tools
    binutils \
    # Cross-compilers for required targets (add more as needed)
    gcc-aarch64-linux-gnu \
    gcc-arm-linux-gnueabihf \
    gcc-x86-64-linux-gnu \
    # Cross-compilation C library development files (glibc)
    libc6-dev-arm64-cross \
    libc6-dev-armhf-cross \
    libc6-dev-amd64-cross \
    # Cleanup
    && apt-get clean && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# --- 의존성 캐싱을 위한 변경 시작 ---

# 1. 의존성 파일 먼저 복사
COPY .cargo ./.cargo
COPY Cargo.toml Cargo.lock ./

# 2. 의존성 빌드 (소스 코드 없이)
#    - 임시 main.rs 생성
#    - cargo build 실행
#    - 임시 파일 정리
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    export RUST_TARGET=$(cat /rust_target.txt) && \
    echo "Building dependencies for target: $RUST_TARGET" && \
    # No specific linker needed for standard GNU targets usually
    cargo build --release --target $RUST_TARGET && \
    rm -rf src # 임시 src 디렉토리 삭제

# 3. 실제 소스 코드 복사
COPY src ./src

# 4. 최종 빌드 (의존성 캐시 활용)
#    - cargo build 실행
RUN export RUST_TARGET=$(cat /rust_target.txt) && \
    echo "Building application for target: $RUST_TARGET" && \
    # No specific linker needed for standard GNU targets usually
    cargo build --release --target $RUST_TARGET

# --- 의존성 캐싱을 위한 변경 끝 ---

# Move the binary to a location free of the target since that is not available in the next stage.
RUN cp target/$(cat /rust_target.txt)/release/overlay-mcp .

# Final minimal image using debian (already uses glibc)
FROM debian:bookworm

# Add /usr/local/bin to PATH
ENV PATH="/usr/local/bin:${PATH}"

# Install tini for process management and ca-certificates for HTTPS
# Also install necessary runtime libraries (glibc is already included in debian:bookworm)
RUN apt-get update && apt-get install -y --no-install-recommends \
    tini \
    ca-certificates \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# Copy the built binary from the builder stage
COPY --from=builder /app/overlay-mcp /usr/local/bin/overlay-mcp

# Set the entrypoint to run overlay-mcp via tini
ENTRYPOINT ["/usr/bin/tini", "--", "overlay-mcp"]
# CMD is removed as the command is now part of the entrypoint
