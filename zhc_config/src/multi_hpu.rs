use serde::Serialize;

use crate::hpu::HpuConfig;

/// Configuration for a multi-HPU system: a per-board config replicated across a
/// fixed number of boards.
#[derive(Clone, Debug, Serialize, PartialEq, Eq, Hash)]
pub struct MultiHpuConfig {
    /// Configuration shared by every board in the system.
    pub hpu_config: HpuConfig,
    /// Number of HPU boards in the system.
    pub n_hpus: u8,
}

impl Default for MultiHpuConfig {
    fn default() -> Self {
        MultiHpuConfig {
            hpu_config: HpuConfig::default(),
            n_hpus: 4,
        }
    }
}
