//! FreeBSD: packages, Fcitx5 included, are built with the base system's
//! clang and its C++ library, libc++. The compilers we need come from
//! packages under versioned names (`g++16`, `clang++22`): the base system's
//! plain `clang++` is older.

use std::path::Path;

use super::Platform;
use crate::version_parts;

pub struct FreeBSD;

impl Platform for FreeBSD {
    fn rust_native_libs(&self) -> &'static [&'static str] {
        // `cargo rustc --package accents-rapides-fcitx5 --release --crate-type
        // staticlib --target x86_64-unknown-freebsd -- --print
        // native-static-libs`, duplicates removed (first occurrence kept).
        &[
            "-lrt",
            "-lutil",
            "-lexecinfo",
            "-lkvm",
            "-lmemstat",
            "-lprocstat",
            "-ldevstat",
            "-lpthread",
            "-lgcc_s",
            "-lc",
            "-lm",
        ]
    }

    fn resolve_compiler(&self, name: &str) -> String {
        versioned_compiler_names(name, &executables_in_path())
            .into_iter()
            .next()
            .unwrap_or_else(|| name.to_owned())
    }

    /// libc++ is clang++'s default; the gcc ports support it through
    /// `-stdlib=libc++`. GCC then reports false positives in libc++'s
    /// `<format>` once it is inlined into our code: a one-line
    /// `std::println("{}", 1)` at `-O2` is enough, and `-Werror` would turn
    /// them into errors. Those two warnings stay on for g++ on Linux
    /// (libstdc++) and for clang++.
    fn cxx_stdlib(&self, name: &str) -> &'static [&'static str] {
        match name {
            "g++" => &[
                "-stdlib=libc++",
                "-Wno-stringop-overflow",
                "-Wno-array-bounds",
            ],
            _ => &[],
        }
    }

    fn released_compiler(&self) -> &'static str {
        "clang++"
    }
}

/// Every executable file name in PATH, without duplicates.
fn executables_in_path() -> Vec<String> {
    let Some(paths) = std::env::var_os("PATH") else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for dir in std::env::split_paths(&paths) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if is_executable(&entry.path()) {
                let name = entry.file_name().to_string_lossy().into_owned();
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
    }
    names
}

/// Whether `path` is a regular executable file.
fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Candidate executables for `name` among `found`: the versioned ones
/// (`g++15`, `g++-15`, `clang++20`, ...), highest version first, then the
/// plain name. Anything that is neither is ignored.
fn versioned_compiler_names(name: &str, found: &[String]) -> Vec<String> {
    let mut candidates = Vec::new();
    for candidate in found {
        if candidate == name {
            continue;
        }
        let Some(suffix) = candidate.strip_prefix(name) else {
            continue;
        };
        let digits = suffix.trim_start_matches('-');
        if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        candidates.push((version_parts(digits), candidate.clone()));
    }
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    candidates.push((Vec::new(), name.to_owned()));
    candidates.into_iter().map(|(_, name)| name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_compiler_uses_libc_plus_plus() {
        assert!(FreeBSD.cxx_stdlib("g++").contains(&"-stdlib=libc++"));
        assert!(FreeBSD.cxx_stdlib("clang++").is_empty());
        assert_eq!(FreeBSD.released_compiler(), "clang++");
        assert!(FreeBSD.rust_native_libs().contains(&"-lexecinfo"));
    }

    #[test]
    fn prefers_the_highest_versioned_compiler() {
        let found = [
            "clang++".to_owned(),
            "clang++-18".to_owned(),
            "clang++20".to_owned(),
            "clang++-22".to_owned(),
            "clang-format".to_owned(),
            "clang-tidy".to_owned(),
        ];
        assert_eq!(
            versioned_compiler_names("clang++", &found),
            ["clang++-22", "clang++20", "clang++-18", "clang++"]
        );
    }

    #[test]
    fn falls_back_to_the_plain_name() {
        assert_eq!(
            FreeBSD.resolve_compiler("accents-rapides-no-such-compiler"),
            "accents-rapides-no-such-compiler"
        );
        assert!(!is_executable(Path::new("/nonexistent/accents-rapides")));
    }
}
