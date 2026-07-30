use std::ffi::CString;

use crate::topology::{
    HardwareData, HardwareDataKind, MemId, MemoryData, ProcId, ProcessingData, Space, TopoId,
    Topology,
};

pub(super) fn detect() -> Topology {
    let hostname = sysctl_string("kern.hostname").unwrap_or_else(|| "machine".to_string());

    let mut topo = Topology::with_toplevel(HardwareData {
        name: hostname,
        kind: HardwareDataKind::Machine,
    });
    // `with_toplevel` always creates the root at index 0.
    let root = TopoId(0);

    let packages = sysctl_u32("hw.packages").unwrap_or(1).max(1);
    let mem_size = sysctl_u64("hw.memsize").unwrap_or(0);
    let nperflevels = sysctl_u32("hw.nperflevels").unwrap_or(1);

    for pkg in 0..packages {
        let socket_id = topo.add_hardware(
            HardwareData {
                name: format!("socket{pkg}"),
                kind: HardwareDataKind::Socket,
            },
            root,
        );

        // `hw.memsize` describes the whole machine (macOS never reports more
        // than one package on real hardware); attribute it to the first
        // socket rather than guessing at a split across packages.
        if pkg == 0 {
            topo.add_memory(
                MemoryData {
                    name: "memory".to_string(),
                    index: MemId(0),
                    space: Space {
                        bytes: mem_size as usize,
                    },
                },
                socket_id,
            );
        } else {
            // No per-package core/perf-level breakdown is exposed on macOS.
            continue;
        }

        if nperflevels > 1 {
            for level in 0..nperflevels {
                add_perflevel(&mut topo, socket_id, level, nperflevels);
            }
        } else {
            add_flat_cores(&mut topo, socket_id);
        }
    }

    topo
}

/// Builds one Apple `hw.perflevelN` cluster (e.g. "Performance"/"Efficiency"
/// on hybrid Apple Silicon) as `Group > L2 > L1 > Processing`, splitting into
/// as many `L2` instances as `cpusperl2` indicates are actually shared.
fn add_perflevel(topo: &mut Topology, socket_id: TopoId, level: u32, nperflevels: u32) {
    let logical = sysctl_u32(&format!("hw.perflevel{level}.logicalcpu")).unwrap_or(0);
    let name = sysctl_string(&format!("hw.perflevel{level}.name"))
        .unwrap_or_else(|| format!("perflevel{level}"));
    let group_id = topo.add_hardware(
        HardwareData {
            name: name.clone(),
            kind: HardwareDataKind::Group,
        },
        socket_id,
    );
    let perf = (nperflevels - level) as usize;

    // `HardwareDataKind::L1` only holds a single size, so this reports the
    // data cache (the more relevant one for compute workloads) and drops the
    // separate instruction-cache size Apple also exposes.
    let l1d = sysctl_u32(&format!("hw.perflevel{level}.l1dcachesize")).unwrap_or(0) as usize;
    let l2 = sysctl_u32(&format!("hw.perflevel{level}.l2cachesize")).unwrap_or(0) as usize;
    let cpus_per_l2 = sysctl_u32(&format!("hw.perflevel{level}.cpusperl2"))
        .filter(|&n| n > 0)
        .unwrap_or(logical.max(1));

    let mut remaining = logical;
    let mut l2_index = 0;
    while remaining > 0 {
        let cpus_here = remaining.min(cpus_per_l2);
        let l2_id = topo.add_hardware(
            HardwareData {
                name: format!("{name}-l2-{l2_index}"),
                kind: HardwareDataKind::L2 {
                    space: Space { bytes: l2 },
                },
            },
            group_id,
        );
        let start = logical - remaining;
        for offset in 0..cpus_here {
            let cpu = start + offset;
            let l1_id = topo.add_hardware(
                HardwareData {
                    name: format!("{name}{cpu}-l1"),
                    kind: HardwareDataKind::L1 {
                        space: Space { bytes: l1d },
                    },
                },
                l2_id,
            );
            topo.add_processing(
                ProcessingData {
                    name: format!("{name}{cpu}"),
                    index: ProcId(cpu as usize),
                    perf,
                },
                l1_id,
            );
        }
        remaining -= cpus_here;
        l2_index += 1;
    }
}

