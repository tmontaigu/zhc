use std::fmt::Display;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Microseconds(pub f64);

impl Display for Microseconds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.2} µs", self.0)
    }
}

impl Eq for Microseconds {}
