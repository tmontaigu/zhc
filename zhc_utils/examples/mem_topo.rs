//! Dumps the detected hardware topology and its current availability.
//! Run via `make mem-topo`, or directly with `cargo run -p zhc_utils --example mem_topo`.
//! Under `taskset -c <range>` (or any cgroup cpuset restriction), the
//! availability section reflects that restriction.

use zhc_utils::Dumpable;
use zhc_utils::topology::Topology;

fn main() {
    let mut topo = Topology::detect_topology();
    println!("──── detected topology ────");
    println!("{}", topo.dump_to_string());

    topo.detect_availability();
    println!("\n──── availability ────");
    println!("completely available: {}", topo.is_completely_available());

    let unavailable_cpus: Vec<_> = topo
        .iter_all_processors()
        .filter(|p| !p.is_available())
        .map(|p| p.get_data().dump_to_string())
        .collect();
    let unavailable_mems: Vec<_> = topo
        .iter_all_memories()
        .filter(|m| !m.is_available())
        .map(|m| m.get_data().dump_to_string())
        .collect();

    if unavailable_cpus.is_empty() && unavailable_mems.is_empty() {
        println!("no restriction detected — every leaf is available");
    } else {
        println!("unavailable cpus: {unavailable_cpus:?}");
        println!("unavailable mems: {unavailable_mems:?}");
    }
}
