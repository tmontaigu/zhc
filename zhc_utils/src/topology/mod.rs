//! Hardware topology discovery and CPU affinity management.
//!
//! This module models the hierarchical structure of a machine's processing and memory resources:
//! sockets, NUMA groups, cache levels, processing units, and memory nodes. The [`Topology`] type
//! owns the full tree; the domain types ([`HardwareDomain`], [`ProcessingDomain`],
//! [`MemoryDomain`]) provide borrowed views into specific nodes.
//!
//! A topology is either detected from the running system via [`Topology::detect_topology`] or
//! constructed manually with [`Topology::with_toplevel`] and the `add_*` methods. Once built,
//! callers traverse the tree through the domain views and can pin threads to specific processors
//! via [`ProcessingDomain::set_for_current`] or [`ProcessingDomain::run_on`].
//!
//! Availability tracking ([`Topology::detect_availability`]) marks which processors and memory
//! nodes the current process is permitted to use, reflecting cpuset or cgroup restrictions on
//! Linux. On platforms without such restrictions (macOS), all nodes remain available.

use std::fmt::{Debug, Display};

use zhc_utils_macro::existential_enum;

use crate::{Dumpable, Store, StoreIndex, small::SmallVec};

mod detect;

/// Index into the internal [`Topology`] stores.
///
/// Each node in the topology tree (hardware, processing, or memory) has a unique `TopoId`.
/// The root node always has index 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, StoreIndex)]
pub struct TopoId(pub u16);

/// OS-level memory node identifier.
///
/// On Linux this corresponds to a NUMA node id from `/sys/devices/system/node/nodeN`. On macOS,
/// which lacks NUMA, a single `MemId(0)` represents the entire system memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemId(pub usize);

/// OS-level logical processor identifier.
///
/// On Linux this corresponds to a cpu id from `/sys/devices/system/cpu/cpuN`. On macOS it is a
/// zero-based index into the logical cpu set reported by sysctl.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcId(pub usize);

impl Dumpable for TopoId {
    fn dump_to_string(&self) -> String {
        format!("#{}", self.0)
    }
}

/// A byte count with human-readable formatting.
///
/// The [`Display`] implementation renders the value in the largest binary unit (KiB, MiB, GiB,
/// TiB) for which the numeric part is at least 1, falling back to raw bytes otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Space {
    bytes: usize,
}

impl Display for Space {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const UNITS: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
        let mut value = self.bytes as f64;
        let mut unit = None;
        for name in UNITS {
            if value < 1024.0 {
                break;
            }
            value /= 1024.0;
            unit = Some(name);
        }
        match unit {
            Some(name) => write!(f, "{value:.2}{name}"),
            None => write!(f, "{}B", self.bytes),
        }
    }
}

/// Classification of a hardware node in the topology tree.
///
/// Variants are ordered from coarsest to finest granularity: `Machine` > `Socket` > `Group` >
/// `L4` > `L3` > `L2` > `L1`. A child node must have a strictly finer granularity than its
/// parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareDataKind {
    Machine,
    Socket,
    Group,
    L4 { space: Space },
    L3 { space: Space },
    L2 { space: Space },
    L1 { space: Space },
}

impl HardwareDataKind {
    fn get_rank(&self) -> u8 {
        match self {
            HardwareDataKind::Machine => 7,
            HardwareDataKind::Socket => 6,
            HardwareDataKind::Group => 5,
            HardwareDataKind::L4 { .. } => 4,
            HardwareDataKind::L3 { .. } => 3,
            HardwareDataKind::L2 { .. } => 2,
            HardwareDataKind::L1 { .. } => 1,
        }
    }
}

impl Dumpable for HardwareDataKind {
    fn dump_to_string(&self) -> String {
        match self {
            HardwareDataKind::Machine => "Machine".to_string(),
            HardwareDataKind::Socket => "Socket".to_string(),
            HardwareDataKind::Group => "Group".to_string(),
            HardwareDataKind::L4 { space } => format!("L4({})", space),
            HardwareDataKind::L3 { space } => format!("L3({})", space),
            HardwareDataKind::L2 { space } => format!("L2({})", space),
            HardwareDataKind::L1 { space } => format!("L1({})", space),
        }
    }
}

/// Metadata for a hardware node (socket, cache level, NUMA group, or machine root).
#[derive(Clone, PartialEq, Eq)]
pub struct HardwareData {
    pub name: String,
    pub kind: HardwareDataKind,
}

