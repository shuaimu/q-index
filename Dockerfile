# Build stage
FROM rust:1.75-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Build dependencies (this is cached as long as Cargo.toml doesn't change)
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy source code
COPY src ./src

# Build the application
RUN touch src/main.rs && \
    cargo build --release

# Runtime stage
FROM alpine:latest

# Install runtime dependencies
RUN apk add --no-cache libgcc

WORKDIR /app

# Copy the binary from builder
COPY --from=builder /app/target/release/qindex /usr/local/bin/qindex

# Copy bibliography data
COPY bib ./bib

# Set entrypoint
ENTRYPOINT ["qindex"]

# Default command
CMD ["calculate"]