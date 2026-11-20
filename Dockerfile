# Development environment for accents-rapides: the Rust toolchain pinned in
# rust-toolchain.toml, the C++ toolchain and Fcitx5 of the native addon, and the
# tools behind `cargo xtask` (llvm-cov, audit, deny). It is the environment of
# the `fcitx5` job of .github/workflows/ci.yml, so what passes here passes there.
#
# The same file builds with Docker, Podman and Buildah: plain instructions only
# (no BuildKit syntax) and a fully qualified base image, since Podman refuses
# short names when it cannot prompt.
#
# The source tree is not copied in, it is mounted at /workspace; the build
# context is just rust-toolchain.toml (see .dockerignore). Usage: README.md,
# "Development container".

# Arch Linux because the addon needs g++ >= 16, clang++ >= 21 and Fcitx5 >=
# 5.1.22 (see [workspace.metadata.cpp] in Cargo.toml), which other distributions
# ship late or never. It is a rolling release: rebuild with `--pull` to update.
# The official image is x86_64 only: on an ARM host, build and run with
# `--platform linux/amd64` (emulated, so slow).
FROM docker.io/library/archlinux:latest

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
# `cargo xtask fcitx5 test` needs are in the fcitx5 package), libxkbcommon (the
# only system library the Rust build needs), a colour emoji font and a text font
# (the editor and overlay tests render both), and the cargo tools prebuilt,
# instead of compiled by `cargo xtask` on first use.
RUN pacman -Syu --noconfirm --needed \
        base-devel clang fcitx5 pkgconf git rustup \
        libxkbcommon noto-fonts-emoji ttf-dejavu \
        cargo-llvm-cov cargo-audit cargo-deny \
    && rm -rf /var/cache/pacman/pkg/*

# The toolchain, components and profile that rust-toolchain.toml pins, made the
# default so that cargo also works outside the source tree. If the pin changes,
# rebuild the image: rustup would install the new toolchain by itself, but on
# every run, since containers are thrown away. The permissions are set in this
# same step, or the layer would hold a second copy of the whole toolchain.
COPY rust-toolchain.toml /tmp/toolchain/
RUN (cd /tmp/toolchain \
        && rustup toolchain install \
        && rustup default "$(rustup show active-toolchain | cut -d' ' -f1)") \
    && rm -rf /tmp/toolchain \
    && mkdir -p /cargo /target /home/dev /workspace \
    && chmod -R a+rwX /rustup /cargo /target /home/dev \
    && git config --system --add safe.directory /workspace

WORKDIR /workspace
CMD ["bash"]
