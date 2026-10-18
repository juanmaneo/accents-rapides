//! `cargo xtask fcitx5`: the build system of the native Fcitx5 addon. There
//! is no Makefile or CMake project: this module runs the compilers itself.
//!
//! The addon is `crates/fcitx5/cpp/addon.cpp` linked with the Rust staticlib
//! of `accents-rapides-fcitx5`. It is built by every compiler listed in
//! `[workspace.metadata.cpp.compilers]`, each into its own directory under
//! `target/fcitx5/`, since each one catches mistakes the other misses.
//! What depends on the operating system is in [`platform`].

mod platform;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use accents_rapides_core::{CONFIG_PATH_ENV, OVERLAY_SOCKET_ENV};
use anyhow::{ensure, Context, Result};
use clap::Subcommand;

use crate::{output_of, run_cargo, run_streamed, version_parts, workspace_root, RootManifest};
use platform::Platform;

#[derive(Subcommand)]
pub enum Fcitx5Command {
    /// Build the addon with every compiler, into
    /// target/fcitx5/<compiler>/accents-rapides.so.
    Build,
    /// Build, then run with every compiler the C ABI test (cpp/abi_test.cpp,
    /// under AddressSanitizer and UndefinedBehaviorSanitizer) and the
    /// integration test of the addon inside Fcitx5 (cpp/fcitx_test.cpp).
    Test,
    /// Install the addon built by `build`, which this does not run, so that
    /// it can run as root: `cargo xtask fcitx5 build && sudo
    /// ./target/debug/xtask fcitx5 install`.
    Install {
        /// Staging root for packagers.
        #[arg(long, default_value = "/")]
        destdir: PathBuf,
        /// Which compiler's build to install (default: the preferred one of
        /// this system).
        #[arg(long)]
        compiler: Option<String>,
    },
    /// Remove the files that `install` copies.
    Uninstall {
        #[arg(long, default_value = "/")]
        destdir: PathBuf,
    },
}

/// `[workspace.metadata.cpp]`: minimum versions of the C++ toolchain.
#[derive(Debug, serde::Deserialize)]
pub struct CppToolchain {
    standard: String,
    fcitx5: String,
    /// Compiler executable -> minimum version.
    compilers: BTreeMap<String, String>,
}

/// Warnings are errors: the code must be clean for both compilers.
const WARNINGS: &[&str] = &[
    "-Wall",
    "-Wextra",
    "-Wpedantic",
    "-Wconversion",
    "-Wsign-conversion",
    "-Wshadow",
    "-Wold-style-cast",
    "-Wnon-virtual-dtor",
    "-Woverloaded-virtual",
    "-Werror",
];

/// Hardening every compiler on every supported system accepts: the addon runs
/// inside the input method server, which sees every key.
const BASE_HARDENING: &[&str] = &["-fstack-protector-strong"];

/// Hardening that depends on the compiler and the C library (glibc's
/// `_FORTIFY_SOURCE`, libstdc++'s assertions, stack clash protection where
/// implemented). Each flag is probed per compiler: one it rejects would fail
/// the build, since warnings are errors.
const OPTIONAL_HARDENING: &[&str] = &[
    "-fstack-clash-protection",
    "-D_GLIBCXX_ASSERTIONS",
    // The same for libc++, the standard library of clang on FreeBSD.
    "-D_LIBCPP_HARDENING_MODE=_LIBCPP_HARDENING_MODE_EXTENSIVE",
    // Some distributions predefine it, with another level.
    "-U_FORTIFY_SOURCE",
    "-D_FORTIFY_SOURCE=3",
];

/// Only the Fcitx5 factory symbol is exported (the linker side is in
/// [`Platform::linker_flags`]).
const SHARED_LIBRARY: &[&str] = &[
    "-shared",
    "-fPIC",
    "-fvisibility=hidden",
    "-fvisibility-inlines-hidden",
];

const SANITIZERS: &[&str] = &[
    "-fsanitize=address,undefined",
    "-fno-sanitize-recover=all",
    "-fno-omit-frame-pointer",
];

/// The pkg-config package of the Fcitx5 development files.
const FCITX5_PACKAGE: &str = "Fcitx5Core";
/// Fcitx5Core plus the headers of its modules, among them testfrontend.
const FCITX5_MODULE_PACKAGE: &str = "Fcitx5Module";

