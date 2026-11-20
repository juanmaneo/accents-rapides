# Development environment for accents-rapides: the Rust toolchain pinned in
# rust-toolchain.toml, the C++ toolchain and Fcitx5 of the native addon, and the
# tools behind `cargo xtask` (llvm-cov, audit, deny). The C++ toolchain and
# Fcitx5 meet the minimums of [workspace.metadata.cpp] in Cargo.toml; the fcitx5
# job of .github/workflows/ci.yml runs on Arch, Ubuntu 26.04 and Fedora.
#
# The same file builds with Docker, Podman and Buildah: plain instructions only
# (no BuildKit syntax) and a fully qualified base image, since Podman refuses
# short names when it cannot prompt.
#
# The source tree is not copied in, it is mounted at /workspace; the build
# context is just rust-toolchain.toml (see .dockerignore). Usage: README.md,
# "Development container".

# Ubuntu 26.04 because its g++ 15.2, clang++ 21.1.8 and Fcitx5 5.1.19 are the
# minimums of [workspace.metadata.cpp]. Ubuntu does not package cargo-audit,
# cargo-deny or cargo-llvm-cov, so they are built below.
FROM docker.io/library/ubuntu:26.04

LABEL org.opencontainers.image.title="accents-rapides development environment" \
      org.opencontainers.image.source="https://github.com/juanmaneo/accents-rapides" \
      org.opencontainers.image.licenses="GPL-3.0-or-later"

# Where things live. Every directory that a build writes to is writable by any
# uid, so the container works as root (rootless Podman and Docker Desktop, where
# that is you) and as `--user "$(id -u):$(id -g)"` (Docker on Linux, so that the
# files it creates in the source tree stay yours).
# CARGO_TARGET_DIR keeps the build output out of the source tree: the host's own
# target/ is never touched, and a volume mounted there is much faster than a
# bind mount on macOS and Windows. Run with `-e CARGO_TARGET_DIR=target` to
# build in the source tree instead.
ENV RUSTUP_HOME=/rustup \
    CARGO_HOME=/cargo \
    CARGO_TARGET_DIR=/target \
    HOME=/home/dev \
    PATH=/cargo/bin:$PATH

# Compilers and Fcitx5 (its headers and the testing addons that
# `cargo xtask fcitx5 test` needs are in the fcitx5 packages), libxkbcommon (the
# only system library the Rust build needs), a colour emoji font and a text font
# (the editor and overlay tests render both), and rustup, which installs the
# toolchain pinned in rust-toolchain.toml.
RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y \
        build-essential clang pkgconf git rustup ca-certificates \
        libxkbcommon-dev fcitx5 libfcitx5core-dev fcitx5-modules-dev \
        fonts-noto-color-emoji fonts-dejavu-core \
    && rm -rf /var/lib/apt/lists/*

# The toolchain, components and profile that rust-toolchain.toml pins, made the
# default so that cargo also works outside the source tree. `rustup show` reads
# the pin and installs it, since Ubuntu's rustup 1.27 needs arguments for
# `rustup toolchain install`. If the pin changes,
# rebuild the image: rustup would install the new toolchain by itself, but on
# every run, since containers are thrown away. The cargo tools are built in the
# same step, and their download caches removed, so that the layer stays small.
# The permissions are set in this step too, or the layer would hold a second
# copy of the whole toolchain.
COPY rust-toolchain.toml /tmp/toolchain/
RUN (cd /tmp/toolchain \
        && rustup show \
        && rustup default "$(rustup show active-toolchain | cut -d' ' -f1)") \
    && rm -rf /tmp/toolchain \
    && cargo install --locked cargo-llvm-cov cargo-audit cargo-deny \
    && rm -rf /cargo/registry /cargo/git \
    && mkdir -p /cargo /target /home/dev /workspace \
    && chmod -R a+rwX /rustup /cargo /target /home/dev \
    && git config --system --add safe.directory /workspace

WORKDIR /workspace
CMD ["bash"]
