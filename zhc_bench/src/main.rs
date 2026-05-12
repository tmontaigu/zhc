use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde::{Deserialize, Serialize};
use zhc_builder::CiphertextSpec;
use zhc_pipeline::compat::Iop;
use zhc_sim::MHz;

const BITS: &[u16] = &[8, 16, 32, 64, 128];
const RESULTS_DIR: &str = "zhc_bench/results";
const SITE_DIR: &str = "zhc_bench/site";

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
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .expect("failed to get commit hash");
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

fn run_benchmarks() -> BenchResult {
    let config = zhc_sim::hpu::HpuConfig::default();
    let freq = MHz(400);
    let mut results: BTreeMap<String, BTreeMap<u16, f64>> = BTreeMap::new();

    for iop in Iop::ALL {
        let iop_name = format!("{:?}", iop);
        println!("Benchmarking {}", iop_name);
        let mut bits_results = BTreeMap::new();

        for &bits in BITS {
            let spec = CiphertextSpec::new(bits, 2, 2);
            let builder = iop.to_builder(spec);
            let latency = zhc_pipeline::compute_latency(&builder, config.clone(), freq);
            bits_results.insert(bits, latency);
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
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("run");

    match cmd {
        "run" => {
            check_git_clean();
            let result = run_benchmarks();
            save_result(&result);
            let all = load_all_results();
            generate_html(&all);
        }
        "site" => {
            let all = load_all_results();
            generate_html(&all);
        }
        _ => {
            eprintln!("Usage: zhc_bench [run|site]");
            eprintln!("  run  - Run benchmarks and regenerate site");
            eprintln!("  site - Regenerate site from existing results");
        }
    }
}