pub fn run(command: Fcitx5Command) -> Result<()> {
    match command {
        Fcitx5Command::Build => build(
            &RootManifest::load()?.workspace.metadata.cpp,
            platform::current()?,
        ),
        Fcitx5Command::Test => test(
            &RootManifest::load()?.workspace.metadata.cpp,
            platform::current()?,
        ),
        Fcitx5Command::Install { destdir, compiler } => {
            let platform = platform::current()?;
            let compiler = compiler.unwrap_or_else(|| platform.released_compiler().to_owned());
            install(&destdir, &compiler)
        }
        Fcitx5Command::Uninstall { destdir } => uninstall(&destdir, platform::current()?),
    }
}

fn cpp_dir() -> PathBuf {
    workspace_root().join("crates/fcitx5/cpp")
}

fn target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map_or_else(|| workspace_root().join("target"), PathBuf::from)
}

fn staticlib() -> PathBuf {
    target_dir().join("release/libaccents_rapides_fcitx5.a")
}

/// Build directory of one compiler.
fn out_dir(compiler: &str) -> PathBuf {
    target_dir().join("fcitx5").join(compiler)
}

fn addon_path(compiler: &str) -> PathBuf {
    out_dir(compiler).join("accents-rapides.so")
}

fn pkg_config(package: &str, args: &[&str]) -> Result<String> {
    let mut args = args.to_vec();
    args.push(package);
    output_of("pkg-config", &args)
        .with_context(|| format!("Fcitx5 development files not found (pkg-config {package})"))
}

fn path_arg(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .with_context(|| format!("non UTF-8 path: {}", path.display()))
}

/// The version in a `--version` banner: the first word starting with a
/// digit and containing a dot, cut at the first character that is neither.
fn compiler_version(banner: &str) -> Option<&str> {
    banner
        .split_whitespace()
        .filter(|word| word.starts_with(|c: char| c.is_ascii_digit()))
        .map(|word| {
            let end = word
                .find(|c: char| !c.is_ascii_digit() && c != '.')
                .unwrap_or(word.len());
            &word[..end]
        })
        .find(|version| version.contains('.'))
}

/// Fail unless `found` is at least `minimum`.
fn check_version(what: &str, found: &str, minimum: &str) -> Result<()> {
    ensure!(
        version_parts(found) >= version_parts(minimum),
        "{what} {found} is older than the minimum {minimum} \
         (Cargo.toml [workspace.metadata.cpp])"
    );
    println!("{what} {found} (minimum {minimum})");
    Ok(())
}

/// One configured compiler: the executable to run (a logical name from
/// `[workspace.metadata.cpp.compilers]`, resolved for the platform) and the
/// flags it needs on this system: the standard library selection
/// ([`Platform::cxx_stdlib`]) and the hardening flags it accepts.
struct Compiler {
    executable: String,
    flags: Vec<String>,
}

/// Whether `compiler` accepts `flag`. An empty translation unit decides: an
/// unsupported option fails even without `-Werror`.
fn accepts_flag(compiler: &str, flag: &str) -> bool {
    let Ok(mut child) = Command::new(compiler)
        .args(["-fsyntax-only", "-x", "c++", "-"])
        .arg(flag)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    drop(child.stdin.take());
    child.wait().is_ok_and(|status| status.success())
}

/// The hardening flags of `compiler`: the base set plus every optional one
/// it accepts.
fn hardening_of(compiler: &str) -> Vec<String> {
    BASE_HARDENING
        .iter()
        .chain(
            OPTIONAL_HARDENING
                .iter()
                .filter(|flag| accepts_flag(compiler, flag)),
        )
        .map(|&flag| flag.to_owned())
        .collect()
}

/// Resolve every compiler of `[workspace.metadata.cpp.compilers]` to an
/// executable, then check the versions of the compilers and Fcitx5.
fn check_toolchain(
    toolchain: &CppToolchain,
    platform: &dyn Platform,
) -> Result<BTreeMap<String, Compiler>> {
    let mut compilers = BTreeMap::new();
    for (name, minimum) in &toolchain.compilers {
        let executable = platform.resolve_compiler(name);
        let banner = output_of(&executable, &["--version"])
            .with_context(|| format!("{name} is required (tried `{executable}`)"))?;
        let found = compiler_version(&banner)
            .with_context(|| format!("no version in `{executable} --version`: {banner}"))?;
        let label = if executable == *name {
            name.clone()
        } else {
            format!("{name} ({executable})")
        };
        check_version(&label, found, minimum)?;
        let flags = platform
            .cxx_stdlib(name)
            .iter()
            .map(|&flag| flag.to_owned())
            .chain(hardening_of(&executable))
            .collect();
        compilers.insert(name.clone(), Compiler { executable, flags });
    }
    check_version(
        "Fcitx5",
        &pkg_config(FCITX5_PACKAGE, &["--modversion"])?,
        &toolchain.fcitx5,
    )?;
    Ok(compilers)
}

