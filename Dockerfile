# Use the official Rust image as a parent image
FROM rust:slim-buster as builder

EXPOSE 8080

ENV PORT 8080

# Install build dependencies
RUN apt-get update && \
  apt-get install -y pkg-config librust-openssl-dev libssl-dev build-essential g++ && \
  rm -rf /var/lib/apt/lists/*

# Set the working directory in the container
WORKDIR /usr/src/felafax-proxy

# Copy the Cargo.toml and Cargo.lock files
COPY Cargo.toml Cargo.lock ./

# Copy the source code
COPY src ./src

# Build the application
RUN cargo build --release

# Start a new stage for a smaller final image
FROM rust:slim-buster

# Install runtime dependencies
RUN apt-get update && \
  apt-get install -y ca-certificates libssl1.1 && \
  rm -rf /var/lib/apt/lists/*

# Copy the binary from the builder stage
COPY --from=builder /usr/src/felafax-proxy/target/release/felafax-proxy /usr/local/bin/felafax-proxy

# Verify the binary exists and is executable
RUN ls -l /usr/local/bin/felafax-proxy && \
  chmod +x /usr/local/bin/felafax-proxy

# Copy required config files
COPY .env .
COPY firebase.json .

# Set the startup command
CMD ["felafax-proxy"]
