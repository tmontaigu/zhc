use super::*;
use std::{
    ptr::null_mut,
    sync::{
        Arc, Barrier,
        atomic::{AtomicBool, AtomicPtr, AtomicU64},
    },
};
use zhc_config::vm::VmConfig;
use zhc_utils::{SafeAs, Store, topology::Topology};

pub struct State {
    pub config: VmConfig,
    pub storage_regs: u16,
    pub run: AtomicPtr<Run>,
    pub barrier: Barrier,
    pub storages: Store<StorageId, Storage>,
    pub spin_nanos: AtomicU64,
    pub exec_nanos: AtomicU64,
    pub wall_nanos: AtomicU64,
    pub drop: AtomicBool,
}

impl State {
    pub fn new(config: &VmConfig, topo: &Topology) -> Arc<Self> {
        assert!(config.regf_size.is_multiple_of(topo.n_memories()));
        let storage_regs = (config.regf_size / topo.n_memories()).sas();
        let run = AtomicPtr::new(null_mut());
        let barrier = Barrier::new(topo.n_processors() + 1);
        let drop = AtomicBool::new(false);
        let storages = topo
            .iter_all_memories()
            .map(|mem_dom| {
                let associated_processor = mem_dom.iter_associated_processors().next().unwrap();
                let storage = associated_processor.run_on(|| Storage::new(config, topo));
                storage
            })
            .collect();
        Arc::new(State {
            run,
            barrier,
            storages,
            drop,
            spin_nanos: AtomicU64::new(0),
            exec_nanos: AtomicU64::new(0),
            wall_nanos: AtomicU64::new(0),
            config: config.to_owned(),
            storage_regs,
        })
    }

    pub fn get_reg_ptr(&self, rid: RegId) -> *mut u64 {
        let (sid, rid) = (
            rid.0.div_euclid(self.storage_regs),
            rid.0.rem_euclid(self.storage_regs),
        );
        unsafe {
            self.storages[StorageId(sid)]
                .reg
                .ptr
                .add(rid as usize * self.config.big_ciphertext_size())
        }
    }
}

unsafe impl Send for State {}
unsafe impl Sync for State {}
