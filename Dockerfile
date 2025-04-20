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
RUN mkdir src && echo 'fn main() {println!("Invalid binary copied");}' > src/main.rs && \
    export RUST_TARGET=$(cat /rust_target.txt) && \
    echo "Building dependencies for target: $RUST_TARGET" && \
    # Set linker for cross-compilation if needed
    case "$RUST_TARGET" in \
      "aarch64-unknown-linux-gnu") export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc ;; \
      "armv7-unknown-linux-gnueabihf") export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc ;; \
      "arm-unknown-linux-gnueabihf") export CARGO_TARGET_ARM_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc ;; \
    esac && \
    # Build dependencies only (no need to produce the dummy binary)
    cargo build --release --target $RUST_TARGET && \
    # Clean up intermediate dummy binary if it was created (optional but safer)
    rm -f target/$RUST_TARGET/release/overlay-mcp target/$RUST_TARGET/release/deps/overlay_mcp* && \
    rm -rf src # 임시 src 디렉토리 삭제

# 3. 실제 소스 코드 복사
COPY src ./src

# 4. 최종 빌드 (의존성 캐시 활용)
#    - cargo build 실행
RUN export RUST_TARGET=$(cat /rust_target.txt) && \
    echo "Building application for target: $RUST_TARGET" && \
    # Set linker for cross-compilation if needed
    case "$RUST_TARGET" in \
      "aarch64-unknown-linux-gnu") export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc ;; \
      "armv7-unknown-linux-gnueabihf") export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc ;; \
      "arm-unknown-linux-gnueabihf") export CARGO_TARGET_ARM_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc ;; \
    esac && \
    cargo build --release --target $RUST_TARGET && \
    # Move the final binary to a known location
    mv target/$RUST_TARGET/release/overlay-mcp /app/overlay-mcp

# --- 의존성 캐싱을 위한 변경 끝 ---

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

# Copy from the known location in the builder stage
COPY --from=builder /app/overlay-mcp /usr/local/bin/overlay-mcp

# Set the entrypoint to run overlay-mcp via tini
ENTRYPOINT ["/usr/bin/tini", "--", "overlay-mcp"]
# CMD is removed as the command is now part of the entrypoint
