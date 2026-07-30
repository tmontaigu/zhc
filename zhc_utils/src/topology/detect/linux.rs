use std::collections::HashSet;

use super::parse::{
    is_data_or_unified_cache_type, parse_cache_level, parse_cache_size, parse_cpuinfo_max_freq,
    parse_cpulist, parse_mem_total_kb, parse_physical_package_id, parse_status_field,
};
use crate::topology::{
    HardwareData, HardwareDataKind, MemId, MemoryData, ProcId, ProcessingData, Space, TopoId,
    Topology,
};

const CPU_BASE: &str = "/sys/devices/system/cpu";
const NODE_BASE: &str = "/sys/devices/system/node";

pub(super) fn detect() -> Topology {
    let hostname = std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "machine".to_string());

    let mut topo = Topology::with_toplevel(HardwareData {
        name: hostname,
        kind: HardwareDataKind::Machine,
    });
    // `with_toplevel` always creates the root at index 0.
    let root = TopoId(0);

    let cpus = logical_cpus();
    if cpus.is_empty() {
        return topo;
    }

    let mut sockets: Vec<(usize, Vec<ProcId>)> = Vec::new();
    for cpu in cpus {
        let pkg = physical_package_id(cpu).unwrap_or(0);
        match sockets.iter_mut().find(|entry| entry.0 == pkg) {
            Some(entry) => entry.1.push(cpu),
            None => sockets.push((pkg, vec![cpu])),
        }
    }

    let nodes = numa_nodes();
    let mut any_memory_attached = false;
    let mut first_socket = None;

    for (pkg, socket_cpus) in sockets {
        let socket_id = topo.add_hardware(
            HardwareData {
                name: format!("socket{pkg}"),
                kind: HardwareDataKind::Socket,
            },
            root,
        );
        first_socket.get_or_insert(socket_id);

        // NUMA nodes fully contained in this socket become locality Groups.
        let socket_set: HashSet<ProcId> = socket_cpus.iter().copied().collect();
        let mut covered: HashSet<ProcId> = HashSet::new();

        for (node_id, node_cpus) in &nodes {
            if node_cpus.is_empty() || !node_cpus.iter().all(|c| socket_set.contains(c)) {
                continue;
            }
            let group_id = topo.add_hardware(
                HardwareData {
                    name: format!("node{}", node_id.0),
                    kind: HardwareDataKind::Group,
                },
                socket_id,
            );
            topo.add_memory(
                MemoryData {
                    name: format!("node{}", node_id.0),
                    index: *node_id,
                    space: Space {
                        bytes: node_mem_size(*node_id) as usize,
                    },
                },
                group_id,
            );
            any_memory_attached = true;
            attach_cores_with_caches(&mut topo, group_id, node_cpus);
            covered.extend(node_cpus);
        }

        // Any cpu not covered by a NUMA group attaches directly to the socket.
        let uncovered: Vec<ProcId> = socket_cpus
            .iter()
            .copied()
            .filter(|c| !covered.contains(c))
            .collect();
        attach_cores_with_caches(&mut topo, socket_id, &uncovered);
    }

    // No NUMA info at all: attribute total system memory to the first socket
    // rather than leaving every socket without a Memory leaf.
    if !any_memory_attached {
        if let Some(socket_id) = first_socket {
            let total_kb = std::fs::read_to_string("/proc/meminfo")
                .ok()
                .and_then(|s| parse_mem_total_kb(&s))
                .unwrap_or(0);
            topo.add_memory(
                MemoryData {
                    name: "memory".to_string(),
                    index: MemId(0),
                    space: Space {
                        bytes: (total_kb * 1024) as usize,
                    },
                },
                socket_id,
            );
        }
    }

    topo
}

fn logical_cpus() -> Vec<ProcId> {
    let mut cpus: Vec<ProcId> = std::fs::read_dir(CPU_BASE)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            e.file_name()
                .to_str()?
                .strip_prefix("cpu")?
                .parse::<usize>()
                .ok()
        })
        .map(ProcId)
        .collect();
    cpus.sort_unstable();
    cpus
}

fn physical_package_id(cpu: ProcId) -> Option<usize> {
    let path = format!("{CPU_BASE}/cpu{}/topology/physical_package_id", cpu.0);
    parse_physical_package_id(&std::fs::read_to_string(path).ok()?)
}

fn max_freq_khz(cpu: ProcId) -> u64 {
    let path = format!("{CPU_BASE}/cpu{}/cpufreq/cpuinfo_max_freq", cpu.0);
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| parse_cpuinfo_max_freq(&s))
        .unwrap_or(0)
}

fn numa_nodes() -> Vec<(MemId, Vec<ProcId>)> {
    let mut ids: Vec<usize> = match std::fs::read_dir(NODE_BASE) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                e.file_name()
                    .to_str()?
                    .strip_prefix("node")?
                    .parse::<usize>()
                    .ok()
            })
            .collect(),
        Err(_) => return Vec::new(),
    };
    ids.sort_unstable();
    ids.into_iter()
        .filter_map(|id| {
            let list = std::fs::read_to_string(format!("{NODE_BASE}/node{id}/cpulist")).ok()?;
            Some((
                MemId(id),
                parse_cpulist(&list).into_iter().map(ProcId).collect(),
            ))
        })
        .collect()
}

fn node_mem_size(node: MemId) -> u64 {
    let path = format!("{NODE_BASE}/node{}/meminfo", node.0);
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| parse_mem_total_kb(&s))
        .map(|kb| kb * 1024)
        .unwrap_or(0)
}

