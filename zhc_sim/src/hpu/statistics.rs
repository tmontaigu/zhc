use super::*;
use zhc_utils::units::Cycle;

/// Tracks completed operations and manages resource cleanup after execution.
#[derive(Debug, Default, Serialize)]
pub struct Statistics {
    #[serde(skip)]
    pub dops: Vec<DOp>,
    pub timeouts: u16,
    /// Total cycles the PBS PE spent processing batches.
    pub pbs_busy: Cycle,
    /// Number of PBS batches launched.
    pub pbs_batches: usize,
    /// Total number of PBSes processed across all batches.
    pub pbs_slots_filled: usize,
    #[serde(skip)]
    pbs_launched_at: Option<Cycle>,
}

impl Simulatable for Statistics {
    type Event = Events;

    fn handle(
        &mut self,
        _dispatcher: &mut impl Dispatch<Event = Self::Event>,
        trigger: Trigger<Self::Event>,
    ) {
        match trigger.event {
            Events::IscRetireDOp(dop) => {
                self.dops.push(dop);
            }
            Events::NotifyStartOnTimeout { .. } => {
                self.timeouts += 1;
            }
            Events::PePbsLaunchProcessing(batch_size) => {
                self.pbs_batches += 1;
                self.pbs_slots_filled += batch_size;
                self.pbs_launched_at = Some(trigger.at);
            }
            Events::PePbsLandProcessing(_) => {
                let launched_at = self
                    .pbs_launched_at
                    .take()
                    .expect("PBS batch landed without a matching launch");
                self.pbs_busy = self.pbs_busy + (trigger.at - launched_at);
            }
            _ => {}
        }
    }
}
