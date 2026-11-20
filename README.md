# Accents Rapides

[![CI](https://github.com/juanmaneo/accents-rapides/actions/workflows/ci.yml/badge.svg)](https://github.com/juanmaneo/accents-rapides/actions/workflows/ci.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue.svg)](LICENSE)

A Rust port of [schnelle-umlaute](https://github.com/Maik-0000FF/schnelle-umlaute): a Linux/FreeBSD Fcitx5 input method for quick accents. Hold a mapped key, then press the leader key (Alt by default) to cycle through its variants.

## Features

- Accent cycling, text snippets, and custom mappings
- Configurable leader key, profiles, and app allow/block lists
- Optional sorting by usage
- Per-window gesture state
- Direct text insertion through Fcitx5, without clipboard access

## Platform support

Linux and FreeBSD are supported. The overlay needs a Wayland compositor with `wlr-layer-shell` (including COSMIC, KDE Plasma, sway, and Hyprland); it does not work on X11 or GNOME/Mutter.

FreeBSD packages for the native addon must come from the `latest` branch. See [Native Fcitx5 addon](#native-fcitx5-addon).

## Requirements

- Rust via [rustup](https://rustup.rs/); the pinned toolchain and components are in `rust-toolchain.toml`.
- `libxkbcommon` development files:
  - Debian/Ubuntu: `sudo apt install libxkbcommon-dev`
  - FreeBSD: `pkg install libxkbcommon`
- A colour emoji font for emoji rendering and the UI tests:
  - Debian/Ubuntu: `sudo apt install fonts-noto-color-emoji`
  - FreeBSD: `pkg install noto-emoji`
- For the native Fcitx5 addon: g++ ≥ 16.1.0, clang++ ≥ 22.1.8, Fcitx5 ≥ 5.1.22 with development files, and `pkg-config`.

## Build and install

```bash
git clone https://github.com/juanmaneo/accents-rapides.git
cd accents-rapides
cargo build --workspace
cargo test --workspace
```

Build and install the Fcitx5 addon, then restart Fcitx5 and add **Accents Rapides** in `fcitx5-configtool`:

```bash
cargo xtask fcitx5 build
sudo ./target/debug/xtask fcitx5 install
fcitx5 -r
```

`install` only copies files; it does not build. To remove the addon:

```bash
sudo ./target/debug/xtask fcitx5 uninstall
```

Build and run the apps:

```bash
cargo build --release --workspace
./target/release/accents-rapides-editor
./target/release/accents-rapides-overlay
./target/release/accents-rapides config
```

## Use

With the input method active, hold a mapped key and press Alt. For example, `e` cycles through `é`, `è`, `ê`, and `ë`. Release the key without pressing Alt to type it normally. Alt shortcuts continue to work when no mapped gesture is active.

Default mappings:

| Hold | Press Alt | Variants |
|---|---|---|
| `a` | Alt | ä á à â |
| `c` | Alt | ç ć č |
| `e` | Alt | é è ê ë € |
| `i` | Alt | í ï î ì |
| `n` | Alt | ñ |
| `o` | Alt | ö ó ò ô |
| `s` | Alt | ß |
| `u` | Alt | ü ú ù û |

### Start the overlay at login

The overlay runs as your user, not as root. On COSMIC, KDE Plasma, or another desktop with XDG autostart, create `~/.config/autostart/accents-rapides-overlay.desktop`:

```ini
[Desktop Entry]
Type=Application
Name=Accents Rapides overlay
Exec=/home/you/.cargo/bin/accents-rapides-overlay
```

Use the binary's absolute path. For sway or Hyprland, add this to the compositor config instead:

```text
exec /home/you/.cargo/bin/accents-rapides-overlay          # sway
exec-once = /home/you/.cargo/bin/accents-rapides-overlay   # Hyprland
```

### Troubleshooting

If Right Alt does not trigger accents, it may be configured as the Compose key (`compose:ralt`), which sends `Multi_key` instead of `Alt_R`. Check with `setxkbmap -query`; move Compose to another key or clear that option in your keyboard settings.

## Configuration

`accents-rapides config` prints the paths. By default, files are in `~/.config/accents-rapides/`:

- `config.toml`: leader key, timing, overlay, app filter, and usage sorting.
- `mappings.toml`: profiles and mappings. Keys can be characters, `Shift+<character>`, or named keys such as `Space` and `LeftArrow`.
- `usage.toml`: per-variant usage counts, saved by Fcitx5. Delete it to reset the counts.

Example mapping:

```toml
current_profile = "Default"

[profiles.Default.mappings.e]
characters = ["é", "è", "ê"]
reverse_cycle = false
```

The editor (`accents-rapides editor`) saves config and mappings immediately. The addon reloads them when a window gains focus. A broken file is reported in the Fcitx5 log; the previous config remains active.

## Development

Run the apps from the workspace:

```bash
cargo run -- editor
cargo run -- overlay
cargo run -- config
```

Developer tasks:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask coverage       # enforce the 100% coverage ratchet
cargo xtask coverage --bump
cargo xtask audit
cargo xtask msrv
cargo xtask fcitx5 build
cargo xtask fcitx5 test
```

`cargo xtask` installs missing `cargo-llvm-cov`, `cargo-audit`, and `cargo-deny` tools automatically. `cargo xtask fcitx5 test` builds with both C++ compilers, runs the C ABI test under ASan/UBSan, and tests the addon inside Fcitx5. In containers where LeakSanitizer cannot use `ptrace`, run it with `ASAN_OPTIONS=detect_leaks=0`.

### Development container

The `Dockerfile` provides the pinned Rust toolchain, current C++ compilers, Fcitx5, system libraries, test fonts, and developer tools. It uses Arch Linux because the required Fcitx5 and compiler versions are not available on every distribution. Build with Docker, Podman, or Buildah:

```bash
docker build -t accents-rapides-dev .
```

Run a task with the source mounted and build/cache data in named volumes:

```bash
docker run --rm -it \
    --user "$(id -u):$(id -g)" \
    -v "$PWD:/workspace" \
    -v accents-rapides-cargo:/cargo \
    -v accents-rapides-target:/target \
    accents-rapides-dev cargo xtask fcitx5 test
```

For rootless Podman, omit `--user`; the container's root maps to your host user. Docker Desktop and Podman machine can build and test, but their Linux VM does not automatically connect GUI apps to the host desktop. The image is x86_64; on ARM, use `--platform linux/amd64` (emulated and slower).

#### Run GUI apps in COSMIC

Run from a terminal in your COSMIC Wayland session. Share the runtime directory and pass the session variables:

```bash
podman run --rm -it \
    -v "$PWD:/workspace" \
    -v "$XDG_RUNTIME_DIR:$XDG_RUNTIME_DIR" \
    -v accents-rapides-cargo:/cargo \
    -v accents-rapides-target:/target \
    -v accents-rapides-home:/home/dev \
    -e XDG_RUNTIME_DIR -e WAYLAND_DISPLAY \
    -e XDG_SESSION_TYPE=wayland \
    accents-rapides-dev
```

For Docker Engine on Linux, add `--user "$(id -u):$(id -g)"`. Then run `cargo run -- editor` or `cargo run -- overlay` in the container. Sharing the runtime directory also lets the host Fcitx5 addon feed the container's overlay; don't run a second overlay on the host. The mount exposes other session sockets, so only run trusted code.

## Roadmap

- [x] Native Fcitx5 addon (C++ shim calling the Rust C ABI)
- [x] One gesture state per window
- [x] Reload the configuration when its file changed
- [x] Overlay fed by the addon
- [x] Integration test of the addon inside a real Fcitx5
- [x] Mappings and profiles persisted and edited in the editor
- [x] Usage counts persisted across Fcitx5 restarts
- [x] FreeBSD support

## License

GPL-3.0-or-later. See [LICENSE](LICENSE) and [AUTHORS](AUTHORS).

## Acknowledgments

- Original [schnelle-umlaute](https://github.com/Maik-0000FF/schnelle-umlaute) by [Maik-0000FF](https://github.com/Maik-0000FF)
- [iced](https://iced.rs/) for the UI toolkit and [waycrate](https://github.com/waycrate) for `iced_layershell`
- [PowerAccent](https://github.com/damienleroy/PowerAccent) by Damien Leroy, the original inspiration

For Docker Engine on Linux, add `--user "$(id -u):$(id -g)"`. Then run `cargo run -- editor` or `cargo run -- overlay` in the container. Sharing the runtime directory