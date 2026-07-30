use super::*;

#[cfg(any(target_os = "linux", test))]
mod parse;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
pub fn detect_topology() -> Topology {
    linux::detect()
}

#[cfg(target_os = "macos")]
pub fn detect_topology() -> Topology {
    macos::detect()
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn detect_topology() -> Topology {
    compile_error!("Unsupported target OS for topology detection")
}

#[cfg(target_os = "linux")]
pub fn allowed_cpus() -> Option<Vec<ProcId>> {
    linux::allowed_cpus()
}

#[cfg(target_os = "macos")]
pub fn allowed_cpus() -> Option<Vec<ProcId>> {
    macos::allowed_cpus()
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn allowed_cpus() -> Option<Vec<ProcId>> {
    compile_error!("Unsupported target OS for topology detection")
}

#[cfg(target_os = "linux")]
pub fn allowed_mems() -> Option<Vec<MemId>> {
    linux::allowed_mems()
}

#[cfg(target_os = "macos")]
pub fn allowed_mems() -> Option<Vec<MemId>> {
    macos::allowed_mems()
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn allowed_mems() -> Option<Vec<MemId>> {
    compile_error!("Unsupported target OS for topology detection")
}

#[cfg(target_os = "linux")]
pub fn set_affinity_for_current(cpu: ProcId, perf: usize) -> bool {
    linux::set_affinity_for_current(cpu, perf)
}

#[cfg(target_os = "macos")]
pub fn set_affinity_for_current(cpu: ProcId, perf: usize) -> bool {
    macos::set_affinity_for_current(cpu, perf)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn set_affinity_for_current(cpu: ProcId, perf: usize) -> bool {
    compile_error!("Unsupported target OS for topology detection")
}