impl Dumpable for HardwareData {
    fn dump_to_string(&self) -> String {
        format!("{} [{}]", self.name, self.kind.dump_to_string())
    }
}

/// Metadata for a memory node.
#[derive(Clone, PartialEq, Eq)]
pub struct MemoryData {
    pub name: String,
    pub index: MemId,
    pub space: Space,
}

impl Debug for MemoryData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (#{}, {})", self.name, self.index.0, self.space)
    }
}

impl Dumpable for MemoryData {
    fn dump_to_string(&self) -> String {
        format!("{:?}", self)
    }
}

/// Metadata for a processing unit (logical CPU).
///
/// The `perf` field is a platform-dependent performance indicator: higher values indicate faster
/// cores. On Linux it is the maximum frequency in kHz; on macOS it is a tier rank derived from
/// the performance-level index. The value is only meaningful for relative ordering within a
/// single topology instance.
#[derive(Clone, PartialEq, Eq)]
pub struct ProcessingData {
    pub name: String,
    pub index: ProcId,
    pub perf: usize,
}

impl ProcessingData {
    /// Attempts to pin the calling thread to this processor.
    ///
    /// Returns true if the affinity was set successfully. On Linux this uses
    /// `sched_setaffinity`; on macOS the behavior is architecture-dependent (Intel uses
    /// `THREAD_AFFINITY_POLICY` as a hint, Apple Silicon uses QoS class steering).
    pub fn set_for_current(&self) -> bool {
        detect::set_affinity_for_current(self.index, self.perf)
    }
}

impl Dumpable for ProcessingData {
    fn dump_to_string(&self) -> String {
        format!("{} (#{}, perf={})", self.name, self.index.0, self.perf)
    }
}

/// Internal node payload, discriminating hardware, memory, and processing nodes.
#[existential_enum]
#[derive(Clone, PartialEq, Eq)]
pub enum Data {
    Hardware(HardwareData),
    Memory(MemoryData),
    Processing(ProcessingData),
}

impl Dumpable for Data {
    fn dump_to_string(&self) -> String {
        match self {
            Data::Hardware(d) => d.dump_to_string(),
            Data::Memory(d) => d.dump_to_string(),
            Data::Processing(d) => d.dump_to_string(),
        }
    }
}

/// A tree of hardware, memory, and processing nodes describing machine topology.
///
/// The root is always a [`HardwareDataKind::Machine`] node. Hardware nodes form the internal
/// tree structure; processing and memory nodes are leaves attached to hardware parents.
#[derive(Clone, PartialEq, Eq)]
pub struct Topology {
    data: Store<TopoId, Data>,
    parent: Store<TopoId, TopoId>,
    children: Store<TopoId, SmallVec<TopoId>>,
    available: Store<TopoId, bool>,
}

impl Topology {
    /// Detects the hardware topology of the current machine.
    ///
    /// Queries platform-specific sources (`/sys` and `/proc` on Linux, sysctl on macOS) to build
    /// a tree of sockets, NUMA groups, cache levels, processors, and memory nodes. Availability
    /// is initialized by calling [`detect_availability`](Self::detect_availability).
    pub fn detect_topology() -> Self {
        let mut oup = detect::detect_topology();
        oup.detect_availability();
        oup
    }

    /// Updates availability flags based on the current process's cpuset restrictions.
    ///
    /// On Linux, processors and memory nodes not in the process's `Cpus_allowed_list` or
    /// `Mems_allowed_list` (from `/proc/self/status`) are marked unavailable. On macOS, where
    /// no such restriction mechanism exists, all nodes remain available.
    pub fn detect_availability(&mut self) {
        let allowed_cpus = detect::allowed_cpus();
        let allowed_mems = detect::allowed_mems();

        for (id, data) in self.data.enumerate_iter() {
            let available = match data {
                Data::Processing(p) => allowed_cpus
                    .as_ref()
                    .map_or(true, |cpus| cpus.contains(&p.index)),
                Data::Memory(m) => allowed_mems
                    .as_ref()
                    .map_or(true, |mems| mems.contains(&m.index)),
                Data::Hardware(_) => true,
            };
            self.available[id] = available;
        }
    }

    /// Returns true if all processors and memory nodes in the topology are available.
    pub fn is_completely_available(&self) -> bool {
        self.toplevel().is_completely_available()
    }

