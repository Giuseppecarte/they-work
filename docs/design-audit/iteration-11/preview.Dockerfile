# Local preview recipe. Each platform uses its native musl compiler inside
# Docker; x86_64 execution on an ARM64 daemon is recorded as emulated.
FROM rust:1.90.0-slim-bookworm AS build
ARG TARGETARCH
RUN apt-get update \
 && apt-get install -y --no-install-recommends musl-tools \
 && rm -rf /var/lib/apt/lists/*
RUN case "$TARGETARCH" in \
      amd64) target=x86_64-unknown-linux-musl ;; \
      arm64) target=aarch64-unknown-linux-musl ;; \
      *) exit 1 ;; \
    esac \
 && rustup target add "$target" \
 && printf '%s' "$target" > /rust-target
# Use musl headers/compiler for bundled SQLite. Rust's self-contained target
# uses its normal C linker: Debian's musl-gcc linker specs unconditionally add
# a dynamic interpreter even for a static PIE.
ENV CC=musl-gcc
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates/theywork-core/Cargo.toml crates/theywork-core/
COPY crates/theywork-collect/Cargo.toml crates/theywork-collect/
COPY crates/theywork-control/Cargo.toml crates/theywork-control/
COPY crates/theywork-terminal-image/Cargo.toml crates/theywork-terminal-image/
COPY crates/theywork-render/Cargo.toml crates/theywork-render/
COPY crates/theywork-tui/Cargo.toml crates/theywork-tui/
RUN for c in core collect control terminal-image render tui; do \
      mkdir -p "crates/theywork-$c/src" \
      && printf '' > "crates/theywork-$c/src/lib.rs"; \
    done \
 && printf 'fn main() {}' > crates/theywork-tui/src/main.rs \
 && mkdir -p crates/theywork-control/tests \
 && printf 'fn main() {}' > crates/theywork-control/tests/storage_process.rs \
 && cargo build --release --locked --target "$(cat /rust-target)" --bin they-work
COPY crates crates
RUN find crates -name '*.rs' -exec touch {} + \
 && cargo build --release --locked --target "$(cat /rust-target)" --bin they-work \
 && cp "target/$(cat /rust-target)/release/they-work" /they-work \
 && rustc -Vv > /toolchain.txt \
 && musl-gcc -v >> /toolchain.txt 2>&1 \
 && dpkg-query -W musl musl-dev musl-tools >> /toolchain.txt

FROM debian:bookworm-slim AS runtime
COPY --from=build /they-work /usr/local/bin/they-work
COPY --from=build /toolchain.txt /toolchain.txt
USER 10001:10001
ENV LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    LC_CTYPE=C.UTF-8 \
    TERM=xterm-256color \
    HOME=/nonexistent \
    XDG_CONFIG_HOME=/nonexistent \
    THEYWORK_CLAUDE_HOME=/nonexistent/claude \
    THEYWORK_CODEX_HOME=/nonexistent/codex
ENTRYPOINT ["/usr/local/bin/they-work"]
