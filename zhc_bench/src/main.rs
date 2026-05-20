use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write as _};
use std::path::PathBuf;
use std::process::Command;

use serde::{Deserialize, Serialize};
use zhc_builder::CiphertextSpec;
use zhc_pipeline::compat::Iop;
use zhc_sim::MHz;

const ALL_BITS: &[u16] = &[8, 16, 32, 64, 128];
const RESULTS_DIR: &str = "zhc_bench/results";
const SITE_DIR: &str = "zhc_bench/site";
const DELTA_COL_WIDTH: usize = 8;
const LATENCY_COL_WIDTH: usize = 12;
const DELTA_THRESHOLD: f64 = 0.5;

const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const RESET: &str = "\x1b[0m";

/// Parsed filter options from CLI arguments.
struct Filters {
    iops: Vec<Iop>,
    bits: Vec<u16>,
}

impl Filters {
    /// Parse filters from CLI args. Returns filters and remaining args.
    fn parse(args: &[String]) -> (Self, Vec<String>) {
        let mut iop_patterns: Vec<String> = vec![];
        let mut bit_values: Vec<u16> = vec![];
        let mut remaining = vec![];
        let mut iter = args.iter().peekable();

        while let Some(arg) = iter.next() {
            if arg == "-i" || arg == "--iops" {
                if let Some(val) = iter.next() {
                    iop_patterns.extend(val.split(',').map(|s| s.trim().to_lowercase()));
                }
            } else if let Some(val) = arg.strip_prefix("--iops=") {
                iop_patterns.extend(val.split(',').map(|s| s.trim().to_lowercase()));
            } else if arg == "-b" || arg == "--bits" {
                if let Some(val) = iter.next() {
                    bit_values.extend(val.split(',').filter_map(|s| s.trim().parse::<u16>().ok()));
                }
            } else if let Some(val) = arg.strip_prefix("--bits=") {
                bit_values.extend(val.split(',').filter_map(|s| s.trim().parse::<u16>().ok()));
            } else {
                remaining.push(arg.clone());
            }
        }

        // Filter iops by case-insensitive substring match
        let iops: Vec<Iop> = if iop_patterns.is_empty() {
            Iop::ALL.to_vec()
        } else {
            Iop::ALL
                .iter()
                .filter(|iop| {
                    let name = format!("{:?}", iop).to_lowercase();
                    iop_patterns.iter().any(|p| name.contains(p))
                })
                .cloned()
                .collect()
        };

        // Filter bits, defaulting to all if none specified
        let bits: Vec<u16> = if bit_values.is_empty() {
            ALL_BITS.to_vec()
        } else {
            ALL_BITS
                .iter()
                .filter(|b| bit_values.contains(b))
                .copied()
                .collect()
        };

        (Self { iops, bits }, remaining)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BenchResult {
    commit: String,
    timestamp: String,
    results: BTreeMap<String, BTreeMap<u16, f64>>, // iop -> bits -> latency_us
}

fn get_commit_hash() -> String {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("failed to get commit hash");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn get_commit_short() -> String {
    resolve_rev_short("HEAD")
}

fn resolve_rev_short(rev: &str) -> String {
    let output = Command::new("git")
        .args(["rev-parse", "--short", rev])
        .output()
        .expect("failed to resolve revision");
    if !output.status.success() {
        eprintln!("Error: unknown revision '{}'", rev);
        std::process::exit(1);
    }
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn get_timestamp() -> String {
    let output = Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .expect("failed to get timestamp");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn check_git_clean() {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .expect("failed to check git status");
    let status = String::from_utf8_lossy(&output.stdout);
    if !status.trim().is_empty() {
        eprintln!("Error: git tree is dirty. Commit your changes before running benchmarks.");
        std::process::exit(1);
    }
}

fn bench_iop(
    iop: &Iop,
    config: &zhc_sim::hpu::HpuConfig,
    freq: MHz,
    bits_filter: &[u16],
) -> BTreeMap<u16, f64> {
    let mut bits_results = BTreeMap::new();
    for &bits in bits_filter {
        let spec = CiphertextSpec::new(bits, 2, 2);
        let builder = iop.to_builder(spec);
        let latency = zhc_pipeline::compute_latency(&builder, config.clone(), freq);
        bits_results.insert(bits, latency);
    }
    bits_results
}

fn run_benchmarks() -> BenchResult {
    let config = zhc_sim::hpu::HpuConfig::default();
    let freq = MHz(400);
    let mut results: BTreeMap<String, BTreeMap<u16, f64>> = BTreeMap::new();

    for iop in Iop::ALL {
        let iop_name = format!("{:?}", iop);
        println!("Benchmarking {}", iop_name);
        let bits_results = bench_iop(iop, &config, freq, ALL_BITS);
        for (&bits, &latency) in &bits_results {
            println!("  {}b: {:.2}us", bits, latency);
        }
        results.insert(iop_name, bits_results);
    }

    BenchResult {
        commit: get_commit_hash(),
        timestamp: get_timestamp(),
        results,
    }
}

fn save_result(result: &BenchResult) {
    let dir = PathBuf::from(RESULTS_DIR);
    fs::create_dir_all(&dir).expect("failed to create results dir");

    let filename = format!("{}.json", get_commit_short());
    let path = dir.join(filename);

    let json = serde_json::to_string_pretty(result).expect("failed to serialize");
    fs::write(&path, json).expect("failed to write result");
    println!("Saved results to {}", path.display());
}

fn load_all_results() -> Vec<BenchResult> {
    let dir = PathBuf::from(RESULTS_DIR);
    if !dir.exists() {
        return vec![];
    }

    let mut results = vec![];
    for entry in fs::read_dir(&dir).expect("failed to read results dir") {
        let entry = entry.expect("failed to read entry");
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json") {
            let content = fs::read_to_string(&path).expect("failed to read file");
            let result: BenchResult = serde_json::from_str(&content).expect("failed to parse");
            results.push(result);
        }
    }

    results.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    results
}

fn load_result_by_rev(rev: &str) -> Option<BenchResult> {
    let short = resolve_rev_short(rev);
    let path = PathBuf::from(RESULTS_DIR).join(format!("{}.json", short));
    if !path.exists() {
        return None;
    }
    let content = fs::read_to_string(&path).expect("failed to read file");
    Some(serde_json::from_str(&content).expect("failed to parse"))
}

fn find_latest_baseline(exclude_commit: &str) -> Option<BenchResult> {
    let results = load_all_results();
    results
        .into_iter()
        .rev()
        .find(|r| !r.commit.starts_with(exclude_commit) && !exclude_commit.starts_with(&r.commit))
}

fn list_available_baselines() -> Vec<String> {
    let dir = PathBuf::from(RESULTS_DIR);
    if !dir.exists() {
        return vec![];
    }
    fs::read_dir(&dir)
        .expect("failed to read results dir")
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            path.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        })
        .collect()
}

fn format_latency(us: f64) -> String {
    let int_part = us.round() as u64;
    let int_str = int_part
        .to_string()
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(|chunk| std::str::from_utf8(chunk).unwrap())
        .collect::<Vec<_>>()
        .join(" ");
    format!("{} µs", int_str)
}

fn format_latency_cell(latency: Option<&f64>) -> String {
    match latency {
        Some(&us) => {
            let formatted = format_latency(us);
            format!("{:>w$}", formatted, w = LATENCY_COL_WIDTH)
        }
        None => format!("{:>w$}", "-", w = LATENCY_COL_WIDTH),
    }
}

fn format_delta(pct: f64, use_color: bool) -> String {
    let sign = if pct >= 0.0 { "+" } else { "" };
    let text = format!("{}{:.1}%", sign, pct);
    if !use_color || pct.abs() < DELTA_THRESHOLD {
        return text;
    }
    if pct > 0.0 {
        format!("{}{}{}", RED, text, RESET)
    } else {
        format!("{}{}{}", GREEN, text, RESET)
    }
}

fn format_diff_cell(curr: Option<&f64>, base: Option<&f64>, use_color: bool) -> String {
    match (curr, base) {
        (Some(c), Some(b)) if *b != 0.0 => {
            let pct = (c - b) / b * 100.0;
            let formatted = format_delta(pct, use_color);
            let visible_len = if pct.abs() >= DELTA_THRESHOLD && use_color {
                formatted.len() - RED.len() - RESET.len()
            } else {
                formatted.len()
            };
            let pad = DELTA_COL_WIDTH.saturating_sub(visible_len);
            let left = pad / 2;
            let right = pad - left;
            format!("{:l$}{}{:r$}", "", formatted, "", l = left, r = right)
        }
        _ => format!("{:^w$}", "-", w = DELTA_COL_WIDTH),
    }
}

fn run_diff_incremental(baseline: &BenchResult, use_color: bool, filters: &Filters) {
    let baseline_short = &baseline.commit[..7.min(baseline.commit.len())];
    let baseline_date = &baseline.timestamp[..10.min(baseline.timestamp.len())];
    println!("vs {} ({})\n", baseline_short, baseline_date);

    let iop_names: Vec<_> = filters
        .iops
        .iter()
        .map(|iop| format!("{:?}", iop))
        .collect();
    let op_width = iop_names.iter().map(|s| s.len()).max().unwrap_or(9).max(9) + 2;

    // Top border
    print!("┌{:─<w$}", "", w = op_width);
    for _ in &filters.bits {
        print!("┬{:─<w$}", "", w = DELTA_COL_WIDTH);
    }
    println!("┐");

    // Header
    print!("│{:^w$}", "Operation", w = op_width);
    for bits in &filters.bits {
        print!("│{:^w$}", format!("{}b", bits), w = DELTA_COL_WIDTH);
    }
    println!("│");

    // Header separator
    print!("├{:─<w$}", "", w = op_width);
    for _ in &filters.bits {
        print!("┼{:─<w$}", "", w = DELTA_COL_WIDTH);
    }
    println!("┤");

    let config = zhc_sim::hpu::HpuConfig::default();
    let freq = MHz(400);

    // Data rows - benchmark and print each row as it completes
    for iop in &filters.iops {
        let iop_name = format!("{:?}", iop);
        print!("│ {:<w$}", iop_name, w = op_width - 1);
        io::stdout().flush().unwrap();

        let bits_results = bench_iop(iop, &config, freq, &filters.bits);

        for bits in &filters.bits {
            let curr = bits_results.get(bits);
            let base = baseline.results.get(&iop_name).and_then(|m| m.get(bits));
            let cell = format_diff_cell(curr, base, use_color);
            print!("│{}", cell);
            io::stdout().flush().unwrap();
        }
        println!("│");
    }

    // Bottom border
    print!("└{:─<w$}", "", w = op_width);
    for _ in &filters.bits {
        print!("┴{:─<w$}", "", w = DELTA_COL_WIDTH);
    }
    println!("┘");
}

fn run_latency_table(filters: &Filters) {
    let iop_names: Vec<_> = filters
        .iops
        .iter()
        .map(|iop| format!("{:?}", iop))
        .collect();
    let op_width = iop_names.iter().map(|s| s.len()).max().unwrap_or(9).max(9) + 2;

    print!("┌{:─<w$}", "", w = op_width);
    for _ in &filters.bits {
        print!("┬{:─<w$}", "", w = LATENCY_COL_WIDTH);
    }
    println!("┐");

    print!("│{:^w$}", "Operation", w = op_width);
    for bits in &filters.bits {
        print!("│{:^w$}", format!("{}b", bits), w = LATENCY_COL_WIDTH);
    }
    println!("│");

    print!("├{:─<w$}", "", w = op_width);
    for _ in &filters.bits {
        print!("┼{:─<w$}", "", w = LATENCY_COL_WIDTH);
    }
    println!("┤");

    let config = zhc_sim::hpu::HpuConfig::default();
    let freq = MHz(400);

    for iop in &filters.iops {
        let iop_name = format!("{:?}", iop);
        print!("│ {:<w$}", iop_name, w = op_width - 1);
        io::stdout().flush().unwrap();

        let bits_results = bench_iop(iop, &config, freq, &filters.bits);

        for bits in &filters.bits {
            let cell = format_latency_cell(bits_results.get(bits));
            print!("│{}", cell);
            io::stdout().flush().unwrap();
        }
        println!("│");
    }

    print!("└{:─<w$}", "", w = op_width);
    for _ in &filters.bits {
        print!("┴{:─<w$}", "", w = LATENCY_COL_WIDTH);
    }
    println!("┘");
}

fn generate_html(results: &[BenchResult]) {
    let dir = PathBuf::from(SITE_DIR);
    fs::create_dir_all(&dir).expect("failed to create site dir");

    let data_json = serde_json::to_string(results).expect("failed to serialize");

    let html = format!(
        r##"<!DOCTYPE html>
<html>
<head>
    <title>ZHC Benchmark Results</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
    <style>
        body {{ font-family: system-ui, sans-serif; margin: 2rem; background: #1a1a2e; color: #eee; }}
        h1 {{ color: #00d4ff; }}
        .charts {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(500px, 1fr)); gap: 2rem; }}
        .chart-container {{ background: #16213e; padding: 1rem; border-radius: 8px; }}
        canvas {{ max-height: 300px; }}
        table {{ border-collapse: collapse; width: 100%; margin-top: 2rem; }}
        th, td {{ border: 1px solid #333; padding: 8px; text-align: right; }}
        th {{ background: #16213e; }}
        tr:nth-child(even) {{ background: #1a1a2e; }}
        tr:nth-child(odd) {{ background: #16213e; }}
    </style>
</head>
<body>
    <h1>ZHC Benchmark Results</h1>
    <div class="charts" id="charts"></div>
    <h2>Latest Results (μs)</h2>
    <div id="table"></div>
    <script>
        const DATA = {data_json};
        const BITS = [8, 16, 32, 64, 128];
        const COLORS = ['#ff6384', '#36a2eb', '#ffce56', '#4bc0c0', '#9966ff'];

        // Get all IOPs from the latest result
        const iops = DATA.length > 0 ? Object.keys(DATA[DATA.length - 1].results) : [];

        // Create a chart for each IOP
        const chartsDiv = document.getElementById('charts');
        iops.forEach(iop => {{
            const container = document.createElement('div');
            container.className = 'chart-container';
            container.innerHTML = `<canvas id="chart-${{iop}}"></canvas>`;
            chartsDiv.appendChild(container);

            const ctx = document.getElementById(`chart-${{iop}}`).getContext('2d');
            const datasets = BITS.map((bits, i) => ({{
                label: `${{bits}}b`,
                data: DATA.map(r => r.results[iop]?.[bits] ?? null),
                borderColor: COLORS[i],
                tension: 0.1,
                fill: false,
            }}));

            new Chart(ctx, {{
                type: 'line',
                data: {{
                    labels: DATA.map(r => r.commit.slice(0, 7)),
                    datasets,
                }},
                options: {{
                    responsive: true,
                    plugins: {{
                        title: {{ display: true, text: iop, color: '#00d4ff' }},
                        legend: {{ labels: {{ color: '#eee' }} }},
                    }},
                    scales: {{
                        x: {{ ticks: {{ color: '#aaa' }}, grid: {{ color: '#333' }} }},
                        y: {{
                            ticks: {{ color: '#aaa' }},
                            grid: {{ color: '#333' }},
                            title: {{ display: true, text: 'Latency (μs)', color: '#aaa' }}
                        }},
                    }},
                }},
            }});
        }});

        // Format number with space as thousand separator
        function fmt(val) {{
            const [int, dec] = val.toFixed(2).split('.');
            const spaced = int.replace(/\B(?=(\d{{3}})+(?!\d))/g, ' ');
            return spaced + '.' + dec;
        }}

        // Generate table with latest results
        if (DATA.length > 0) {{
            const latest = DATA[DATA.length - 1];
            let html = '<table><tr><th>Operation</th>';
            BITS.forEach(b => html += `<th>${{b}}b</th>`);
            html += '</tr>';
            iops.forEach(iop => {{
                html += `<tr><td style="text-align:left">${{iop}}</td>`;
                BITS.forEach(b => {{
                    const val = latest.results[iop]?.[b];
                    html += `<td>${{val ? fmt(val) : '-'}}</td>`;
                }});
                html += '</tr>';
            }});
            html += '</table>';
            document.getElementById('table').innerHTML = html;
        }}
    </script>
</body>
</html>
"##
    );

    let path = dir.join("index.html");
    fs::write(&path, html).expect("failed to write html");
    println!("Generated {}", path.display());
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (filters, remaining) = Filters::parse(&args[1..]);
    let cmd = remaining.first().map(|s| s.as_str()).unwrap_or("run");

    if filters.iops.is_empty() {
        eprintln!("Error: no iops match the filter");
        std::process::exit(1);
    }
    if filters.bits.is_empty() {
        eprintln!("Error: no bits match the filter");
        std::process::exit(1);
    }

    match cmd {
        "run" => {
            run_latency_table(&filters);
        }
        "export" => {
            check_git_clean();
            let result = run_benchmarks();
            save_result(&result);
            let all = load_all_results();
            generate_html(&all);
        }
        "diff" => {
            let use_color = !remaining.iter().any(|a| a == "--no-color");
            let rev_arg = remaining.iter().skip(1).find(|a| !a.starts_with("--"));
            let baseline = if let Some(rev) = rev_arg {
                match load_result_by_rev(rev) {
                    Some(b) => b,
                    None => {
                        let available = list_available_baselines();
                        eprintln!("Error: no saved results for '{}'", rev);
                        if available.is_empty() {
                            eprintln!("No baselines available. Run 'zhc_bench export' first.");
                        } else {
                            eprintln!("Available baselines: {}", available.join(", "));
                        }
                        std::process::exit(1);
                    }
                }
            } else {
                let current_short = get_commit_short();
                match find_latest_baseline(&current_short) {
                    Some(b) => b,
                    None => {
                        eprintln!("Error: no baseline found.");
                        eprintln!("Run 'zhc_bench export' on a commit first.");
                        std::process::exit(1);
                    }
                }
            };
            run_diff_incremental(&baseline, use_color, &filters);
        }
        _ => {
            eprintln!("Usage: zhc_bench [run|export|diff] [OPTIONS]");
            eprintln!();
            eprintln!("Commands:");
            eprintln!("  run                     - Run benchmarks and display latency table");
            eprintln!(
                "  export                  - Run benchmarks, save results, and regenerate site"
            );
            eprintln!(
                "  diff [REV] [--no-color] - Compare current against REV (default: latest baseline)"
            );
            eprintln!();
            eprintln!("Filter options (for run and diff):");
            eprintln!(
                "  -i, --iops=PATTERNS     - Comma-separated iop name patterns (case-insensitive substring match)"
            );
            eprintln!("  -b, --bits=VALUES       - Comma-separated bit widths (8,16,32,64,128)");
            eprintln!();
            eprintln!("Examples:");
            eprintln!("  zhc_bench run -i mul,div -b 8,16");
            eprintln!("  zhc_bench diff --iops=cmp --bits=64");
        }
    }
}
