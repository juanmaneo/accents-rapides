//! What the addon build needs to know about the operating system.
//!
//! The rest of `cargo xtask fcitx5` is portable: pkg-config, and compiler
//! flags whose portability is checked where it is not universal (hardening
//! flags are probed per compiler). Each supported system implements
//! [`Platform`] in a module of its own; another system needs one more module
//! and a case in [`for_os`].

mod freebsd;
mod linux;

use anyhow::{bail, Result};

pub use freebsd::FreeBSD;
pub use linux::Linux;

pub trait Platform {
    /// System libraries the Rust staticlib needs, as printed by
    /// `rustc --print native-static-libs` on that system. The linker drops
    /// the unused ones (`--as-needed`).
    fn rust_native_libs(&self) -> &'static [&'static str];

    /// Linker flags of the addon shared library. The default suits ELF
    /// linkers (GNU ld and LLD).
    fn linker_flags(&self) -> &'static [&'static str] {
        ELF_LINKER_FLAGS
    }

    /// The executable to run for a logical compiler name (`g++`, `clang++`).
    /// The default is the name itself.
    fn resolve_compiler(&self, name: &str) -> String {
        name.to_owned()
    }

    /// Flags that make the logical compiler `name` use the system's C++
    /// standard library. Fcitx5's API passes standard library types, so the
    /// addon must use the standard library Fcitx5 was built with. The default
    /// is none: each compiler's own library is the system's.
    fn cxx_stdlib(&self, _name: &str) -> &'static [&'static str] {
        &[]
    }

    /// Logical name of the compiler whose addon `install` copies by default:
    /// the one the system builds its packages, Fcitx5 included, with.
    fn released_compiler(&self) -> &'static str;
}

const ELF_LINKER_FLAGS: &[&str] = &[
    // Fail at build time, not when Fcitx5 loads the addon.
    "-Wl,--no-undefined",
    // Keep the Rust staticlib's symbols internal to the addon.
    "-Wl,--exclude-libs,ALL",
    "-Wl,--gc-sections",
    "-Wl,--as-needed",
    // Full RELRO.
    "-Wl,-z,relro,-z,now",
];

/// The platform the addon is built on.
pub fn current() -> Result<&'static dyn Platform> {
    for_os(std::env::consts::OS)
}

/// The platform of `os`, a value of `std::env::consts::OS`.
fn for_os(os: &str) -> Result<&'static dyn Platform> {
    match os {
        "linux" => Ok(&Linux),
        "freebsd" => Ok(&FreeBSD),
        _ => bail!(
            "building the Fcitx5 addon on {os} is not supported yet: \
             implement `Platform` for it in xtask/src/fcitx5/platform/"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_systems_link_elf_shared_libraries() {
        for os in ["linux", "freebsd"] {
            let platform = for_os(os).unwrap();
            assert!(platform.rust_native_libs().contains(&"-lc"), "{os}");
            assert!(
                platform
                    .rust_native_libs()
                    .iter()
                    .all(|l| l.starts_with("-l")),
                "{os}"
            );
            assert_eq!(platform.linker_flags(), ELF_LINKER_FLAGS, "{os}");
        }
    }

    #[test]
    fn other_systems_ask_for_a_port() {
        let error = for_os("plan9").err().unwrap().to_string();
        assert!(error.contains("plan9") && error.contains("implement `Platform`"));
    }
}
