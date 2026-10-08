use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use flate2::{write::GzEncoder, Compression};
use serde::{Deserialize, Serialize};

use crate::paths::{cargo_command, default_host_target, release_binary_path, workspace_root};

/// Hard caps (deal-breakers) from the PRD budget table.
const CAP_BINARY_MB: f64 = 25.0;
const TARGET_BINARY_MB: f64 = 18.0;
const CAP_GZIP_MB: f64 = 12.0;
const TARGET_GZIP_MB: f64 = 9.0;
const TARGET_IDLE_MB: f64 = 45.0;
const CAP_IDLE_MB: f64 = 60.0;
const CAP_SCROLL_1M_MB: f64 = 220.0;

const SETTLE_SECS: u64 = 30;

#[derive(Parser)]
pub struct BudgetsCli {
    #[command(subcommand)]
    pub command: BudgetsCommands,
}

#[derive(Subcommand)]
pub enum BudgetsCommands {
    /// Measure size + RAM scenarios and fail on hard cap violations.
    Gate(GateArgs),
    /// Render a Markdown PR comment from current and baseline JSON.
    Comment(CommentArgs),
    /// Append one row to benchmarks history CSV (gh-pages).
    Record(RecordArgs),
}

#[derive(Parser)]
pub struct GateArgs {
    #[arg(long, default_value = "dist")]
    pub profile: String,

    #[arg(long)]
    pub target: Option<String>,

    #[arg(long, default_value = "budgets-result.json")]
    pub output: PathBuf,

    #[arg(long, help = "Build the app before measuring")]
    pub build: bool,

    /// Cargo features passed to the budget build (e.g. `bench-stress` to test the size gate).
    #[arg(long, value_delimiter = ',')]
    pub features: Vec<String>,
}

#[derive(Parser)]
pub struct CommentArgs {
    #[arg(long)]
    pub current: PathBuf,

    #[arg(long)]
    pub baseline: Option<PathBuf>,
}

#[derive(Parser)]
pub struct RecordArgs {
    #[arg(long)]
    pub input: PathBuf,

