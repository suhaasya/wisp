//! Workspace automation (`cargo xtask …`).

mod budgets;
mod ci;
mod paths;

use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use flate2::{write::GzEncoder, Compression};

use paths::{cargo_command, default_host_target, release_binary_path, workspace_root};

#[derive(Parser)]
#[command(name = "xtask")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// CI helpers (fmt, clippy, tests, release build, supply-chain checks).
    #[command(subcommand)]
    Ci(ci::CiCommands),
    /// Build the app and report binary size plus per-crate breakdown.
    Size(SizeArgs),
    /// Resource budget gates (size + RAM scenarios).
    #[command(subcommand)]
    Budgets(budgets::BudgetsCommands),
}

#[derive(Parser)]
struct SizeArgs {
    /// Cargo profile (`release` or `dist`).
    #[arg(long, default_value = "release")]
    profile: String,

    /// Rust target triple (defaults to host).
    #[arg(long)]
    target: Option<String>,

    /// Skip `cargo build` (measure an existing artifact).
    #[arg(long)]
    no_build: bool,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Commands::Ci(cmd) => cmd.run(),
        Commands::Budgets(cmd) => cmd.run(),
        Commands::Size(args) => run_size(args),
    }
}

fn run_size(args: SizeArgs) -> Result<()> {
    ensure_cargo_subcommand(
        "bloat",
        "cargo install cargo-bloat --version 0.11.1 --locked",
    )?;

    let workspace_root = workspace_root()?;
    let target = args
        .target
        .clone()
        .or_else(default_host_target)
        .context("could not determine host target triple")?;

    if !args.no_build {
        build_app(&workspace_root, &args.profile, args.target.as_deref())?;
    }

    let binary = release_binary_path(&workspace_root, &args.profile, &target)?;
    if !binary.is_file() {
        bail!(
            "binary not found at {} (run without --no-build?)",
            binary.display()
        );
    }

    let bytes = fs::metadata(&binary)?.len();
    let gzip_bytes = gzip_size(&binary)?;

    println!("=== Wisp size report ===");
    println!("profile:  {}", args.profile);
    println!("target:   {target}");
    println!("binary:   {}", binary.display());
    println!(
        "stripped: {} bytes ({:.3} MiB)",
        bytes,
        bytes as f64 / (1024.0 * 1024.0)
    );
    println!(
        "gzip:     {} bytes ({:.3} MiB) — download budget proxy",
        gzip_bytes,
        gzip_bytes as f64 / (1024.0 * 1024.0)
    );
    println!();

    run_cargo_bloat(&workspace_root, &args.profile, args.target.as_deref())?;

    if cargo_subcommand_available("llvm-lines") {
        println!();
        println!("=== LLVM IR lines (cargo llvm-lines) ===");
        let mut llvm = cargo_command(&workspace_root);
        llvm.args(["llvm-lines", "-p", "wisp", "--profile", &args.profile]);
        if let Some(triple) = args.target.as_deref() {
            llvm.args(["--target", triple]);
        }
        let status = llvm.status().context("failed to run cargo llvm-lines")?;
        if !status.success() {
            bail!("cargo llvm-lines failed");
        }
    } else {
        eprintln!(
            "note: install cargo-llvm-lines for IR-level breakdown (requires Rust ≥ 1.89; `cargo install cargo-llvm-lines`)"
        );
    }

    Ok(())
}

fn build_app(workspace_root: &Path, profile: &str, target: Option<&str>) -> Result<()> {
    let mut cmd = cargo_command(workspace_root);
    cmd.args(["build", "-p", "wisp", "--profile", profile]);
    if let Some(triple) = target {
        cmd.args(["--target", triple]);
    }
    let status = cmd.status().context("failed to run cargo build")?;
    if !status.success() {
        bail!("cargo build failed");
    }
    Ok(())
}

fn run_cargo_bloat(workspace_root: &Path, profile: &str, target: Option<&str>) -> Result<()> {
    println!("=== Top 20 crates by size (cargo-bloat) ===");
    let mut cmd = cargo_command(workspace_root);
    cmd.args([
        "bloat",
        "--crates",
        "--split-std",
        "-n",
        "20",
        "-p",
        "wisp",
        "--profile",
        profile,
    ]);
    if let Some(triple) = target {
        cmd.args(["--target", triple]);
    }
    cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    let status = cmd.status().context("failed to run cargo bloat")?;
    if !status.success() {
        bail!("cargo bloat failed");
    }
    Ok(())
}

fn gzip_size(path: &Path) -> Result<u64> {
    let input = fs::read(path)?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&input)?;
    let out = encoder.finish()?;
    Ok(out.len() as u64)
}

fn ensure_cargo_subcommand(subcommand: &str, install_hint: &str) -> Result<()> {
    if cargo_subcommand_available(subcommand) {
        Ok(())
    } else {
        bail!("`cargo {subcommand}` not available. Install with: {install_hint}");
    }
}

fn cargo_subcommand_available(subcommand: &str) -> bool {
    Command::new("cargo")
        .args([subcommand, "--version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