/// macOS has no equivalent of Linux's cpuset/`sched_getaffinity` — no syscall
/// lets an unprivileged process query or be restricted to a cpu subset, so
/// there is nothing to report here. Always `None` (unrestricted).
pub(super) fn allowed_cpus() -> Option<Vec<ProcId>> {
    None
}

/// See [`allowed_cpus`]: no restriction mechanism exists on macOS.
pub(super) fn allowed_mems() -> Option<Vec<MemId>> {
    None
}

/// Sets a `THREAD_AFFINITY_POLICY` tag on the calling thread. Unlike Linux's
/// `sched_setaffinity`, this is only a hint: XNU uses matching tags to
/// *prefer* co-scheduling threads on the same cache cluster, but gives no
/// guarantee the thread actually runs on `cpu`, and there is no unprivileged
/// API that does. Kept for behavioral parity with `core_affinity`'s own
/// (equally weak) macOS backend.
///
/// Intel Macs only: on Apple Silicon this policy is a no-op (XNU returns
/// `KERN_NOT_SUPPORTED` for it unconditionally, verified empirically), so
/// that target uses QoS instead — see the other `set_affinity_for_current`
/// below, `cfg`-gated on `target_arch`.
#[cfg(not(target_arch = "aarch64"))]
#[allow(deprecated)] // `mach_thread_self`/`mach_task_self`: libc suggests the `mach2` crate,
// not worth a new dependency for two calls.
pub(super) fn set_affinity_for_current(cpu: ProcId, _perf: usize) -> bool {
    // Not exposed by `libc` (unlike the rest of this function's mach calls).
    unsafe extern "C" {
        fn mach_port_deallocate(
            task: libc::mach_port_t,
            name: libc::mach_port_t,
        ) -> libc::kern_return_t;
    }

    unsafe {
        let policy = libc::thread_affinity_policy_data_t {
            affinity_tag: cpu.0 as libc::integer_t,
        };
        let count = (std::mem::size_of::<libc::thread_affinity_policy_data_t>()
            / std::mem::size_of::<libc::integer_t>())
            as libc::mach_msg_type_number_t;
        let thread = libc::mach_thread_self();
        let result = libc::thread_policy_set(
            thread,
            libc::THREAD_AFFINITY_POLICY as u32,
            &policy as *const libc::thread_affinity_policy_data_t as libc::thread_policy_t,
            count,
        );
        mach_port_deallocate(libc::mach_task_self(), thread);
        result == libc::KERN_SUCCESS
    }
}

/// Apple Silicon: there is no per-core (or even per-cluster) binding API at
/// all, so this biases the calling thread toward the Performance or
/// Efficiency cluster via its QoS class instead — the mechanism XNU's
/// scheduler actually uses for P/E placement decisions on this hardware.
/// This is coarser than the `ProcessingDomain` it's given: it can only steer
/// toward *a cluster*, never toward this specific cpu within it.
///
/// `perf` (`nperflevels - level`, from `add_perflevel`) is read as a tier
/// rank: today's chips only ship two tiers (Performance/Efficiency), so
/// `perf >= 2` means Performance and `perf == 1` means Efficiency; `perf ==
/// 0` (unranked, i.e. this isn't actually a hybrid-topology core) has
/// nothing to bias toward. This heuristic would need revisiting if a 3-tier
/// chip ever ships.
#[cfg(target_arch = "aarch64")]
pub(super) fn set_affinity_for_current(_cpu: ProcId, perf: usize) -> bool {
    unsafe extern "C" {
        fn pthread_set_qos_class_self_np(
            qos_class: libc::c_uint,
            relative_priority: libc::c_int,
        ) -> libc::c_int;
    }

    // qos_class_t values from <pthread/qos.h>; not exposed by `libc`.
    const QOS_CLASS_USER_INITIATED: libc::c_uint = 0x19;
    const QOS_CLASS_BACKGROUND: libc::c_uint = 0x09;

    let qos = match perf {
        0 => return false,
        1 => QOS_CLASS_BACKGROUND,
        _ => QOS_CLASS_USER_INITIATED,
    };
    unsafe { pthread_set_qos_class_self_np(qos, 0) == 0 }
}

