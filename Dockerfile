# syntax=docker/dockerfile:1
#
# ara hub image — multi-stage musl → distroless (Stage 5, D2 embedded-only).
#
# The viewer bundle is already committed under crates/ara-cli/assets/viewer/ and
# baked into the binary at compile time via `include_dir!`, so the builder is a
# plain `cargo build --target x86_64-unknown-linux-musl -p ara-cli` that consumes
# the committed bytes. There is intentionally NO wasm toolchain (trunk /
# wasm-bindgen / wasm32 target) and NO embed-viewer.sh in the image: the regen
# happens on the dev machine and is enforced by the `viewer-embed-fresh` CI gate,
# and a wasm rebuild here would not be cargo-chef-cacheable.
#
# cargo-chef caches the Rust dependency compile as its own layer so app-only
# edits don't recompile the whole dependency tree.
#
# Result: a static musl binary on distroless — no shell, no libc, runs nonroot.
# The viewer wasm is served UNCOMPRESSED on the wire (embedded assets are not
# content-negotiated); compression is the reverse proxy's job (see docs/deploy.md).

# ── Chef base ────────────────────────────────────────────────────────────────
FROM rust:1-bookworm AS chef
WORKDIR /app
# Pin to the repo toolchain BEFORE adding the musl target so the target lands on
# the exact channel every stage uses (rust-toolchain.toml activates it). Without
# this, `rustup target add` would install onto the base image's default channel
# while the later `COPY . .` switches to the pinned one — leaving the pinned
# toolchain without the musl target.
COPY rust-toolchain.toml .
RUN cargo install cargo-chef --locked \
    && rustup target add x86_64-unknown-linux-musl \
    && apt-get update \
    && apt-get install -y --no-install-recommends musl-tools \
    && rm -rf /var/lib/apt/lists/*
# Point the musl target's C compiler at musl-gcc so any `*-sys` build script
# (which shells out to `cc`) doesn't fall back to host gcc and pass `-m64`, which
# musl-gcc rejects. Do NOT override the *linker* to musl-gcc: musl-gcc links
# dynamically by default (against /lib/ld-musl-*.so.1, absent from distroless),
# yielding an `exec /ara: no such file or directory`. rustc's musl target links
# fully static via its self-contained startup objects, so leave the linker
# alone and pin +crt-static to be explicit.
ENV CC_x86_64_unknown_linux_musl=musl-gcc \
    RUSTFLAGS="-C target-feature=+crt-static"

# ── Plan: capture the dependency graph for caching ───────────────────────────
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ── Build: cook deps (cached), then build the binary ─────────────────────────
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Compile dependencies as a standalone, cacheable layer. `--locked` matches CI:
# the committed Cargo.lock is copied in, so the image compiles the exact
# dependency versions CI tested rather than silently drifting the lockfile.
RUN cargo chef cook --release --locked --target x86_64-unknown-linux-musl --recipe-path recipe.json
COPY . .
RUN cargo build --release --locked --target x86_64-unknown-linux-musl -p ara-cli \
    && cp target/x86_64-unknown-linux-musl/release/ara /app/ara

# ── Runtime: distroless static, nonroot ──────────────────────────────────────
FROM gcr.io/distroless/static-debian12:nonroot AS runtime
COPY --from=builder /app/ara /ara
# ARAs are bind-mounted at /aras; bind 0.0.0.0 so the port is reachable from the
# host (the binary defaults to loopback, which is unreachable in a container).
EXPOSE 8080
ENTRYPOINT ["/ara"]
CMD ["serve", "--hub", "--ara-root", "/aras", "--host", "0.0.0.0", "--port", "8080"]