    /// Creates a new topology with the given root node.
    ///
    /// The root's [`TopoId`] is always 0. Use [`add_hardware`](Self::add_hardware),
    /// [`add_memory`](Self::add_memory), and [`add_processing`](Self::add_processing) to attach
    /// children.
    ///
    /// # Panics
    ///
    /// Panics if `data.kind` is not [`HardwareDataKind::Machine`].
    pub fn with_toplevel(data: HardwareData) -> Self {
        assert!(
            matches!(data.kind, HardwareDataKind::Machine),
            "the top-level domain must be a `Machine`",
        );
        let mut top = Topology {
            data: Store::empty(),
            parent: Store::empty(),
            children: Store::empty(),
            available: Store::empty(),
        };
        let root = top.data.push(Data::Hardware(data));
        top.parent.push(root);
        top.children.push(SmallVec::new());
        top.available.push(true);
        top
    }

    fn push_under(&mut self, data: Data, parent: TopoId) -> TopoId {
        match (&self.data[parent], &data) {
            (Data::Hardware(pdata), Data::Hardware(cdata)) => {
                assert!(pdata.kind.get_rank() > cdata.kind.get_rank())
            }
            (Data::Hardware(_), Data::Processing(_) | Data::Memory(_)) => {}
            (_, _) => panic!(),
        }
        let id = self.data.push(data);
        self.parent.push(parent);
        self.children.push(SmallVec::new());
        self.children[parent].push(id);
        self.available.push(true);
        id
    }

    /// Attaches a hardware node under the given parent and returns its [`TopoId`].
    ///
    /// # Panics
    ///
    /// Panics if `parent` is not a hardware node, or if `data.kind` does not have a strictly
    /// finer granularity than the parent's kind.
    pub fn add_hardware(&mut self, data: HardwareData, parent: TopoId) -> TopoId {
        self.push_under(Data::Hardware(data), parent)
    }

    /// Attaches a memory node under the given parent and returns its [`TopoId`].
    ///
    /// # Panics
    ///
    /// Panics if `parent` is not a hardware node.
    pub fn add_memory(&mut self, data: MemoryData, parent: TopoId) -> TopoId {
        self.push_under(Data::Memory(data), parent)
    }

    /// Attaches a processing node under the given parent and returns its [`TopoId`].
    ///
    /// # Panics
    ///
    /// Panics if `parent` is not a hardware node.
    pub fn add_processing(&mut self, data: ProcessingData, parent: TopoId) -> TopoId {
        self.push_under(Data::Processing(data), parent)
    }

    /// Creates a minimal virtual topology with a single core and a single memory node.
    ///
    /// Useful for testing or as a fallback when real detection is unavailable.
    pub fn single_core() -> Self {
        let mut topo = Self::with_toplevel(HardwareData {
            name: "VirtualMachine".to_string(),
            kind: HardwareDataKind::Machine,
        });
        let root = topo.toplevel().id;
        topo.add_memory(
            MemoryData {
                name: "VirtualMemory".to_string(),
                index: MemId(0),
                space: Space { bytes: 0 },
            },
            root,
        );
        topo.add_processing(
            ProcessingData {
                name: "VirtualCore0".to_string(),
                index: ProcId(0),
                perf: 0,
            },
            root,
        );
        topo
    }

    /// Returns a view of the root hardware node.
    pub fn toplevel(&self) -> HardwareDomain<'_> {
        HardwareDomain {
            top: self,
            id: TopoId(0),
        }
    }

    /// Iterates over all memory nodes in the topology.
    pub fn iter_all_memories(&self) -> impl DoubleEndedIterator<Item = MemoryDomain<'_>> + use<'_> {
        self.data
            .enumerate_iter()
            .filter(|(_, data)| data.is_memory())
            .map(|(id, _)| MemoryDomain { top: self, id })
    }

    /// Iterates over all processing nodes in the topology.
    pub fn iter_all_processors(
        &self,
    ) -> impl DoubleEndedIterator<Item = ProcessingDomain<'_>> + use<'_> {
        self.data
            .enumerate_iter()
            .filter(|(_, data)| data.is_processing())
            .map(|(id, _)| ProcessingDomain { top: self, id })
    }

    /// Returns the total number of processing nodes.
    pub fn n_processors(&self) -> usize {
        self.iter_all_processors().count()
    }

    /// Returns the total number of memory nodes.
    pub fn n_memories(&self) -> usize {
        self.iter_all_memories().count()
    }
}

impl Debug for Topology {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{:?}", self.toplevel().dump_to_string())
    }
}

