use serde::{Deserialize, Serialize};

use crate::units::Microseconds;

static S_IN_US: Microseconds = Microseconds(1_000_000.);

/// Represents a frequency in megahertz for simulation timing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Hash)]
pub struct MHz(pub usize);

impl MHz {
    const fn as_raw_hertz(&self) -> f64 {
        self.0 as f64 * 1_000_000.
    }

    /// Calculates the period duration in microseconds for this frequency.
    pub const fn period(&self) -> Microseconds {
        Microseconds((1. / self.as_raw_hertz()) * S_IN_US.0)
    }

    /// Returns the number of whole cycles spanning `duration` at this frequency.
    ///
    /// The result is truncated toward zero, so a `duration` shorter than one
    /// period yields zero.
    pub const fn n_cycles(&self, duration: Microseconds) -> usize {
        (duration.0 / self.period().0) as usize
    }
}

impl Default for MHz {
    fn default() -> Self {
        MHz(400)
    }
}
