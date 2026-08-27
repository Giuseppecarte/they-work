# Build stage: the Rust toolchain never leaves this layer.
FROM rust:1.90-slim-bookworm AS build

# rusqlite's bundled SQLite needs a C compiler.
RUN apt-get update \
 && apt-get install -y --no-install-recommends gcc libc6-dev \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /src

# Cache dependencies separately from source, so editing code does not refetch
# the registry on every build.
COPY Cargo.toml Cargo.lock* rust-toolchain.toml ./
COPY crates/theywork-core/Cargo.toml    crates/theywork-core/
COPY crates/theywork-collect/Cargo.toml crates/theywork-collect/
COPY crates/theywork-render/Cargo.toml  crates/theywork-render/
COPY crates/theywork-tui/Cargo.toml     crates/theywork-tui/
RUN for c in core collect render tui; do \
      mkdir -p "crates/theywork-$c/src" && \
      echo "" > "crates/theywork-$c/src/lib.rs"; \
    done \
 && mkdir -p crates/theywork-tui/src \
 && echo "fn main() {}" > crates/theywork-tui/src/main.rs \
 && cargo build --release --locked 2>/dev/null || cargo build --release

COPY . .
# Touch sources so cargo rebuilds them over the dependency-cache stubs.
RUN find crates -name '*.rs' -exec touch {} + \
 && cargo build --release --bin they-work

# Runtime stage: just the binary and a non-root user.
FROM debian:bookworm-slim AS runtime
RUN useradd --create-home --uid 10001 watcher
COPY --from=build /src/target/release/they-work /usr/local/bin/they-work
USER watcher
ENV TERM=xterm-256color \
    THEYWORK_CLAUDE_HOME=/data/claude \
    THEYWORK_CODEX_HOME=/data/codex
ENTRYPOINT ["/usr/local/bin/they-work"]
