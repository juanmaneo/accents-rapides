# Accents Rapides

Accents Rapides is a Rust port of [schnelle-umlaute](https://github.com/Maik-0000FF/schnelle-umlaute), a keyboard utility that provides quick access to special characters and accents (a Linux alternative to Windows PowerToys Quick Accent).

## About

This project is a Rust port of the original [schnelle-umlaute](https://github.com/Maik-0000FF/schnelle-umlaute) C++/Qt Fcitx5 addon, initially created with assistance from Mistral AI.

The implementation uses:
- **[Rust](https://www.rust-lang.org/)** for the core functionality (key mapping, accent cycling, configuration) and system integration
- **[iced](https://iced.rs/)** for the editor and the cycle overlay (replacing the original Qt/QML editor and LayerShellQt overlay). The overlay is a `wlr-layer-shell` surface via [`iced_layershell`](https://github.com/waycrate/exwlshelleventloop)

## Features

- Hold a mapped key, then press the leader key (default: Space) to type an accent character
- Accent cycling: press the leader key repeatedly to cycle through variants (é → è → ê → ë), forward or backward
- Sort variants by usage: optionally order each key's cycle by how often you use them
- Snippets: map single keys to entire text phrases
- Configurable leader keys (Space, arrow keys, Alt, AltGr, or custom keys)
- Profiles with multiple mapping sets
- App blacklist/whitelist to disable the input method in games, password managers, etc.
- Direct text insertion via Fcitx5 `commitString()` - no clipboard interference

## Platform Support

**Linux** and **FreeBSD** are supported (the original is a Fcitx5 addon, and
Fcitx5 runs on both). Other platforms (Windows, macOS) may come later if someone
ever wants to contribute on ports.

On FreeBSD everything builds and passes its tests, the native addon included
(verified on FreeBSD 15.1, and in CI). Two differences come from FreeBSD's
packages: the Fcitx5 and LLVM versions we need are in the `latest` package
branch, not `quarterly`, and the Fcitx5 package is built with clang and libc++,
so both compilers build the addon against the system's libc++ (see
[Native Fcitx5 addon](#native-fcitx5-addon)).

## Project Structure

```
accents-rapides/
├── Cargo.toml              # Workspace root + CLI binary
├── src/main.rs             # CLI entry point (editor/overlay/config subcommands)
├── crates/
│   ├── core/               # Core logic: mappings, cycling, the gesture engine, config
│   ├── fcitx5/             # Fcitx5 integration: the C ABI (Rust staticlib) and the native addon
│   │   ├── cpp/            # C++23 addon shim, ABI header, ABI and integration tests (built by `cargo xtask fcitx5`)
│   │   └── data/           # Fcitx5 addon and input method registration files
│   ├── editor/             # iced app for managing mappings and settings
│   ├── overlay/            # iced layer-shell surface showing the accent cycle
│   └── test-support/       # Shared test helpers (temporary config and overlay socket), not published
├── xtask/                  # `cargo xtask` developer tasks (coverage, audit, MSRV, C++ addon build)
├── coverage-ratchet.json   # Coverage gate enforced by `cargo xtask coverage`
├── deny.toml               # cargo-deny policy used by `cargo xtask audit`
├── rust-toolchain.toml     # Pinned Rust version used locally and in CI
├── .github/workflows/ci.yml
├── AUTHORS
└── LICENSE                 # GNU GPL version 3
```

## Requirements

- Rust via [rustup](https://rustup.rs/): the exact version and components are pinned in
  `rust-toolchain.toml`. Running `cargo` inside the repository uses that version automatically
  (the `cargo` that rustup installs is a proxy that reads this file and installs the toolchain if needed)
- `libxkbcommon` development files (the only system library needed to build).
  On Debian/Ubuntu: `sudo apt install libxkbcommon-dev`. On FreeBSD:
  `pkg install libxkbcommon` (packages include their headers). Wayland, X11 and Vulkan are loaded at runtime.
- For the overlay: a Wayland compositor with `wlr-layer-shell` (KDE Plasma, sway, Hyprland, COSMIC, ...).
  As in the original, GNOME/Mutter and X11 cannot host it; everything else works there.
  The overlay renders on the GPU (wgpu) so that its background can be transparent, and falls
  back to the CPU renderer when no GPU is available.

For the native Fcitx5 addon, additionally:

- **g++ ≥ 16.1.0** and **clang++ ≥ 22.1.8**: both are required, the addon is built and tested
  with each because they report different mistakes
- **Fcitx5 ≥ 5.1.22** with its development files (headers and `Fcitx5Core.pc`), and `pkg-config`

On FreeBSD, use the `latest` package branch (`quarterly` is older than these minimums;
set `url: "pkg+https://pkg.FreeBSD.org/${ABI}/latest"` in
`/usr/local/etc/pkg/repos/FreeBSD.conf`) and install the packages
`fcitx5 llvm22 gcc16 pkgconf`. They provide the compilers as `clang++22` and `g++16`: the
FreeBSD base system's own `clang++` is older than the minimum, and `cargo xtask fcitx5`
picks the highest versioned compiler it finds in `PATH`.

These are minimum versions, the ones the addon was tested with: newer releases are fine. Their
source of truth is `[workspace.metadata.cpp]` in the root `Cargo.toml`, and `cargo xtask fcitx5`
checks them before building.

## Building

```bash
# Clone the repository
git clone https://github.com/juanmaneo/accents-rapides.git
cd accents-rapides

# Build everything
cargo build

# Run the test suite
cargo test --workspace
```

## Usage

```bash
# Build the Fcitx5 addon, then install it system-wide (installing builds nothing,
# so only the copy runs as root; use `doas` instead of `sudo` if you prefer).
# The released build is installed, see below.
cargo xtask fcitx5 build
sudo ./target/debug/xtask fcitx5 install
fcitx5 -r   # then add "Accents Rapides" as an input method in fcitx5-configtool

# Remove it again
sudo ./target/debug/xtask fcitx5 uninstall

# Launch the mapping/profile editor
cargo run -- editor

# Launch the cycle overlay (keep it running next to Fcitx5, e.g. from your
# compositor's autostart): the addon tells it what to show
cargo run -- overlay

# Show configuration info
cargo run -- config
```

With the Accents Rapides input method active:
- Hold a mapped key (e.g. `a`), then press <kbd>Space</kbd> to insert `ä`
- Press the leader key repeatedly to cycle through variants (e.g. `e` → é → è → ê);
  while you cycle, the overlay shows every variant with the current one highlighted
- Release the key without pressing the leader to type the normal letter

Each window keeps its own gesture: a key held in one window never commits in
another.

### Default Mappings

| Hold | + | Press | = | Result |
|------|---|-------|---|--------|
| <kbd>a</kbd> | + | <kbd>Space</kbd> | = | ä |
| <kbd>o</kbd> | + | <kbd>Space</kbd> | = | ö |
| <kbd>u</kbd> | + | <kbd>Space</kbd> | = | ü |
| <kbd>s</kbd> | + | <kbd>Space</kbd> | = | ß |
| <kbd>Shift</kbd>+<kbd>a</kbd> | + | <kbd>Space</kbd> | = | Ä |
| <kbd>Shift</kbd>+<kbd>o</kbd> | + | <kbd>Space</kbd> | = | Ö |
| <kbd>Shift</kbd>+<kbd>u</kbd> | + | <kbd>Space</kbd> | = | Ü |

All mappings are fully customizable: add French, Spanish, emoji, Braille, or any Unicode character.

## Configuration

Three files live in `~/.config/accents-rapides/` (`accents-rapides config` prints their
paths). The first two are created with defaults on first run:

- `config.toml`: the leader key and time window, whether to show the overlay, the app
  filter (blacklist/whitelist) mode and entries, and whether to sort variants by usage
- `mappings.toml`: the profiles, their mappings, and which profile is current:

  ```toml
  current_profile = "Default"

  [profiles.Default.mappings.a]
  characters = ["ä"]
  reverse_cycle = false

  [profiles.Default.mappings."Shift+A"]
  characters = ["Ä"]
  reverse_cycle = false
  ```

  Keys are written as `a`, `Shift+A` or a named key (`Space`, `LeftArrow`, `RightArrow`,
  `UpArrow`, `DownArrow`); `characters` lists the variants in cycling order.

- `usage.toml`: how many times each variant was typed, written by the addon after every
  variant typed. With "sort variants by usage" on, each key's cycle starts with its most
  used variant (ties keep the `mappings.toml` order), from the moment Fcitx5 starts:

  ```toml
  [e]
  "è" = 1
  "é" = 5
  ```

  Delete the file to start the counts over. A broken one is reported in the Fcitx5 log
  and replaced by the next variant typed.

The editor (`accents-rapides editor`) edits the first two files and saves every change
right away. Its tabs:

- **Profiles**: add profiles, use one (make it current) or delete the others
- **Mappings**: the mappings of the current profile. Variants are typed comma-separated
  (`é,è,ê`; a literal comma as `,,`), with a reverse-cycling toggle per mapping
- **Settings**: everything in `config.toml`

No restart is needed after editing them, with the editor or by hand: the addon checks
both files' modification times whenever a window gains focus, and reloads them if either
changed (a broken file is reported in the Fcitx5 log and the previous configuration
stays).

## Development

Developer tasks run through the `cargo xtask` pattern (see `.cargo/config.toml`):

```bash
# Measure test coverage and enforce the coverage ratchet
cargo xtask coverage

# Raise the ratchet to the measured coverage (it only ever goes up)
cargo xtask coverage --bump

# Security audit of the dependency tree (cargo-audit + cargo-deny)
cargo xtask audit

# Check the workspace with the minimum supported Rust version
cargo xtask msrv

# Native Fcitx5 addon: xtask calls g++ and clang++ itself (no Makefile, no CMake)
cargo xtask fcitx5 build      # target/fcitx5/{g++,clang++}/accents-rapides.so
cargo xtask fcitx5 test       # build, then run the C ABI test (ASan + UBSan) and the test inside Fcitx5
cargo xtask fcitx5 install    # [--destdir DIR] [--compiler g++|clang++], default: see below
cargo xtask fcitx5 uninstall  # [--destdir DIR]
```

The tools each task needs (`cargo-llvm-cov`, `cargo-audit`, `cargo-deny`) are
installed automatically with `cargo install --locked` when missing.

### Coverage

Coverage uses [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) over
the whole workspace, excluding the developer tooling (`xtask` and
`test-support`). The ratchet gate is stored in `coverage-ratchet.json` and
enforced on every `cargo xtask coverage` run: if measured coverage falls below
the gate, the command fails. The gate is currently at **100%** - full coverage of
all files, lines, regions and functions - and must stay there.

### Security audit

This tool sits between the keyboard and every application, so dependencies are
audited on every CI run and weekly:

- [`cargo audit`](https://github.com/rustsec/rustsec/tree/main/cargo-audit) fails on
  known vulnerabilities in `Cargo.lock` (RustSec database).
- [`cargo deny check`](https://github.com/EmbarkStudios/cargo-deny) additionally
  rejects unmaintained and unsound crates, licenses incompatible with
  GPL-3.0-or-later, and dependencies from anything other than crates.io.

Accepted advisories are listed **only** in `deny.toml`, each with a reason;
`cargo xtask audit` forwards their ids to `cargo audit`. Currently both come from
iced 0.14's text rendering (`lru`, `ttf-parser`) and should be removed with the
next iced release. Both tools download the advisory database, so the audit needs
network access.

### Rust versions

Two versions, kept coherent by a test in `xtask`:

- `rust-toolchain.toml` pins the **development toolchain** used locally and in
  CI, so clippy lints and coverage counts don't change under our feet.
- `rust-version` in the root `Cargo.toml` is the **MSRV** we promise to users
  building from source. It's the floor set by our dependencies and must never be
  newer than the pinned toolchain. `cargo xtask msrv` (and CI) checks that the
  workspace really builds with it.

### Native Fcitx5 addon

Fcitx5 addons are C++ shared libraries, so `crates/fcitx5/cpp/addon.cpp` is a
thin C++23 engine: it translates each Fcitx5 key event for the Rust gesture
engine (`accents_rapides_core::InputHandler`) through the C ABI declared in
`cpp/accents_rapides.hpp`, and applies the returned commit/preedit/forward
action. It holds no logic of its own. Key translation tables are `constexpr`
and checked at compile time with `static_assert`, so the per-key work at run
time is a table lookup and one call into Rust.

The addon state behind the C ABI has three parts:

- **One gesture state per window.** Configuration and mappings are shared, but
  what is held, shown and pending lives in an `accents_rapides_core::Session`.
  The C++ side attaches one to every Fcitx5 input context (an
  `InputContextProperty`), so it is created and freed with its window.
- **Reload on focus.** Activating a window calls `fcitx_accents_rapides_refresh`,
  which reloads the configuration only if its file's modification time changed
  since it was loaded. Fcitx5's own "reload configuration" still forces a reload.
- **The overlay feed.** While a key cycles through its variants, the addon sends
  the overlay `Show { variants, index }`, and `Hide` when the cycle ends (merely
  holding a mapped key shows nothing, or every letter would flash). Messages
  are JSON datagrams on a Unix socket, `$XDG_RUNTIME_DIR/accents-rapides-overlay.sock`
  (override it with `ACCENTS_RAPIDES_OVERLAY_SOCKET`). Only changes are sent,
  without blocking, and ignoring errors: the keyboard works the same whether
  an overlay runs or not. The overlay binds the socket, replacing one that a
  previous overlay left behind.

There is no Makefile or CMake project: `cargo xtask fcitx5` is the build
system of the C++ side. It is plain Rust code (`xtask/src/fcitx5.rs`) that runs
`pkg-config`, `g++` and `clang++` directly and prints each command line it runs,
so the exact flags are visible in its output. Everything that depends on the
operating system (linker flags, the system libraries the Rust staticlib needs,
how compiler executables are named, which C++ standard library to use and
which build is released) sits behind the `Platform` trait in
`xtask/src/fcitx5/platform.rs`, with one module per system:
`platform/linux.rs` and `platform/freebsd.rs` (where packages install
versioned compiler names like `g++16` and `clang++22`). Optional hardening
flags are probed per compiler.

`cargo xtask fcitx5 build` builds the Rust part as a staticlib, then compiles
and links the addon with every compiler, with warnings as errors, hardening
flags (`_FORTIFY_SOURCE=3`, stack protector, full RELRO) and only the Fcitx5
factory symbol exported. `cargo xtask fcitx5 test` then runs two tests, each
built by each compiler:

- `cpp/abi_test.cpp` checks both sides of the C ABI against each other. It
  links the Rust staticlib alone, so it runs under AddressSanitizer and
  UndefinedBehaviorSanitizer. LeakSanitizer needs `ptrace`: where that is
  blocked (some sandboxes and containers), run it with
  `ASAN_OPTIONS=detect_leaks=0`.
- `cpp/fcitx_test.cpp` starts a real Fcitx5 (`fcitx::Instance`) that loads the
  addon from an installed layout under `target/`, and drives it through
  Fcitx5's own `testfrontend` addon the way applications do: gestures, Shift,
  per-window state, what happens to a pending key on an input method switch,
  focus change or reset, and the reload of a configuration saved meanwhile.
  Only the test addons and ours are enabled, from the given directories, so a
  system-wide installation stays out of it. The addon is loaded as built, so
  there are no sanitizers here.

Fcitx5's API passes standard library types, so every build uses the system's
C++ standard library, the one Fcitx5 was built with, and both builds pass both
tests on both systems. **The released `.so`**, the one `cargo xtask fcitx5
install` copies by default, is the build of the compiler the system builds its
packages with:

- **Linux**: the g++ build (`target/fcitx5/g++/accents-rapides.so`). Distributions
  build Fcitx5 with GCC and libstdc++, which clang++ also uses there. The clang++
  build is a check, installed only with `--compiler clang++`.
- **FreeBSD**: the clang++ build (`target/fcitx5/clang++/accents-rapides.so`). The
  Fcitx5 package is built with the base system's clang and libc++. `g++` builds
  against the same libc++ (`-stdlib=libc++`, which the gcc ports support); its
  build is the check there, installed only with `--compiler g++`. With libc++,
  GCC reports false `-Wstringop-overflow` and `-Warray-bounds` positives inside
  libc++'s `<format>` (a one-line `std::println("{}", 1)` at `-O2` triggers
  them), so these two warnings are off for g++ on FreeBSD only.

The gesture engine port leaves out some workarounds of the original: deferred
commits for Alt-led gestures under KWin Wayland auto-repeat, the dual
(hand-split) custom leaders, and the timer that commits an expired key while it
is still held (it commits on the next key event instead).

### Unsafe code

`unsafe_code = "forbid"` is set once in the workspace `[workspace.lints]` and
inherited by every crate except `crates/fcitx5`, whose C ABI for the native
addon requires `unsafe`.

### Continuous integration

`.github/workflows/ci.yml` runs on every push and pull request, and weekly to
catch newly published advisories:

- **check**: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`,
  and `cargo xtask coverage`
- **msrv**: `cargo xtask msrv`
- **fcitx5**: `cargo xtask fcitx5 test` in an Arch Linux container, since
  Ubuntu's compilers are older than the minimums
- **freebsd** (currently disabled with `if: false`: the runner runs out of disk
  space): `cargo test --workspace` and `cargo xtask fcitx5 test` in a FreeBSD
  15.1 VM (QEMU, through [`vmactions/freebsd-vm`](https://github.com/vmactions/freebsd-vm))
  with the `latest` package branch. The security audit is not repeated there:
  it works on `Cargo.lock`, which is the same on every system, and `deny.toml`
  already evaluates the FreeBSD targets
- **audit**: `cargo xtask audit`

Scratch output from local analysis (coverage exports, logs) belongs under
`target/`, so `cargo clean` removes it.

## Roadmap

- [x] Native Fcitx5 addon (C++ shim calling the Rust C ABI)
- [x] One gesture state per window
- [x] Reload the configuration when its file changed
- [x] Overlay fed by the addon
- [x] Integration test of the addon inside a real Fcitx5 (its `testfrontend` addon)
- [x] Mappings and profiles persisted (`mappings.toml`) and edited in the editor
- [x] Persist usage counts across Fcitx5 restarts (`usage.toml`)
- [x] FreeBSD support (build, tests, native addon and CI on FreeBSD)

### Non-goals

- Feeding the overlay over DBus (the original's `Show(variants, index)` / `Quit()` interface): a Unix datagram socket does the same with no DBus dependency
- Shipping character preset files for multiple languages: mappings are customised with the editor or directly in the config file
- Profile merge mode for multi-language setups: a single profile can hold every mapping a user needs

## License

This project is licensed under the GNU General Public License, version 3 or (at
your option) any later version (`GPL-3.0-or-later`), matching the original
schnelle-umlaute license. See [`LICENSE`](LICENSE) for the full text and
[`AUTHORS`](AUTHORS) for the contributors.

## Acknowledgments

- Original [schnelle-umlaute](https://github.com/Maik-0000FF/schnelle-umlaute) project by [Maik-0000FF](https://github.com/Maik-0000FF)
- [iced](https://iced.rs/) for the UI toolkit and [waycrate](https://github.com/waycrate) for `iced_layershell`
- [PowerAccent](https://github.com/damienleroy/PowerAccent) by Damien Leroy (the original inspiration, later integrated into Windows PowerToys as Quick Accent)
