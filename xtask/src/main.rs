//! Developer tasks for accents-rapides.
//!
//! Run with `cargo xtask <subcommand>` (see .cargo/config.toml for the alias).

mod fcitx5;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, ensure, Context, Result};
use clap::{Parser, Subcommand};

/// The workspace root, resolved from this crate's manifest directory.
fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives in the workspace root")
}

/// A `cargo <args>` command running in the workspace root.
fn cargo(args: &[&str]) -> Command {
    let mut command = Command::new("cargo");
    command.args(args).current_dir(workspace_root());
    command
}

/// Whether the `cargo <tool>` subcommand is available.
fn is_installed(tool: &str) -> bool {
    cargo(&[tool, "--version"])
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Install the `cargo-<tool>` subcommand if it is missing, so that a fresh
/// machine (or CI runner) only needs `cargo xtask <task>`.
fn ensure_installed(tool: &str) -> Result<()> {
    if is_installed(tool) {
        return Ok(());
    }
    println!("`cargo {tool}` is missing, installing cargo-{tool}...");
    run_cargo(&["install", "--locked", &format!("cargo-{tool}")])
}

/// Run `<program> <args>` in the workspace root with its output streamed to
/// the terminal, failing on a non-zero exit status.
fn run_streamed<S: AsRef<str>>(program: &str, args: &[S]) -> Result<()> {
    let args: Vec<&str> = args.iter().map(AsRef::as_ref).collect();
    let command_line = format!("{program} {}", args.join(" "));
    let status = Command::new(program)
        .args(&args)
        .current_dir(workspace_root())
        .status()
        .with_context(|| format!("failed to run `{command_line}`"))?;
    ensure!(status.success(), "`{command_line}` failed");
    Ok(())
}

fn run_cargo(args: &[&str]) -> Result<()> {
    run_streamed("cargo", args)
}

/// The trimmed standard output of `<program> <args>`, failing on a non-zero
/// exit status.
fn output_of(program: &str, args: &[&str]) -> Result<String> {
    let command_line = format!("{program} {}", args.join(" "));
    let output = Command::new(program)
        .args(args)
        .stderr(Stdio::inherit())
        .output()
        .with_context(|| format!("failed to run `{command_line}`"))?;
    ensure!(output.status.success(), "`{command_line}` failed");
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// `1.89` -> `[1, 89]`, so versions compare numerically. Stops at the first
/// part that is not a number.
fn version_parts(version: &str) -> Vec<u64> {
    version
        .split('.')
        .map_while(|part| part.parse().ok())
        .collect()
}

/// Read a file from the workspace root.
fn read_workspace_file(name: &str) -> Result<String> {
    std::fs::read_to_string(workspace_root().join(name))
        .with_context(|| format!("failed to read {name}"))
}

/// Path of the coverage ratchet file.
fn ratchet_path() -> PathBuf {
    workspace_root().join("coverage-ratchet.json")
}

/// The ratchet gate: the minimum total coverage (percent) that `cargo xtask
/// coverage` enforces. It only ever moves up (`coverage --bump`), never down,
/// until it reaches 100.
#[derive(serde::Serialize, serde::Deserialize)]
struct CoverageRatchet {
    /// Minimum allowed total coverage, in percent.
    percent_covered: f64,
}

impl CoverageRatchet {
    fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read ratchet file {}", path.display()))?;
        serde_json::from_str(&content)
            .with_context(|| format!("failed to parse ratchet file {}", path.display()))
    }

    fn store(&self, path: &Path) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)
            .with_context(|| format!("failed to write ratchet file {}", path.display()))
    }
}

/// Selected totals from `cargo llvm-cov --json` output.
#[derive(serde::Deserialize)]
struct LlvmCovReport {
    data: Vec<LlvmCovData>,
}

#[derive(serde::Deserialize)]
struct LlvmCovData {
    totals: LlvmCovTotals,
}

#[derive(serde::Deserialize)]
struct LlvmCovTotals {
    lines: LlvmCovMetric,
    regions: LlvmCovMetric,
    functions: LlvmCovMetric,
}

#[derive(serde::Deserialize)]
struct LlvmCovMetric {
    covered: u64,
    /// Total for the metric (llvm-cov JSON names it `count`).
    count: u64,
}

