#!/bin/sh
# Builds pkg/ inside a pinned container, because the artifact is committed and published.
#
# The same source built with the same rustc produces a different .wasm on macOS than on Linux —
# the two toolchains ship separately compiled wasm32 std, so the builds pull in different std
# functions and the binary differs by a handful of bytes. Neither is wrong, but only one can be
# the committed one, so the build environment is part of the artifact's definition.
#
# Pinned by digest rather than tag: `rust:1.97.1` is mutable and could be re-pushed.
set -e

IMAGE="rust@sha256:b1b3c9c0d921d7fa0a6d1f9ec7e4eab87f8c8ec97644c3d791450f131dec813f"
WASM_PACK_VERSION="0.15.0"

command -v docker >/dev/null || {
  echo "docker is required to build the committed artifact — see README, 'Building'." >&2
  echo "For local iteration where the bytes do not matter, use: npm run build:native" >&2
  exit 1
}

# A named volume keeps the crates registry, the wasm-opt download and the build tree across runs.
#
# CARGO_TARGET_DIR points into that volume rather than the bind mount on purpose. The container
# runs as root — `rustup target add` writes into the image — so anything it leaves in the working
# tree is root-owned on Linux, where bind mounts preserve real UIDs. A root-owned `target/` then
# blocks the host's own `cargo test`, which is the next thing `npm run verify` does. macOS hides
# this: Docker Desktop maps ownership back to the calling user.
#
# `pkg/` still has to be written through the bind mount, so it is chowned back afterwards.
docker run --rm \
  --platform linux/amd64 \
  -v "$PWD:/work" -w /work \
  -v normalize-svg-build-cache:/cache \
  -e CARGO_HOME=/cache/cargo \
  -e CARGO_TARGET_DIR=/cache/target \
  -e XDG_CACHE_HOME=/cache/xdg \
  -e WASM_PACK_VERSION="$WASM_PACK_VERSION" \
  -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
  "$IMAGE" sh -c '
    set -e
    export PATH="$CARGO_HOME/bin:$PATH"
    if [ ! -x "$CARGO_HOME/bin/wasm-pack" ]; then
      rustup target add wasm32-unknown-unknown >/dev/null
      mkdir -p "$CARGO_HOME/bin"
      curl -sSL "https://github.com/rustwasm/wasm-pack/releases/download/v${WASM_PACK_VERSION}/wasm-pack-v${WASM_PACK_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
        | tar xz -C /tmp
      install "/tmp/wasm-pack-v${WASM_PACK_VERSION}-x86_64-unknown-linux-musl/wasm-pack" "$CARGO_HOME/bin/"
    fi

    # Repeats the 8 MB stack flag because RUSTFLAGS overrides .cargo/config.toml wholesale, and
    # remaps build paths so the binary does not embed this container’s layout. Keep in sync with
    # the build:native script in package.json.
    RUSTFLAGS="-C link-arg=-zstack-size=8388608 \
      --remap-path-prefix=${CARGO_HOME}=/cargo \
      --remap-path-prefix=$(rustc --print sysroot)=/rust" \
      wasm-pack build . --release --target nodejs --out-dir pkg --out-name normalize_svg

    # wasm-pack scatters these into pkg/; this repo commits pkg/ deliberately.
    rm -f pkg/.gitignore pkg/LICENSE pkg/README.md

    # Give pkg/ back to the caller, or the next host-side write to it is denied. Ignored on
    # macOS, where the bind mount already presents files as the calling user.
    chown -R "$HOST_UID:$HOST_GID" pkg 2>/dev/null || true
  '