/// `-I<dir>` -> `-isystem <dir>`: warnings in Fcitx5 headers are not ours
/// to fix, and `-Werror` would turn them into errors.
fn system_includes(cflags: &str) -> Vec<String> {
    cflags
        .split_whitespace()
        .flat_map(|flag| match flag.strip_prefix("-I") {
            Some(dir) => vec!["-isystem".to_owned(), dir.to_owned()],
            None => vec![flag.to_owned()],
        })
        .collect()
}

/// Flags shared by every compilation (and link) of one compiler.
fn compile_flags(toolchain: &CppToolchain, compiler_flags: &[String]) -> Vec<String> {
    let mut flags = vec![format!("-std={}", toolchain.standard), "-O2".to_owned()];
    flags.extend(WARNINGS.iter().map(|&f| f.to_owned()));
    flags.extend(compiler_flags.iter().cloned());
    flags
}

/// Run a compiler, printing its command line first so the flags are visible.
fn compile(compiler: &str, args: &[String]) -> Result<()> {
    println!("{compiler} {}", args.join(" "));
    run_streamed(compiler, args)
}

fn build(toolchain: &CppToolchain, platform: &dyn Platform) -> Result<()> {
    let compilers = check_toolchain(toolchain, platform)?;
    build_with(toolchain, platform, &compilers)
}

fn build_with(
    toolchain: &CppToolchain,
    platform: &dyn Platform,
    compilers: &BTreeMap<String, Compiler>,
) -> Result<()> {
    run_cargo(&[
        "rustc",
        "--package",
        "accents-rapides-fcitx5",
        "--release",
        "--crate-type",
        "staticlib",
    ])?;

    let includes = system_includes(&pkg_config(FCITX5_PACKAGE, &["--cflags"])?);
    let libs = pkg_config(FCITX5_PACKAGE, &["--libs"])?;
    for (name, compiler) in compilers {
        std::fs::create_dir_all(out_dir(name))?;
        let mut args = compile_flags(toolchain, &compiler.flags);
        args.extend(includes.iter().cloned());
        args.extend(
            SHARED_LIBRARY
                .iter()
                .chain(platform.linker_flags())
                .map(|&f| f.to_owned()),
        );
        args.extend([
            "-o".to_owned(),
            path_arg(&addon_path(name))?,
            path_arg(&cpp_dir().join("addon.cpp"))?,
            path_arg(&staticlib())?,
        ]);
        args.extend(libs.split_whitespace().map(str::to_owned));
        args.extend(platform.rust_native_libs().iter().map(|&f| f.to_owned()));
        compile(&compiler.executable, &args)?;
    }
    Ok(())
}

fn test(toolchain: &CppToolchain, platform: &dyn Platform) -> Result<()> {
    let compilers = check_toolchain(toolchain, platform)?;
    build_with(toolchain, platform, &compilers)?;
    let includes = system_includes(&pkg_config(FCITX5_MODULE_PACKAGE, &["--cflags"])?);
    let libs = pkg_config(FCITX5_MODULE_PACKAGE, &["--libs"])?;
    for (name, compiler) in &compilers {
        abi_test(toolchain, platform, name, compiler)?;
        fcitx_test(toolchain, name, compiler, &includes, &libs)?;
    }
    Ok(())
}

/// The C ABI test links the staticlib alone, so it can run fully under the
/// sanitizers.
fn abi_test(
    toolchain: &CppToolchain,
    platform: &dyn Platform,
    name: &str,
    compiler: &Compiler,
) -> Result<()> {
    let test = TestFiles::fresh(name, "abi_test")?;
    let mut args = compile_flags(toolchain, &compiler.flags);
    args.extend(SANITIZERS.iter().map(|&f| f.to_owned()));
    args.extend([
        "-o".to_owned(),
        path_arg(&test.exe)?,
        path_arg(&cpp_dir().join("abi_test.cpp"))?,
        path_arg(&staticlib())?,
    ]);
    args.extend(platform.rust_native_libs().iter().map(|&f| f.to_owned()));
    compile(&compiler.executable, &args)?;
    test.run(&[])
}