/// Combined coverage summary used for the ratchet comparison and reporting.
#[derive(Debug, Clone, Copy, PartialEq)]
struct CoverageSummary {
    lines: (u64, u64),
    regions: (u64, u64),
    functions: (u64, u64),
}

impl CoverageSummary {
    /// Overall percent covered: covered (lines + regions + functions) over
    /// their totals, matching how llvm-cov computes `percent_covered`.
    fn percent_covered(&self) -> f64 {
        let covered = self.lines.0 + self.regions.0 + self.functions.0;
        let total = self.lines.1 + self.regions.1 + self.functions.1;
        if total == 0 {
            return 100.0;
        }
        (covered as f64 / total as f64) * 100.0
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("xtask failed: {error:#}");
        std::process::exit(1);
    }
}

#[derive(Parser)]
#[command(name = "xtask", about = "Developer tasks for accents-rapides")]
struct Cli {
    #[command(subcommand)]
    command: Command_,
}

#[derive(Subcommand)]
enum Command_ {
    /// Measure test coverage and enforce the coverage ratchet.
    ///
    /// Runs `cargo llvm-cov` over the workspace (excluding xtask itself),
    /// parses the totals, and fails if total coverage dropped below the
    /// gate stored in coverage-ratchet.json. The ratchet only ever goes up:
    /// raise it with `cargo xtask coverage --bump` after improving
    /// coverage. The goal is 100% of all files, lines and regions.
    Coverage {
        /// Raise the ratchet to the measured coverage (never lowers it).
        #[arg(long)]
        bump: bool,

        /// Evaluate an existing `cargo llvm-cov --json` report instead of
        /// running a fresh measurement (useful for CI or debugging).
        #[arg(long)]
        json_report: Option<PathBuf>,
    },

    /// Audit the dependency tree for security and policy problems.
    ///
    /// Runs `cargo audit` (RustSec vulnerabilities and yanked crates in
    /// Cargo.lock), then `cargo deny check` (RustSec advisories including
    /// unmaintained and unsound crates, the license allow-list, duplicate
    /// versions and dependency sources, as configured in deny.toml).
    /// Accepted advisories are listed, with their reason, in deny.toml only;
    /// their ids are forwarded to `cargo audit --ignore`. Missing tools are
    /// installed with `cargo install --locked`. Needs network access to
    /// fetch the advisory database.
    Audit,

    /// Check that the workspace builds with its minimum supported Rust
    /// version.
    ///
    /// Reads `workspace.package.rust-version` from Cargo.toml, installs that
    /// toolchain with rustup if needed, and runs `cargo check` on every
    /// target with it, using Cargo.lock as is.
    Msrv,

    /// Build, test and install the native Fcitx5 addon (crates/fcitx5/cpp).
    ///
    /// xtask is the build system of the C++ side (there is no Makefile or
    /// CMake project): it runs the compilers itself. Every compiler listed in
    /// `[workspace.metadata.cpp]` of Cargo.toml builds it, after its version
    /// and the Fcitx5 version were checked against the minimums there.
    Fcitx5 {
        #[command(subcommand)]
        command: fcitx5::Fcitx5Command,
    },
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command_::Coverage { bump, json_report } => coverage(bump, json_report),
        Command_::Audit => audit(),
        Command_::Msrv => msrv(),
        Command_::Fcitx5 { command } => fcitx5::run(command),
    }
}

/// The parts of the root Cargo.toml that xtask needs.
#[derive(serde::Deserialize)]
struct RootManifest {
    workspace: WorkspaceTable,
}

#[derive(serde::Deserialize)]
struct WorkspaceTable {
    package: WorkspacePackage,
    metadata: WorkspaceMetadata,
}

#[derive(serde::Deserialize)]
struct WorkspacePackage {
    #[serde(rename = "rust-version")]
    rust_version: String,
}

#[derive(serde::Deserialize)]
struct WorkspaceMetadata {
    cpp: fcitx5::CppToolchain,
}

impl RootManifest {
    fn parse(cargo_toml: &str) -> Result<Self> {
        toml::from_str(cargo_toml).context("failed to parse the workspace Cargo.toml")
    }

    fn load() -> Result<Self> {
        Self::parse(&read_workspace_file("Cargo.toml")?)
    }
}

