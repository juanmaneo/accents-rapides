//! Linux: distributions build Fcitx5 with GCC and libstdc++, which clang++
//! also uses there, and install the compilers under their plain names.

use super::Platform;

pub struct Linux;

impl Platform for Linux {
    fn rust_native_libs(&self) -> &'static [&'static str] {
        &[
            "-lgcc_s",
            "-lutil",
            "-lrt",
            "-lpthread",
            "-lm",
            "-ldl",
            "-lc",
        ]
    }

    fn released_compiler(&self) -> &'static str {
        "g++"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compilers_use_their_own_standard_library() {
        assert!(Linux.cxx_stdlib("g++").is_empty());
        assert!(Linux.cxx_stdlib("clang++").is_empty());
        assert_eq!(Linux.resolve_compiler("g++"), "g++");
        assert_eq!(Linux.released_compiler(), "g++");
    }
}
