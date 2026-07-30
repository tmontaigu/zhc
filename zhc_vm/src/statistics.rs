use zhc_utils::Dumpable;

/// Execution performance counters collected across all VM worker threads.
///
/// Returned by [`Vm::get_statistics`](crate::Vm::get_statistics), this struct captures
/// cumulative timing information since the last
/// [`reset_statistics`](crate::Vm::reset_statistics) call (or since VM creation). All
/// time values are in nanoseconds.
///
/// The three time components — execution, spin-wait, and parked — partition the total
/// available worker time (`wall_nanos * n_workers`). High utilization means workers
/// spent most of their time doing useful FHE operations; high spin fraction indicates
/// contention on data dependencies; high parked fraction suggests the workload does
/// not have enough parallelism to keep all workers busy.
#[derive(Debug, Clone, Copy)]
pub struct Statistics {
    /// Cumulative nanoseconds workers spent executing FHE operations (PBS, KS, arithmetic).
    pub exec_nanos: u64,
    /// Cumulative nanoseconds workers spent spin-waiting on unresolved data dependencies.
    pub spin_nanos: u64,
    /// Cumulative wall-clock nanoseconds across all [`Vm::execute`](crate::Vm::execute)
    /// calls.
    pub wall_nanos: u64,
    /// The number of worker threads in the VM.
    pub n_workers: usize,
}

impl Statistics {
    /// Returns the total available worker-nanoseconds (`wall_nanos * n_workers`).
    ///
    /// This is the theoretical maximum execution time if every worker were busy for the
    /// entire wall-clock duration. It serves as the denominator for utilization metrics.
    pub fn available_nanos(&self) -> u64 {
        self.wall_nanos * self.n_workers as u64
    }

    /// Returns the fraction of available time spent executing FHE operations.
    ///
    /// A value of 1.0 means all workers were busy the entire time; 0.0 means no useful
    /// work was performed. In practice, values above 0.8 indicate good parallel efficiency.
    pub fn utilization(&self) -> f64 {
        self.exec_nanos as f64 / self.available_nanos() as f64
    }

    /// Returns the fraction of available time spent spin-waiting on dependencies.
    ///
    /// A high spin fraction suggests that the execution plan has long dependency chains
    /// that serialize workers. Restructuring the plan to expose more parallelism can
    /// reduce this.
    pub fn spin_fraction(&self) -> f64 {
        self.spin_nanos as f64 / self.available_nanos() as f64
    }

    /// Returns the fraction of available time where workers were neither executing nor
    /// spinning.
    ///
    /// Parked time typically represents workers that had no instructions assigned to them
    /// at all — the workload simply did not have enough independent operations to fill
    /// every core.
    pub fn parked_fraction(&self) -> f64 {
        1.0 - self.utilization() - self.spin_fraction()
    }
}

impl Dumpable for Statistics {
    fn dump_to_string(&self) -> String {
        let ms = |nanos: u64| nanos as f64 / 1e6;
        format!(
            "╔══════════════════════════════════════════════════════════════════════════════
║ VM Statistics
║──────────────────────────────────────────────────────────────────────────────
║   Threads   : {}
║   Wall      : {:.3} ms
║   Available : {:.3} ms  (wall × threads)
║──────────────────────────────────────────────────────────────────────────────
║   Exec      : {:>10.3} ms   ({:>5.1}%)
║   Spin      : {:>10.3} ms   ({:>5.1}%)
║   Parked    : {:>10.3} ms   ({:>5.1}%)
╚══════════════════════════════════════════════════════════════════════════════",
            self.n_workers,
            ms(self.wall_nanos),
            ms(self.available_nanos()),
            ms(self.exec_nanos),
            100.0 * self.utilization(),
            ms(self.spin_nanos),
            100.0 * self.spin_fraction(),
            ms(self.available_nanos()) - ms(self.exec_nanos) - ms(self.spin_nanos),
            100.0 * self.parked_fraction(),
        )
    }
}