/// Builds a flat `L3? > L2 > L1 > Processing` hierarchy for machines without
/// per-perf-level reporting (Intel Macs): the cache sizes are machine-wide,
/// so every core shares the same single `L3`/`L2` instance.
fn add_flat_cores(topo: &mut Topology, socket_id: TopoId) {
    let logical = sysctl_u32("hw.logicalcpu")
        .map(|n| n as usize)
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
        });
    let l1d = sysctl_u32("hw.l1dcachesize").unwrap_or(0) as usize;
    let l2 = sysctl_u32("hw.l2cachesize").unwrap_or(0) as usize;
    let l3 = sysctl_u32("hw.l3cachesize").unwrap_or(0) as usize;

    let mut parent = socket_id;
    if l3 > 0 {
        parent = topo.add_hardware(
            HardwareData {
                name: "l3".to_string(),
                kind: HardwareDataKind::L3 {
                    space: Space { bytes: l3 },
                },
            },
            parent,
        );
    }
    let l2_id = topo.add_hardware(
        HardwareData {
            name: "l2".to_string(),
            kind: HardwareDataKind::L2 {
                space: Space { bytes: l2 },
            },
        },
        parent,
    );
    for i in 0..logical {
        let l1_id = topo.add_hardware(
            HardwareData {
                name: format!("cpu{i}-l1"),
                kind: HardwareDataKind::L1 {
                    space: Space { bytes: l1d },
                },
            },
            l2_id,
        );
        topo.add_processing(
            ProcessingData {
                name: format!("cpu{i}"),
                index: ProcId(i),
                perf: 0,
            },
            l1_id,
        );
    }
}

fn sysctl_u32(name: &str) -> Option<u32> {
    let cname = CString::new(name).ok()?;
    let mut value: u32 = 0;
    let mut len = std::mem::size_of::<u32>();
    let ret = unsafe {
        libc::sysctlbyname(
            cname.as_ptr(),
            &mut value as *mut u32 as *mut libc::c_void,
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    (ret == 0).then_some(value)
}

fn sysctl_u64(name: &str) -> Option<u64> {
    let cname = CString::new(name).ok()?;
    let mut value: u64 = 0;
    let mut len = std::mem::size_of::<u64>();
    let ret = unsafe {
        libc::sysctlbyname(
            cname.as_ptr(),
            &mut value as *mut u64 as *mut libc::c_void,
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    (ret == 0).then_some(value)
}

fn sysctl_string(name: &str) -> Option<String> {
    let cname = CString::new(name).ok()?;
    // First call with a null buffer to discover the required length.
    let mut len: usize = 0;
    let ret = unsafe {
        libc::sysctlbyname(
            cname.as_ptr(),
            std::ptr::null_mut(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if ret != 0 || len == 0 {
        return None;
    }
    let mut buf = vec![0u8; len];
    let ret = unsafe {
        libc::sysctlbyname(
            cname.as_ptr(),
            buf.as_mut_ptr() as *mut libc::c_void,
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if ret != 0 {
        return None;
    }
    buf.truncate(len.saturating_sub(1)); // drop the trailing NUL
    String::from_utf8(buf).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_reports_every_logical_cpu_and_nonzero_memory() {
        let topo = detect();
        let root = topo.toplevel();
        assert!(matches!(root.get_data().kind, HardwareDataKind::Machine));

        let total_processing = topo.iter_all_processors().count();
        let total_memory: usize = topo
            .iter_all_memories()
            .map(|m| m.get_data().space.bytes)
            .sum();

        let expected = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        assert_eq!(total_processing, expected);
        assert!(total_memory > 0);
    }

    #[test]
    fn detect_availability_leaves_everything_available_on_macos() {
        // Everything defaults to available before `detect_availability` is
        // ever called; on macOS, with no restriction mechanism to report,
        // it should stay that way afterward too.
        let mut topo = detect();
        assert!(topo.is_completely_available());
        topo.detect_availability();
        assert!(topo.is_completely_available());
    }

    #[test]
    fn sysctl_u64_reads_memsize() {
        assert!(sysctl_u64("hw.memsize").unwrap_or(0) > 0);
    }

    #[test]
    fn sysctl_string_reads_hostname() {
        assert!(sysctl_string("kern.hostname").is_some_and(|s| !s.is_empty()));
    }

    #[test]
    fn sysctl_missing_key_returns_none() {
        assert_eq!(sysctl_u32("hw.does_not_exist"), None);
        assert_eq!(sysctl_string("hw.does_not_exist"), None);
    }
}