impl Dumpable for Topology {
    fn dump_to_string(&self) -> String {
        self.toplevel().dump_to_string()
    }
}

fn indent(s: &str) -> String {
    s.lines()
        .map(|l| format!("  {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A borrowed view of a hardware node within a [`Topology`].
///
/// Provides access to the node's metadata and traversal methods for its children (hardware,
/// memory, and processing nodes).
pub struct HardwareDomain<'a> {
    top: &'a Topology,
    id: TopoId,
}

impl<'a> HardwareDomain<'a> {
    /// Returns the metadata for this hardware node.
    pub fn get_data(&self) -> &'a HardwareData {
        self.top.data[self.id].unwrap_hardware_ref()
    }

    /// Returns the parent hardware node, or `None` if this is the root.
    pub fn get_parent(&self) -> Option<HardwareDomain<'a>> {
        if self.is_top_level() {
            None
        } else {
            Some(HardwareDomain {
                top: self.top,
                id: self.top.parent[self.id],
            })
        }
    }

    /// Returns true if this is the root node of the topology.
    pub fn is_top_level(&self) -> bool {
        self.top.parent[self.id].0 == self.id.0
    }

    /// Returns true if this node and all its descendants are available.
    pub fn is_completely_available(&self) -> bool {
        self.top.available[self.id]
            && self.iter_memories().all(|m| m.is_completely_available())
            && self.iter_processors().all(|p| p.is_completely_available())
            && self.iter_hardwares().all(|h| h.is_completely_available())
    }

    /// Iterates over the memory nodes that are direct children of this hardware node.
    pub fn iter_memories(&self) -> impl DoubleEndedIterator<Item = MemoryDomain<'a>> + use<'a> {
        let top = self.top;
        top.children[self.id]
            .iter()
            .copied()
            .filter(|&id| top.data[id].is_memory())
            .map(|id| MemoryDomain { top, id })
    }

    /// Iterates over the processing nodes that are direct children of this hardware node.
    pub fn iter_processors(
        &self,
    ) -> impl DoubleEndedIterator<Item = ProcessingDomain<'a>> + use<'a> {
        let top = self.top;
        top.children[self.id]
            .iter()
            .copied()
            .filter(|&id| top.data[id].is_processing())
            .map(|id| ProcessingDomain { top, id })
    }

    /// Iterates over all processing nodes in the subtree rooted at this hardware node.
    ///
    /// The iteration order is unspecified.
    pub fn iter_all_processors(
        &self,
    ) -> impl DoubleEndedIterator<Item = ProcessingDomain<'a>> + use<'a> {
        let top = self.top;
        let mut procs = SmallVec::new();
        let mut pending = SmallVec::new();
        pending.push(self.id);
        while let Some(id) = pending.pop() {
            let children = &top.children[id];
            procs.extend(
                children
                    .iter()
                    .copied()
                    .filter(|&c| top.data[c].is_processing()),
            );
            pending.extend(
                children
                    .iter()
                    .rev()
                    .copied()
                    .filter(|&c| top.data[c].is_hardware()),
            );
        }
        procs
            .into_iter()
            .map(move |id| ProcessingDomain { top, id })
    }

    /// Iterates over the hardware nodes that are direct children of this hardware node.
    pub fn iter_hardwares(&self) -> impl DoubleEndedIterator<Item = HardwareDomain<'a>> + use<'a> {
        let top = self.top;
        top.children[self.id]
            .iter()
            .copied()
            .filter(|&id| top.data[id].is_hardware())
            .map(|id| HardwareDomain { top, id })
    }
}

impl Dumpable for HardwareDomain<'_> {
    fn dump_to_string(&self) -> String {
        let mut lines = vec![self.get_data().dump_to_string()];
        for mem in self.iter_memories() {
            lines.push(indent(&mem.dump_to_string()));
        }
        for proc in self.iter_processors() {
            lines.push(indent(&proc.dump_to_string()));
        }
        for hw in self.iter_hardwares() {
            lines.push(indent(&hw.dump_to_string()));
        }
        lines.join("\n")
    }
}

/// A borrowed view of a processing node within a [`Topology`].
///
/// Provides access to the processor's metadata, availability, parent hardware node, and
/// affinity-setting methods.
pub struct ProcessingDomain<'a> {
    top: &'a Topology,
    id: TopoId,
}

