use std::hint::black_box;

use zhc::prelude::*;
use zhc_builder::{CiphertextSpec, div};

fn main() {
    black_box(div(CiphertextSpec::new(128, 2, 2)).compute_hpu_latency());
}