/// The integration test runs a real Fcitx5, which loads the addon exactly as
/// built and installed: no sanitizers, since neither Fcitx5 nor the addon is
/// built with them.
fn fcitx_test(
    toolchain: &CppToolchain,
    name: &str,
    compiler: &Compiler,
    includes: &[String],
    libs: &str,
) -> Result<()> {
    let test = TestFiles::fresh(name, "fcitx_test")?;
    let mut args = compile_flags(toolchain, &compiler.flags);
    args.extend(includes.iter().cloned());
    args.extend([
        "-o".to_owned(),
        path_arg(&test.exe)?,
        path_arg(&cpp_dir().join("fcitx_test.cpp"))?,
    ]);
    args.extend(libs.split_whitespace().map(str::to_owned));
    compile(&compiler.executable, &args)?;

    // Installed under the test directory, where Fcitx5 is told to look.
    let layout = Layout::new(&test.dir, "lib", "");
    copy_files(&layout.files(addon_path(name)))?;
    test.run(&[
        path_arg(&layout.addon_dir)?,
        path_arg(&layout.data_dir)?,
        path_arg(&test.config)?,
    ])
}

/// The files of one test built by one compiler, in a directory of its own.
struct TestFiles {
    dir: PathBuf,
    exe: PathBuf,
    config: PathBuf,
}

impl TestFiles {
    /// In a fresh directory: the tests expect the default configuration.
    fn fresh(compiler: &str, name: &str) -> Result<Self> {
        let dir = out_dir(compiler).join(name);
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        std::fs::create_dir_all(&dir)?;
        Ok(Self {
            exe: dir.join(name),
            config: dir.join("config.toml"),
            dir,
        })
    }

    /// Run the test, with its configuration and overlay socket in its
    /// directory rather than the user's.
    fn run(&self, args: &[String]) -> Result<()> {
        println!("{}", self.exe.display());
        let status = Command::new(&self.exe)
            .args(args)
            .env(CONFIG_PATH_ENV, &self.config)
            .env(OVERLAY_SOCKET_ENV, self.dir.join("overlay.sock"))
            .status()
            .with_context(|| format!("failed to run {}", self.exe.display()))?;
        ensure!(status.success(), "{} failed", self.exe.display());
        Ok(())
    }
}

/// Where Fcitx5 looks for the addon and its registration files.
struct Layout {
    addon_dir: PathBuf,
    data_dir: PathBuf,
}

impl Layout {
    /// `libdir` and `prefix` are those of the Fcitx5 pkg-config file;
    /// `destdir` prefixes both.
    fn new(destdir: &Path, libdir: &str, prefix: &str) -> Self {
        let under = |dir: &str| destdir.join(dir.trim_start_matches('/'));
        Self {
            addon_dir: under(libdir).join("fcitx5"),
            data_dir: under(prefix).join("share/fcitx5"),
        }
    }

    fn system(destdir: &Path) -> Result<Self> {
        Ok(Self::new(
            destdir,
            &pkg_config(FCITX5_PACKAGE, &["--variable=libdir"])?,
            &pkg_config(FCITX5_PACKAGE, &["--variable=prefix"])?,
        ))
    }

    /// `(source, destination)` of every installed file, `addon` being the
    /// built shared library.
    fn files(&self, addon: PathBuf) -> [(PathBuf, PathBuf); 3] {
        let data = workspace_root().join("crates/fcitx5/data");
        [
            (addon, self.addon_dir.join("accents-rapides.so")),
            (
                data.join("accents-rapides-addon.conf"),
                self.data_dir.join("addon/accents-rapides.conf"),
            ),
            (
                data.join("accents-rapides-inputmethod.conf"),
                self.data_dir.join("inputmethod/accents-rapides.conf"),
            ),
        ]
    }
}

fn install(destdir: &Path, compiler: &str) -> Result<()> {
    copy_files(&Layout::system(destdir)?.files(addon_path(compiler)))?;
    println!(
        "Restart Fcitx5 (`fcitx5 -r`), then add \"Accents Rapides\" as an input \
         method in fcitx5-configtool."
    );
    Ok(())
}

/// Copy each `(source, destination)` of an install plan.
fn copy_files(plan: &[(PathBuf, PathBuf)]) -> Result<()> {
    for (source, destination) in plan {
        ensure!(
            source.exists(),
            "{} is missing: run `cargo xtask fcitx5 build` first",
            source.display()
        );
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("cannot create {}", parent.display()))?;
        }
        std::fs::copy(source, destination)
            .with_context(|| format!("cannot install {}", destination.display()))?;
        println!("installed {}", destination.display());
    }
    Ok(())
}