/// The cpus this process is currently allowed to run on (`Cpus_allowed_list`
/// already reflects any cgroup cpuset restriction, no separate read needed).
/// `None` means the file couldn't be read/parsed — callers should treat that
/// as "unrestricted" rather than wrongly marking everything unavailable.
pub(super) fn allowed_cpus() -> Option<Vec<ProcId>> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let list = parse_cpulist(parse_status_field(&status, "Cpus_allowed_list:")?);
    Some(list.into_iter().map(ProcId).collect())
}

/// The NUMA nodes this process is currently allowed to allocate memory from
/// (`Mems_allowed_list`, the cpuset `mems` counterpart to `Cpus_allowed_list`).
pub(super) fn allowed_mems() -> Option<Vec<MemId>> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let list = parse_cpulist(parse_status_field(&status, "Mems_allowed_list:")?);
    Some(list.into_iter().map(MemId).collect())
}

/// Pins the calling thread to `cpu` via `sched_setaffinity`. `pid` `0` means
/// "the calling thread" (not the whole process).
pub(super) fn set_affinity_for_current(cpu: ProcId, _perf: usize) -> bool {
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        libc::CPU_SET(cpu.0, &mut set);
        libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set) == 0
    }
}

struct CacheLevel {
    level: u8,
    size: u64,
    /// Every cpu (within the region being built) that shares this exact
    /// cache instance.
    shared_cpus: Vec<ProcId>,
}

/// Reads `cpu`'s data/unified caches from `.../cache/indexN/*`, sorted from
/// the outermost level (highest number) to the innermost (L1). Instruction
/// caches are skipped: [`HardwareDataKind`]'s `L1`/etc. only hold one size,
/// so this reports the data/unified cache, matching the macOS backend.
fn cpu_caches(cpu: ProcId) -> Vec<CacheLevel> {
    let base = format!("{CPU_BASE}/cpu{}/cache", cpu.0);
    let Ok(entries) = std::fs::read_dir(&base) else {
        return Vec::new();
    };

    let mut levels: Vec<CacheLevel> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            e.file_name()
                .to_str()?
                .starts_with("index")
                .then(|| e.path())
        })
        .filter_map(|dir| {
            let ty = std::fs::read_to_string(dir.join("type")).ok()?;
            if !is_data_or_unified_cache_type(&ty) {
                return None;
            }
            let level = parse_cache_level(&std::fs::read_to_string(dir.join("level")).ok()?)?;
            if level == 0 || level > 4 {
                // Beyond `HardwareDataKind`'s L1..L4 range; not representable.
                return None;
            }
            let size = parse_cache_size(&std::fs::read_to_string(dir.join("size")).ok()?)?;
            let shared_cpus = std::fs::read_to_string(dir.join("shared_cpu_list"))
                .ok()
                .map(|s| parse_cpulist(&s).into_iter().map(ProcId).collect())
                .unwrap_or_else(|| vec![cpu]);
            Some(CacheLevel {
                level,
                size,
                shared_cpus,
            })
        })
        .collect();

    levels.sort_by(|a, b| b.level.cmp(&a.level));
    levels
}

fn cache_kind(level: u8, size: usize) -> HardwareDataKind {
    let space = Space { bytes: size };
    match level {
        4 => HardwareDataKind::L4 { space },
        3 => HardwareDataKind::L3 { space },
        2 => HardwareDataKind::L2 { space },
        _ => HardwareDataKind::L1 { space },
    }
}

/// Attaches `cpus` under `parent`, inserting whatever `L4`/`L3`/`L2`/`L1`
/// cache levels Linux reports in between. A cache instance shared by several
/// cpus becomes a single Hardware node with each of those cpus nested under
/// it, rather than one node per cpu.
fn attach_cores_with_caches(topo: &mut Topology, parent: TopoId, cpus: &[ProcId]) {
    let Some(&representative) = cpus.first() else {
        return;
    };
    attach_cache_level(topo, parent, cpus, &cpu_caches(representative));
}

fn attach_cache_level(topo: &mut Topology, parent: TopoId, cpus: &[ProcId], levels: &[CacheLevel]) {
    let Some((level, rest)) = levels.split_first() else {
        for &cpu in cpus {
            topo.add_processing(
                ProcessingData {
                    name: format!("cpu{}", cpu.0),
                    index: cpu,
                    perf: max_freq_khz(cpu) as usize,
                },
                parent,
            );
        }
        return;
    };

    let mut remaining: Vec<ProcId> = cpus.to_vec();
    let mut instance = 0;
    while let Some(&representative) = remaining.first() {
        // Re-derive the shared-cpu set for `representative` at this level:
        // different cpus in `remaining` may belong to different instances of
        // the same cache level (e.g. two separate L2 clusters).
        let shared: Vec<ProcId> = cpu_caches(representative)
            .into_iter()
            .find(|l| l.level == level.level)
            .map(|l| l.shared_cpus)
            .unwrap_or_else(|| vec![representative])
            .into_iter()
            .filter(|c| remaining.contains(c))
            .collect();

        let node_id = topo.add_hardware(
            HardwareData {
                name: format!("l{}-{instance}", level.level),
                kind: cache_kind(level.level, level.size as usize),
            },
            parent,
        );
        attach_cache_level(topo, node_id, &shared, rest);

        remaining.retain(|c| !shared.contains(c));
        instance += 1;
    }
}