fn msrv() -> Result<()> {
    let version = RootManifest::load()?.workspace.package.rust_version;
    println!("Checking the workspace with its MSRV, Rust {version}");
    run_streamed(
        "rustup",
        &["toolchain", "install", &version, "--profile", "minimal"],
    )?;
    // `+<version>` takes precedence over rust-toolchain.toml.
    run_cargo(&[
        &format!("+{version}"),
        "check",
        "--workspace",
        "--all-targets",
        "--locked",
    ])
}

/// The parts of deny.toml that `cargo xtask audit` shares with cargo-audit.
#[derive(serde::Deserialize)]
struct DenyConfig {
    advisories: DenyAdvisories,
}

#[derive(serde::Deserialize)]
struct DenyAdvisories {
    #[serde(default)]
    ignore: Vec<DenyIgnore>,
}

#[derive(serde::Deserialize)]
struct DenyIgnore {
    id: String,
}

/// Advisory ids accepted in deny.toml.
fn ignored_advisories(deny_toml: &str) -> Result<Vec<String>> {
    let config: DenyConfig = toml::from_str(deny_toml).context("failed to parse deny.toml")?;
    Ok(config.advisories.ignore.into_iter().map(|i| i.id).collect())
}

fn audit() -> Result<()> {
    ensure_installed("audit")?;
    ensure_installed("deny")?;

    let ignored = ignored_advisories(&read_workspace_file("deny.toml")?)?;
    let mut audit_args = vec!["audit"];
    for id in &ignored {
        audit_args.extend(["--ignore", id.as_str()]);
    }

    run_cargo(&audit_args)?;
    run_cargo(&["deny", "check"])
}

fn coverage(bump: bool, json_report: Option<PathBuf>) -> Result<()> {
    let ratchet_file = ratchet_path();
    let ratchet = CoverageRatchet::load(&ratchet_file)?;

    let raw = match json_report {
        Some(path) => std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read report {}", path.display()))?,
        None => run_llvm_cov()?,
    };

    let summary = parse_report(&raw)?;
    let measured = summary.percent_covered();

    println!("Coverage summary:");
    println!(
        "  lines:     {}/{} ({:.1}%)",
        summary.lines.0,
        summary.lines.1,
        percent(summary.lines)
    );
    println!(
        "  regions:   {}/{} ({:.1}%)",
        summary.regions.0,
        summary.regions.1,
        percent(summary.regions)
    );
    println!(
        "  functions: {}/{} ({:.1}%)",
        summary.functions.0,
        summary.functions.1,
        percent(summary.functions)
    );
    println!("  total:     {:.2}%", measured);
    println!("  ratchet:   {:.2}% (goal: 100%)", ratchet.percent_covered);

    if measured + f64::EPSILON < ratchet.percent_covered {
        bail!(
            "coverage ratchet failed: measured {:.2}% is below the gate {:.2}%. \
             Fix the uncovered code or revert the regression.",
            measured,
            ratchet.percent_covered
        );
    }

    if bump {
        if measured > ratchet.percent_covered {
            let updated = CoverageRatchet {
                percent_covered: measured,
            };
            updated.store(&ratchet_file)?;
            println!(
                "Ratchet raised: {:.2}% -> {:.2}%",
                ratchet.percent_covered, measured
            );
        } else {
            println!(
                "Ratchet unchanged: measured {:.2}% does not exceed the gate {:.2}%",
                measured, ratchet.percent_covered
            );
        }
    } else if measured > ratchet.percent_covered {
        println!("Coverage is above the gate. Raise it with `cargo xtask coverage --bump`.");
    }

    Ok(())
}

fn percent((covered, total): (u64, u64)) -> f64 {
    if total == 0 {
        100.0
    } else {
        (covered as f64 / total as f64) * 100.0
    }
}

