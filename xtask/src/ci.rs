use std::{
    io::{self, Write},
    path::Path,
    process::{Command, Stdio},
};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use crate::paths::{cargo_command, default_host_target, release_binary_path, workspace_root};

const CI_RUSTFLAGS: &str = "-D warnings";

#[derive(Subcommand)]
pub enum CiCommands {
    /// `cargo fmt --check`
    Fmt,
    /// `cargo clippy` with warnings denied
    Clippy,
    /// `cargo test --workspace` for the host or `--target`
    Test(TestArgs),
    /// Release build of the `wisp` binary (`dist` profile by default)
    BuildRelease(BuildReleaseArgs),
    /// Print the release artifact path (for CI upload steps)
    ArtifactPath(BuildReleaseArgs),
    /// `cargo deny check`
    Deny,
    /// `cargo audit`
    Audit,
    /// Local smoke: fmt, clippy, tests, workspace debug build, deny, audit
    Smoke,
}

#[derive(Parser)]
pub struct TestArgs {
    #[arg(long)]
    pub target: Option<String>,
}

#[derive(Parser)]
pub struct BuildReleaseArgs {
    #[arg(long)]
    pub target: Option<String>,

    #[arg(long, default_value = "dist")]
    pub profile: String,
}

impl CiCommands {
    pub fn run(self) -> Result<()> {
        match self {
            Self::Fmt => run_fmt(),
            Self::Clippy => run_clippy(),
            Self::Test(args) => run_test(args),
            Self::BuildRelease(args) => run_build_release(args),
            Self::ArtifactPath(args) => run_artifact_path(args),
            Self::Deny => run_deny(),
            Self::Audit => run_audit(),
            Self::Smoke => run_smoke(),
        }
    }
}

fn run_fmt() -> Result<()> {
    let root = workspace_root()?;
    let status = cargo_command(&root)
        .args(["fmt", "--all", "--", "--check"])
        .status()
        .context("failed to run cargo fmt")?;
    if !status.success() {
        bail!("cargo fmt check failed");
    }
    Ok(())
}

fn run_clippy() -> Result<()> {
    let root = workspace_root()?;
    let status = cargo_command(&root)
        .env("RUSTFLAGS", CI_RUSTFLAGS)
        .args([
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ])
        .status()
        .context("failed to run cargo clippy")?;
    if !status.success() {
        bail!("cargo clippy failed");
    }
    Ok(())
}

fn run_test(args: TestArgs) -> Result<()> {
    let root = workspace_root()?;
    let mut cmd = cargo_command(&root);
    cmd.env("RUSTFLAGS", CI_RUSTFLAGS);
    cmd.args(["test", "--workspace"]);
    if let Some(target) = args.target {
        cmd.args(["--target", &target]);
    }
    let status = cmd.status().context("failed to run cargo test")?;
    if !status.success() {
        bail!("cargo test failed");
    }
    Ok(())
}

fn run_build_release(args: BuildReleaseArgs) -> Result<()> {
    let root = workspace_root()?;
    let mut cmd = cargo_command(&root);
    cmd.env("RUSTFLAGS", CI_RUSTFLAGS);
    cmd.args(["build", "-p", "wisp", "--profile", &args.profile]);
    if let Some(target) = args.target.as_deref() {
        cmd.args(["--target", target]);
    }
    let status = cmd.status().context("failed to run cargo build")?;
    if !status.success() {
        bail!("release build failed");
    }
    Ok(())
}

fn run_artifact_path(args: BuildReleaseArgs) -> Result<()> {
    let root = workspace_root()?;
    let target = args
        .target
        .or_else(default_host_target)
        .context("could not determine target triple")?;
    let path = release_binary_path(&root, &args.profile, &target)?;
    io::stdout().write_all(path.to_string_lossy().as_bytes())?;
    Ok(())
}

fn run_deny() -> Result<()> {
    ensure_cargo_plugin("deny")?;
    let root = workspace_root()?;
    let status = cargo_plugin_command(&root, "deny")?
        .args(["check"])
        .status()
        .context("failed to run cargo-deny")?;
    if !status.success() {
        bail!("cargo deny check failed");
    }
    Ok(())
}

fn run_audit() -> Result<()> {
    ensure_cargo_plugin("audit")?;
    let root = workspace_root()?;
    let status = cargo_plugin_command(&root, "audit")?
        .status()
        .context("failed to run cargo-audit")?;
    if !status.success() {
        bail!("cargo audit failed");
    }
    Ok(())
}

fn run_smoke() -> Result<()> {
    run_fmt()?;
    run_clippy()?;
    let root = workspace_root()?;
    let status = cargo_command(&root)
        .env("RUSTFLAGS", CI_RUSTFLAGS)
        .args(["build", "--workspace"])
        .status()
        .context("failed to run cargo build")?;
    if !status.success() {
        bail!("cargo build failed");
    }
    run_test(TestArgs { target: None })?;
    if cargo_plugin_available("deny") {
        run_deny()?;
    } else {
        eprintln!("note: skipping cargo deny (not installed)");
    }
    if cargo_plugin_available("audit") {
        run_audit()?;
    } else {
        eprintln!("note: skipping cargo audit (not installed)");
    }
    Ok(())
}

fn ensure_cargo_plugin(plugin: &str) -> Result<()> {
    if cargo_plugin_available(plugin) {
        Ok(())
    } else {
        bail!("`cargo-{plugin}` not found in PATH (install via `cargo install cargo-{plugin}`)");
    }
}

fn cargo_plugin_available(plugin: &str) -> bool {
    Command::new(format!("cargo-{plugin}"))
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn cargo_plugin_command(workspace_root: &Path, plugin: &str) -> Result<Command> {
    let mut cmd = Command::new(format!("cargo-{plugin}"));
    cmd.current_dir(workspace_root);
    Ok(cmd)
}