impl<'a> ProcessingDomain<'a> {
    /// Returns the metadata for this processing node.
    pub fn get_data(&self) -> &'a ProcessingData {
        self.top.data[self.id].unwrap_processing_ref()
    }

    /// Returns true if this processor is available to the current process.
    pub fn is_available(&self) -> bool {
        self.top.available[self.id]
    }

    /// Returns true if this processor is available.
    ///
    /// Equivalent to [`is_available`](Self::is_available) for processing nodes, which have no
    /// children.
    pub fn is_completely_available(&self) -> bool {
        self.is_available()
    }

    /// Returns the parent hardware node.
    pub fn get_hardware(&self) -> HardwareDomain<'a> {
        HardwareDomain {
            top: self.top,
            id: self.top.parent[self.id],
        }
    }

    /// Returns the nearest memory node by ascending through ancestor hardware nodes.
    ///
    /// Walks up the tree from this processor's parent until a hardware node with at least one
    /// memory child is found, then returns the first such memory node.
    ///
    /// # Panics
    ///
    /// Panics if no memory node exists in any ancestor. A well-formed topology (produced by
    /// [`Topology::detect_topology`] or [`Topology::single_core`]) always contains at least one
    /// memory node.
    pub fn get_closest_memory(&self) -> MemoryDomain<'a> {
        let mut hardware = self.get_hardware();
        loop {
            if let Some(memory) = hardware.iter_memories().next() {
                return memory;
            }
            match hardware.get_parent() {
                Some(parent) => hardware = parent,
                None => panic!(
                    "no memory domain found above {}",
                    self.get_data().dump_to_string(),
                ),
            }
        }
    }

    /// Attempts to pin the calling thread to this processor.
    ///
    /// Returns true if the affinity was set successfully. Delegates to
    /// [`ProcessingData::set_for_current`].
    ///
    /// # Panics
    ///
    /// Panics if this processor is not available (see [`is_available`](Self::is_available)).
    pub fn set_for_current(&self) -> bool {
        assert!(
            self.is_available(),
            "cannot pin to {}: domain unavailable",
            self.get_data().dump_to_string(),
        );
        self.get_data().set_for_current()
    }

    /// Spawns a scoped thread pinned to this processor, runs the closure, and returns its result.
    ///
    /// The closure executes on a new thread whose affinity is set to this processor before
    /// invoking `f`. The call blocks until the thread completes.
    ///
    /// # Panics
    ///
    /// Panics if this processor is not available, or if the spawned thread panics.
    pub fn run_on<T: Send>(&self, f: impl FnOnce() -> T + Send) -> T {
        std::thread::scope(|s| {
            s.spawn(|| {
                self.set_for_current();
                f()
            })
            .join()
            .unwrap()
        })
    }
}

impl Dumpable for ProcessingDomain<'_> {
    fn dump_to_string(&self) -> String {
        self.get_data().dump_to_string()
    }
}

/// A borrowed view of a memory node within a [`Topology`].
///
/// Provides access to the node's metadata, availability, parent hardware node, and the set of
/// processors whose closest memory is this node.
pub struct MemoryDomain<'a> {
    top: &'a Topology,
    id: TopoId,
}

impl<'a> MemoryDomain<'a> {
    /// Returns the metadata for this memory node.
    pub fn get_data(&self) -> &'a MemoryData {
        self.top.data[self.id].unwrap_memory_ref()
    }

    /// Returns true if this memory node is available to the current process.
    pub fn is_available(&self) -> bool {
        self.top.available[self.id]
    }

    /// Returns true if this memory node is available.
    ///
    /// Equivalent to [`is_available`](Self::is_available) for memory nodes, which have no
    /// children.
    pub fn is_completely_available(&self) -> bool {
        self.is_available()
    }

    /// Returns the parent hardware node.
    pub fn get_hardware(&self) -> HardwareDomain<'a> {
        HardwareDomain {
            top: self.top,
            id: self.top.parent[self.id],
        }
    }

    /// Iterates over all processors in the subtree rooted at this memory node's parent hardware.
    ///
    /// These are the processors for which this memory node is the "closest" memory in the
    /// topology hierarchy.
    pub fn iter_associated_processors(
        &self,
    ) -> impl DoubleEndedIterator<Item = ProcessingDomain<'a>> + use<'a> {
        self.get_hardware().iter_all_processors()
    }
}

impl Dumpable for MemoryDomain<'_> {
    fn dump_to_string(&self) -> String {
        self.get_data().dump_to_string()
    }
}