/// Run `cargo llvm-cov` over the workspace and return its JSON report.
fn run_llvm_cov() -> Result<String> {
    ensure_installed("llvm-cov")?;

    // xtask and test-support are developer tooling, not product code:
    // exclude them from the measured set. Build and test progress goes to
    // stderr and is streamed; the JSON report is captured from stdout.
    let output = cargo(&[
        "llvm-cov",
        "--workspace",
        "--exclude",
        "xtask",
        "--exclude",
        "accents-rapides-test-support",
        "--json",
    ])
    .stderr(Stdio::inherit())
    .output()
    .context("failed to run `cargo llvm-cov`")?;
    ensure!(output.status.success(), "`cargo llvm-cov` failed");

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Parse the totals out of a `cargo llvm-cov --json` report.
fn parse_report(raw: &str) -> Result<CoverageSummary> {
    let report: LlvmCovReport =
        serde_json::from_str(raw).context("failed to parse llvm-cov JSON report")?;

    let data = report
        .data
        .first()
        .context("llvm-cov report contains no data")?;

    Ok(CoverageSummary {
        lines: (data.totals.lines.covered, data.totals.lines.count),
        regions: (data.totals.regions.covered, data.totals.regions.count),
        functions: (data.totals.functions.covered, data.totals.functions.count),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_llvm_cov_json_totals() {
        let raw = r#"{
            "data": [
                {
                    "totals": {
                        "lines": {"count": 10, "covered": 8, "percent": 80.0},
                        "regions": {"count": 20, "covered": 12, "percent": 60.0},
                        "functions": {"count": 4, "covered": 3, "percent": 75.0}
                    }
                }
            ]
        }"#;
        let summary = parse_report(raw).unwrap();
        assert_eq!(summary.lines, (8, 10));
        assert_eq!(summary.regions, (12, 20));
        assert_eq!(summary.functions, (3, 4));
        // (8+12+3) / (10+20+4) = 23/34 = 67.647%
        assert!((summary.percent_covered() - 67.647_058).abs() < 0.001);
    }

    #[test]
    fn empty_report_is_full_coverage() {
        let raw = r#"{
            "data": [
                {
                    "totals": {
                        "lines": {"count": 0, "covered": 0, "percent": 0.0},
                        "regions": {"count": 0, "covered": 0, "percent": 0.0},
                        "functions": {"count": 0, "covered": 0, "percent": 0.0}
                    }
                }
            ]
        }"#;
        let summary = parse_report(raw).unwrap();
        assert!((summary.percent_covered() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ratchet_roundtrip() {
        let dir = std::env::temp_dir().join("xtask-ratchet-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ratchet.json");

        CoverageRatchet {
            percent_covered: 42.5,
        }
        .store(&path)
        .unwrap();
        let loaded = CoverageRatchet::load(&path).unwrap();
        assert!((loaded.percent_covered - 42.5).abs() < f64::EPSILON);
    }

    #[test]
    fn detects_missing_tool() {
        assert!(!is_installed("no-such-xtask-tool"));
    }

    #[test]
    fn reads_ignored_advisories_from_deny_toml() {
        let ids = ignored_advisories(
            r#"
            [advisories]
            ignore = [
                { id = "RUSTSEC-0000-0001", reason = "a" },
                { id = "RUSTSEC-0000-0002", reason = "b" },
            ]
            "#,
        )
        .unwrap();
        assert_eq!(ids, ["RUSTSEC-0000-0001", "RUSTSEC-0000-0002"]);
    }

    #[test]
    fn workspace_deny_toml_is_readable() {
        let raw = read_workspace_file("deny.toml").unwrap();
        assert!(ignored_advisories(&raw).is_ok());
    }

    #[test]
    fn splits_versions_into_numbers() {
        assert_eq!(version_parts("1.89"), [1, 89]);
        assert_eq!(version_parts("16.2.1"), [16, 2, 1]);
        assert_eq!(version_parts("5.1.x"), [5, 1]);
        assert!(version_parts("22.1.8") > version_parts("22.1"));
        assert!(version_parts("16.10.0") > version_parts("16.9.9"));
    }

    /// The pinned development toolchain must satisfy the promised MSRV,
    /// otherwise our own builds could rely on newer features unnoticed.
    #[test]
    fn pinned_toolchain_is_not_older_than_msrv() {
        #[derive(serde::Deserialize)]
        struct ToolchainFile {
            toolchain: Toolchain,
        }
        #[derive(serde::Deserialize)]
        struct Toolchain {
            channel: String,
        }

        let msrv = RootManifest::load().unwrap().workspace.package.rust_version;
        let file: ToolchainFile =
            toml::from_str(&read_workspace_file("rust-toolchain.toml").unwrap()).unwrap();
        let pinned = file.toolchain.channel;
        assert!(
            version_parts(&pinned) >= version_parts(&msrv),
            "rust-toolchain.toml pins {pinned}, older than rust-version {msrv}"
        );
    }
}
