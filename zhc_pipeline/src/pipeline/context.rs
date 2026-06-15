use zhc_builder::Builder;
use zhc_config::{hpu::HpuConfig, multi_hpu::MultiHpuConfig};

#[derive(Debug)]
pub struct PipelineContext {
    pub builder: Option<Builder>,
    pub hpu_config: Option<HpuConfig>,
    pub multi_hpu_config: Option<MultiHpuConfig>,
    pub legacy_hpu_scheduler: bool,
    pub hpu_trace_events: bool,
}

impl PipelineContext {
    pub fn new() -> Self {
        PipelineContext {
            builder: None,
            hpu_config: None,
            multi_hpu_config: None,
            legacy_hpu_scheduler: false,
            hpu_trace_events: false,
        }
    }
}