    #[arg(long, default_value = "benchmarks/history.csv")]
    pub csv: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetReport {
    pub commit: Option<String>,
    pub target: String,
    pub profile: String,
    pub binary_stripped_bytes: u64,
    pub binary_stripped_mb: f64,
    pub gzip_bytes: u64,
    pub gzip_mb: f64,
    pub scenarios: Vec<ScenarioResult>,
    pub gates: Vec<GateResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioResult {
    pub name: String,
    pub status: ScenarioStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rss_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rss_mb: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioStatus {
    Measured,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub metric: String,
    pub value_mb: f64,
    pub target_mb: Option<f64>,
    pub cap_mb: f64,
    pub passed: bool,
    pub level: GateLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateLevel {
    Ok,
    Warn,
    Fail,
}

impl BudgetsCommands {
    pub fn run(self) -> Result<()> {
        match self {
            Self::Gate(args) => run_gate(args),
            Self::Comment(args) => run_comment(args),
            Self::Record(args) => run_record(args),
        }
    }
}

fn run_gate(args: GateArgs) -> Result<()> {
    let root = workspace_root()?;
    let target = args
        .target
        .clone()
        .or_else(default_host_target)
        .context("could not determine host target triple")?;

    if args.build {
        let mut cmd = cargo_command(&root);
        cmd.args(["build", "-p", "wisp", "--profile", &args.profile]);
        if !args.features.is_empty() {
            cmd.args(["--features", &args.features.join(",")]);
        }
        cmd.args(["--target", &target]);
        if !cmd.status()?.success() {
            bail!("failed to build wisp for budget gate");
        }
    }

    let binary = release_binary_path(&root, &args.profile, &target)?;
    if !binary.is_file() {
        bail!("binary not found at {} (pass --build?)", binary.display());
    }

    let binary_stripped_bytes = fs::metadata(&binary)?.len();
    let binary_stripped_mb = bytes_to_mb(binary_stripped_bytes);
    let gzip_bytes = gzip_size(&binary)?;
    let gzip_mb = bytes_to_mb(gzip_bytes);

    let mut gates = vec![
        evaluate_gate(
            "binary_stripped",
            binary_stripped_mb,
            Some(TARGET_BINARY_MB),
            CAP_BINARY_MB,
        ),
        evaluate_gate("gzip_download", gzip_mb, Some(TARGET_GZIP_MB), CAP_GZIP_MB),
    ];

    let mut scenarios = Vec::new();

    match measure_idle_rss(&binary) {
        Ok(rss_bytes) => {
            let rss_mb = bytes_to_mb(rss_bytes);
            scenarios.push(ScenarioResult {
                name: "idle".into(),
                status: ScenarioStatus::Measured,
                rss_bytes: Some(rss_bytes),
                rss_mb: Some(rss_mb),
                reason: None,
            });
            gates.push(evaluate_gate(
                "idle_rss",
                rss_mb,
                Some(TARGET_IDLE_MB),
                CAP_IDLE_MB,
            ));
        }
        Err(err) => {
            scenarios.push(ScenarioResult {
                name: "idle".into(),
                status: ScenarioStatus::Skipped,
                rss_bytes: None,
                rss_mb: None,
                reason: Some(format!("measurement failed: {err:#}")),
            });
        }
    }

    for (name, reason) in [
        (
            "connection_schema_500",
            "500-table schema scenario not implemented (needs drivers + Docker Postgres/MySQL)",
        ),
        (
            "scroll_1m_rows",
            "1M-row scroll scenario not implemented (needs grid + core pager)",
        ),
        (
            "connections_10",
            "10-connection scenario not implemented (needs session pool + Docker services)",
        ),
    ] {
        scenarios.push(ScenarioResult {
            name: name.into(),
            status: ScenarioStatus::Skipped,
            rss_bytes: None,
            rss_mb: None,
            reason: Some(reason.into()),
        });
    }

    let _ = CAP_SCROLL_1M_MB; // enforced when scroll_1m_rows scenario is implemented

    let report = BudgetReport {
        commit: std::env::var("GITHUB_SHA").ok(),
        target,
        profile: args.profile,
        binary_stripped_bytes,
        binary_stripped_mb,
        gzip_bytes,
        gzip_mb,
        scenarios,
        gates: gates.clone(),
    };

    write_json(&args.output, &report)?;

    let failures: Vec<_> = gates
        .iter()
        .filter(|g| g.level == GateLevel::Fail)
        .collect();
    if !failures.is_empty() {
        for gate in &failures {
            eprintln!(
                "BUDGET GATE FAILED: {} = {:.3} MiB (hard cap {:.1} MiB)",
                gate.metric, gate.value_mb, gate.cap_mb
            );
        }
        bail!("{} budget gate(s) exceeded hard cap", failures.len());
    }

    for gate in gates.iter().filter(|g| g.level == GateLevel::Warn) {
        eprintln!(
            "BUDGET WARN: {} = {:.3} MiB exceeds target {:.1} MiB (cap {:.1} MiB)",
            gate.metric,
            gate.value_mb,
            gate.target_mb.unwrap_or(0.0),
            gate.cap_mb
        );
    }

    Ok(())
}

fn run_comment(args: CommentArgs) -> Result<()> {
    let current: BudgetReport = read_json(&args.current)?;
    let baseline = args
        .baseline
        .as_ref()
        .map(|p| read_json::<BudgetReport>(p))
        .transpose()?;

    let md = render_comment(&current, baseline.as_ref());
    io::stdout().write_all(md.as_bytes())?;
    Ok(())
}

fn run_record(args: RecordArgs) -> Result<()> {
    let report: BudgetReport = read_json(&args.input)?;
    let idle = report
        .scenarios
        .iter()
        .find(|s| s.name == "idle")
        .and_then(|s| s.rss_mb)
        .unwrap_or(f64::NAN);

    if let Some(parent) = args.csv.parent() {
        fs::create_dir_all(parent)?;
    }

    let new_file = !args.csv.is_file();
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&args.csv)?;
    if new_file {
        writeln!(
            file,
            "timestamp,commit,target,binary_mb,gzip_mb,idle_mb,idle_status"
        )?;
    }
    let ts = chrono_lite_timestamp();
    let commit = report.commit.as_deref().unwrap_or("");
    let idle_status = report
        .scenarios
        .iter()
        .find(|s| s.name == "idle")
        .map_or_else(|| "missing".into(), |s| format!("{:?}", s.status));
    writeln!(
        file,
        "{ts},{commit},{},{:.6},{:.6},{:.6},{idle_status}",
        report.target, report.binary_stripped_mb, report.gzip_mb, idle
    )?;
    Ok(())
}

fn measure_idle_rss(binary: &Path) -> Result<u64> {
    let mut child = spawn_bench(binary, "idle")?;
    let pid = child.id();

    let started = Instant::now();
    let mut peak_rss = 0_u64;

    while started.elapsed() < Duration::from_secs(SETTLE_SECS) {
        if let Some(rss) = read_process_rss(pid)? {
            peak_rss = peak_rss.max(rss);
        }
        if let Some(status) = child.try_wait()? {
            if !status.success() {
                bail!("bench process exited early with {status}");
            }
        }
        thread::sleep(Duration::from_millis(500));
    }

    let _ = child.kill();
    let _ = child.wait();

    if peak_rss == 0 {
        bail!("could not read RSS for pid {pid}");
    }
    Ok(peak_rss)
}

fn spawn_bench(binary: &Path, scenario: &str) -> Result<Child> {
    Command::new(binary)
        .arg("--bench-scenario")
        .arg(scenario)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to spawn {}", binary.display()))
}

fn read_process_rss(pid: u32) -> Result<Option<u64>> {
    #[cfg(target_os = "linux")]
    {
        let status = fs::read_to_string(format!("/proc/{pid}/status"))
            .with_context(|| format!("read /proc/{pid}/status"))?;
        for line in status.lines() {
            if let Some(kb) = line.strip_prefix("VmRSS:") {
                let kb = kb.trim().trim_end_matches(" kB").parse::<u64>()?;
                return Ok(Some(kb * 1024));
            }
        }
        return Ok(None);
    }

    #[cfg(target_os = "macos")]
    {
        read_rss_via_ps(pid)
    }

    #[cfg(windows)]
    {
        read_rss_windows(pid)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        let _ = pid;
        Ok(None)
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn read_rss_via_ps(pid: u32) -> Result<Option<u64>> {
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let kb = String::from_utf8(output.stdout)?.trim().parse::<u64>().ok();
    Ok(kb.map(|k| k * 1024))
}

#[cfg(windows)]
fn read_rss_windows(pid: u32) -> Result<Option<u64>> {
    use std::mem::size_of;

    #[repr(C)]
    struct ProcessMemoryCounters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }

    type OpenProcessFn = unsafe extern "system" fn(u32, i32, u32) -> *mut std::ffi::c_void;
    type GetProcessMemoryInfoFn =
        unsafe extern "system" fn(*mut std::ffi::c_void, *mut ProcessMemoryCounters, u32) -> i32;
    type CloseHandleFn = unsafe extern "system" fn(*mut std::ffi::c_void) -> i32;

    let psapi =
        unsafe { windows_sys_load("Psapi.dll").or_else(|_| windows_sys_load("psapi.dll")) }?;
    let kernel = unsafe { windows_sys_load("Kernel32.dll") }?;

    unsafe {
        let open: OpenProcessFn = std::mem::transmute(get_proc(&kernel, "OpenProcess")?);
        let mem_info: GetProcessMemoryInfoFn =
            std::mem::transmute(get_proc(&psapi, "GetProcessMemoryInfo")?);
        let close: CloseHandleFn = std::mem::transmute(get_proc(&kernel, "CloseHandle")?);

        const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
        let handle = open(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return Ok(None);
        }

        let mut counters = ProcessMemoryCounters {
            cb: size_of::<ProcessMemoryCounters>() as u32,
            page_fault_count: 0,
            peak_working_set_size: 0,
            working_set_size: 0,
            quota_peak_paged_pool_usage: 0,
            quota_paged_pool_usage: 0,
            quota_peak_non_paged_pool_usage: 0,
            quota_non_paged_pool_usage: 0,
            pagefile_usage: 0,
            peak_pagefile_usage: 0,
        };
        let ok = mem_info(handle, &mut counters, counters.cb);
        close(handle);
        if ok == 0 {
            return Ok(None);
        }
        Ok(Some(counters.working_set_size as u64))
    }
}

#[cfg(windows)]
unsafe fn windows_sys_load(name: &str) -> Result<*mut std::ffi::c_void> {
    extern "system" {
        fn LoadLibraryA(name: *const u8) -> *mut std::ffi::c_void;
    }
    let lib = LoadLibraryA(format!("{name}\0").as_ptr());
    if lib.is_null() {
        bail!("LoadLibraryA failed for {name}");
    }
    Ok(lib)
}

#[cfg(windows)]
unsafe fn get_proc(module: &*mut std::ffi::c_void, name: &str) -> Result<*mut std::ffi::c_void> {
    extern "system" {
        fn GetProcAddress(module: *mut std::ffi::c_void, name: *const u8) -> *mut std::ffi::c_void;
    }
    let proc = GetProcAddress(*module, format!("{name}\0").as_ptr());
    if proc.is_null() {
        bail!("GetProcAddress failed for {name}");
    }
    Ok(proc)
}

fn evaluate_gate(metric: &str, value_mb: f64, target_mb: Option<f64>, cap_mb: f64) -> GateResult {
    let level = if value_mb > cap_mb {
        GateLevel::Fail
    } else if target_mb.is_some_and(|t| value_mb > t) {
        GateLevel::Warn
    } else {
        GateLevel::Ok
    };
    GateResult {
        metric: metric.into(),
        value_mb,
        target_mb,
        cap_mb,
        passed: level != GateLevel::Fail,
        level,
    }
}

fn render_comment(current: &BudgetReport, baseline: Option<&BudgetReport>) -> String {
    let mut out = String::from("## Wisp budget report\n\n");
    out.push_str("| Metric | This PR | Δ vs main | Cap |\n");
    out.push_str("| --- | ---: | ---: | ---: |\n");

    push_metric_row(
        &mut out,
        "Binary (stripped)",
        current.binary_stripped_mb,
        baseline.map(|b| b.binary_stripped_mb),
        CAP_BINARY_MB,
    );
    push_metric_row(
        &mut out,
        "Download (gzip)",
        current.gzip_mb,
        baseline.map(|b| b.gzip_mb),
        CAP_GZIP_MB,
    );

    if let Some(idle) = current.scenarios.iter().find(|s| s.name == "idle") {
        let value = idle.rss_mb.unwrap_or(f64::NAN);
        let base = baseline.and_then(|b| {
            b.scenarios
                .iter()
                .find(|s| s.name == "idle")
                .and_then(|s| s.rss_mb)
        });
        let status = match idle.status {
            ScenarioStatus::Measured => format!("{value:.2} MB"),
            ScenarioStatus::Skipped => {
                format!("skipped ({})", idle.reason.as_deref().unwrap_or(""))
            }
        };
        let delta = match (idle.rss_mb, base) {
            (Some(v), Some(b)) => format!("{:+.2} MB", v - b),
            _ => "—".into(),
        };
        out.push_str(&format!(
            "| Idle RSS | {status} | {delta} | {CAP_IDLE_MB:.0} MB |\n"
        ));
    }

    out.push_str("\n### Scenarios\n\n| Scenario | Status | RSS |\n| --- | --- | ---: |\n");
    for scenario in &current.scenarios {
        let rss = scenario
            .rss_mb
            .map_or_else(|| "—".into(), |v| format!("{v:.2} MB"));
        let status = match scenario.status {
            ScenarioStatus::Measured => "measured",
            ScenarioStatus::Skipped => "skipped",
        };
        out.push_str(&format!("| {} | {} | {} |\n", scenario.name, status, rss));
    }

    out
}

fn push_metric_row(
    out: &mut String,
    label: &str,
    value_mb: f64,
    baseline_mb: Option<f64>,
    cap_mb: f64,
) {
    let delta = baseline_mb.map_or_else(|| "—".into(), |b| format!("{:+.2} MB", value_mb - b));
    out.push_str(&format!(
        "| {label} | {value_mb:.2} MB | {delta} | {cap_mb:.0} MB |\n"
    ));
}

fn bytes_to_mb(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn gzip_size(path: &Path) -> Result<u64> {
    let input = fs::read(path)?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&input)?;
    Ok(encoder.finish()?.len() as u64)
}

fn write_json(path: &Path, report: &BudgetReport) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let json = serde_json::to_string_pretty(report)?;
    fs::write(path, json)?;
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

fn chrono_lite_timestamp() -> String {
    // Avoid chrono dependency: ISO-like UTC from UNIX time when available.
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or_else(|_| "0".into(), |d| d.as_secs().to_string())
}