fn uninstall(destdir: &Path, platform: &dyn Platform) -> Result<()> {
    let addon = addon_path(platform.released_compiler());
    for (_, destination) in Layout::system(destdir)?.files(addon) {
        if destination.exists() {
            std::fs::remove_file(&destination)
                .with_context(|| format!("cannot remove {}", destination.display()))?;
            println!("removed {}", destination.display());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_compiler_versions() {
        assert_eq!(
            compiler_version("g++ (GCC) 16.2.1 20260810\nCopyright (C) 2026"),
            Some("16.2.1")
        );
        assert_eq!(
            compiler_version("clang version 22.1.8\nTarget: x86_64-pc-linux-gnu"),
            Some("22.1.8")
        );
        assert_eq!(
            compiler_version("g++ (Ubuntu 13.2.0-23ubuntu4) 13.2.0"),
            Some("13.2.0")
        );
        assert_eq!(
            compiler_version("Ubuntu clang version 18.1.3 (1ubuntu1)"),
            Some("18.1.3")
        );
        assert_eq!(compiler_version("no version here 2026"), None);
    }

    #[test]
    fn version_check_accepts_newer_and_rejects_older() {
        assert!(check_version("g++", "16.2.1", "16.2.1").is_ok());
        assert!(check_version("g++", "17.0.0", "16.2.1").is_ok());
        assert!(check_version("g++", "16.10", "16.2.1").is_ok());
        assert!(check_version("g++", "14.2.0", "16.2.1").is_err());
    }

    #[test]
    fn fcitx5_includes_become_system_includes() {
        assert_eq!(
            system_includes("-I/usr/include/Fcitx5/Core -DFOO -I/usr/include/Fcitx5/Utils"),
            [
                "-isystem",
                "/usr/include/Fcitx5/Core",
                "-DFOO",
                "-isystem",
                "/usr/include/Fcitx5/Utils"
            ]
        );
    }

    #[test]
    fn workspace_declares_the_cpp_toolchain() {
        let toolchain = RootManifest::load().unwrap().workspace.metadata.cpp;
        assert_eq!(toolchain.standard, "c++23");
        assert!(!version_parts(&toolchain.fcitx5).is_empty());
        assert_eq!(
            toolchain.compilers.keys().collect::<Vec<_>>(),
            ["clang++", "g++"]
        );
        for version in toolchain.compilers.values() {
            assert!(compiler_version(version).is_some(), "{version}");
        }
        assert_eq!(compile_flags(&toolchain, &[])[0], "-std=c++23");
        assert!(compile_flags(&toolchain, &["-D_FOO".to_owned()]).contains(&"-D_FOO".to_owned()));
    }

    #[test]
    fn platforms_release_a_configured_compiler() {
        let toolchain = RootManifest::load().unwrap().workspace.metadata.cpp;
        for platform in [&platform::Linux as &dyn Platform, &platform::FreeBSD] {
            let name = platform.released_compiler();
            assert!(toolchain.compilers.contains_key(name), "{name}");
        }
    }

    #[test]
    fn missing_compiler_accepts_no_flag() {
        assert!(!accepts_flag(
            "accents-rapides-no-such-compiler",
            "-fsyntax-only"
        ));
    }

    #[test]
    fn optional_hardening_never_repeats_the_base_set() {
        assert!(BASE_HARDENING
            .iter()
            .all(|flag| !OPTIONAL_HARDENING.contains(flag)));
    }

    #[test]
    fn layout_honors_destdir() {
        let layout = Layout::new(Path::new("/stage"), "/usr/lib", "/usr");
        assert_eq!(layout.addon_dir, Path::new("/stage/usr/lib/fcitx5"));
        assert_eq!(layout.data_dir, Path::new("/stage/usr/share/fcitx5"));
        let [(addon, so), (_, addon_conf), (_, im_conf)] = layout.files(PathBuf::from("a.so"));
        assert_eq!(addon, Path::new("a.so"));
        assert_eq!(so, Path::new("/stage/usr/lib/fcitx5/accents-rapides.so"));
        assert_eq!(
            addon_conf,
            Path::new("/stage/usr/share/fcitx5/addon/accents-rapides.conf")
        );
        assert_eq!(
            im_conf,
            Path::new("/stage/usr/share/fcitx5/inputmethod/accents-rapides.conf")
        );
    }
}
