use super::*;
use std::{
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};
use tfhe::{
    core_crypto::prelude::{CiphertextModulus, *},
    integer::IntegerCiphertext,
    shortint::prelude::LweDimension,
};
use zhc_config::vm::VmConfig;
use zhc_langs::vmlang::VmByteCode;
use zhc_profiling::{interval_begin, interval_end};
use zhc_utils::{
    SafeAs,
    topology::{MemoryData, ProcessingData},
};

pub struct Worker {
    pub wid: WorkerId,
    pub sid: StorageId,
    pub config: VmConfig,
    pub pbs_buffers: ComputationBuffers,
    pub state: Arc<State>,
    pub spin_time: Duration,
    pub exec_time: Duration,
    pub processor: ProcessingData,
    #[allow(unused)]
    pub memory: MemoryData,
}

impl Worker {
    pub fn run(mut self) {
        self.processor.set_for_current();
        loop {
            self.state.barrier.wait();
            if self.state.drop.load(Ordering::Acquire) {
                break;
            }
            let run = unsafe { &*self.state.run.load(Ordering::Acquire) };
            let bytecode: &[VmByteCode] = &run.bytecodes[self.wid];
            let mut t = Instant::now();
            for instr in bytecode.iter() {
                let id = instr.get_id();
                if run.locks[id as usize].load(Ordering::Acquire) > 0 {
                    interval_begin(c"Spin", self.wid.0 as u64);
                    while run.locks[id as usize].load(Ordering::Acquire) > 0 {
                        std::hint::spin_loop();
                    }
                    interval_end(c"Spin", self.wid.0 as u64);
                }
                self.spin_time += t.elapsed();
                t = Instant::now();
                self.exec(&instr);
                self.exec_time += t.elapsed();
                t = Instant::now();
                for s in run.successors[id as usize].iter() {
                    run.locks[*s as usize].fetch_sub(1, Ordering::Release);
                }
            }
            self.state
                .spin_nanos
                .fetch_add(self.spin_time.as_nanos() as u64, Ordering::Relaxed);
            self.state
                .exec_nanos
                .fetch_add(self.exec_time.as_nanos() as u64, Ordering::Relaxed);
            self.spin_time = Duration::ZERO;
            self.exec_time = Duration::ZERO;
            self.state.barrier.wait();
        }
    }

    fn get_bsk<'a>(&self) -> FourierLweBootstrapKey<&'a [c64]> {
        let bsk = unsafe {
            std::slice::from_raw_parts(
                self.state.storages[self.sid].bsk.ptr,
                self.config.bsk_alloc_size(),
            )
        };
        FourierLweBootstrapKey::from_container(
            bsk,
            LweDimension(self.config.lwe_dim),
            GlweSize(self.config.bsk_glwe_dim + 1),
            PolynomialSize(self.config.bsk_polynomial_size),
            DecompositionBaseLog(self.config.bsk_dec_base_log),
            DecompositionLevelCount(self.config.bsk_dec_levels),
        )
    }

    fn get_ksk<'a>(&self) -> LweKeyswitchKey<&'a [u32]> {
        let ksk = unsafe {
            std::slice::from_raw_parts(
                self.state.storages[self.sid].ksk.ptr,
                self.config.ksk_alloc_size(),
            )
        };
        LweKeyswitchKey::from_container(
            ksk,
            DecompositionBaseLog(self.config.ksk_dec_base_log),
            DecompositionLevelCount(self.config.ksk_dec_levels),
            LweSize(self.config.lwe_dim + 1),
            CiphertextModulus::new_native(),
        )
    }

    fn get_lut<'a>(&self, lut_id: usize) -> GlweCiphertext<&'a [u64]> {
        GlweCiphertext::from_container(
            unsafe {
                std::slice::from_raw_parts(
                    self.state.storages[self.sid]
                        .luts
                        .ptr
                        .add(lut_id * self.config.lut_alloc_size()),
                    self.config.lut_alloc_size(),
                )
            },
            PolynomialSize(self.config.bsk_polynomial_size),
            CiphertextModulus::new_native(),
        )
    }

    fn get_big_dst_reg<'a>(&self, rid: usize) -> LweCiphertext<&'a mut [u64]> {
        LweCiphertext::from_container(
            unsafe {
                std::slice::from_raw_parts_mut(
                    self.state.get_reg_ptr(RegId(rid.sas())),
                    self.config.big_ciphertext_size(),
                )
            },
            CiphertextModulus::new_native(),
        )
    }

    fn get_big_src_reg<'a>(&self, rid: usize) -> LweCiphertext<&'a [u64]> {
        LweCiphertext::from_container(
            unsafe {
                std::slice::from_raw_parts(
                    self.state.get_reg_ptr(RegId(rid.sas())),
                    self.config.big_ciphertext_size(),
                )
            },
            CiphertextModulus::new_native(),
        )
    }

    fn get_small_dst_reg<'a>(&self, rid: usize) -> LweCiphertext<&'a mut [u32]> {
        LweCiphertext::from_container(
            unsafe {
                std::slice::from_raw_parts_mut(
                    self.state.get_reg_ptr(RegId(rid.sas())) as *mut u32,
                    self.config.small_ciphertext_size(),
                )
            },
            CiphertextModulus::new_native(),
        )
    }

    fn get_small_src_reg<'a>(&self, rid: usize) -> LweCiphertext<&'a [u32]> {
        LweCiphertext::from_container(
            unsafe {
                std::slice::from_raw_parts(
                    self.state.get_reg_ptr(RegId(rid.sas())) as *const u32,
                    self.config.small_ciphertext_size(),
                )
            },
            CiphertextModulus::new_native(),
        )
    }

    fn get_run(&self) -> &Run {
        unsafe { &*self.state.run.load(Ordering::Acquire) }
    }

    fn get_pt_src(&self, id: usize, blk: usize) -> u64 {
        let Value::Uint(val) = self.get_run().inputs[id] else {
            unreachable!()
        };
        val.get_block(blk.sas()).raw_message_bits() as u64
    }

    fn get_ct_src(&self, id: usize, blk: usize) -> LweCiphertext<&[u64]> {
        let Value::FheUint(val) = self.get_run().inputs[id] else {
            unreachable!()
        };
        let radix_ct = unsafe { &*val };
        radix_ct.blocks()[blk].ct.as_view()
    }

    fn get_ct_dst(&self, id: usize, blk: usize) -> LweCiphertext<&mut [u64]> {
        let ValueMut::FheUint(val) = self.get_run().outputs[id];
        let radix_ct = unsafe { &mut *val };
        radix_ct.blocks_mut()[blk].ct.as_mut_view()
    }

    fn exec(&mut self, op: &VmByteCode) {
        use VmByteCode::*;
        match op {
            ADD {
                dst, src1, src2, ..
            } => {
                let mut out = self.get_big_dst_reg(*dst as usize);
                let lhs = self.get_big_src_reg(*src1 as usize);
                let rhs = self.get_big_src_reg(*src2 as usize);
                lwe_ciphertext_add(&mut out, &lhs, &rhs);
            }
            SUB {
                dst, src1, src2, ..
            } => {
                let mut out = self.get_big_dst_reg(*dst as usize);
                let lhs = self.get_big_src_reg(*src1 as usize);
                let rhs = self.get_big_src_reg(*src2 as usize);
                lwe_ciphertext_sub(&mut out, &lhs, &rhs);
            }
            MAC {
                dst,
                src1,
                src2,
                cst,
                ..
            } => {
                let mut out = self.get_big_dst_reg(*dst as usize);
                let lhs = self.get_big_src_reg(*src1 as usize);
                let rhs = self.get_big_src_reg(*src2 as usize);
                lwe_ciphertext_cleartext_mul(&mut out, &lhs, Cleartext(*cst as u64));
                lwe_ciphertext_add_assign(&mut out, &rhs);
            }
            ADDC { dst, src, cst, .. } => {
                let mut out = self.get_big_dst_reg(*dst as usize);
                let src = self.get_big_src_reg(*src as usize);
                out.as_mut_view()
                    .into_container()
                    .clone_from_slice(src.as_view().into_container());
                let pt = Plaintext(*cst as u64 * self.config.delta as u64);
                lwe_ciphertext_plaintext_add_assign(&mut out, pt);
            }
            SUBC { dst, src, cst, .. } => {
                let mut out = self.get_big_dst_reg(*dst as usize);
                let src = self.get_big_src_reg(*src as usize);
                out.as_mut_view()
                    .into_container()
                    .clone_from_slice(src.as_view().into_container());
                let pt = Plaintext(*cst as u64 * self.config.delta as u64);
                lwe_ciphertext_plaintext_sub_assign(&mut out, pt);
            }
            CSUB { dst, src, cst, .. } => {
                let mut out = self.get_big_dst_reg(*dst as usize);
                let src = self.get_big_src_reg(*src as usize);
                out.as_mut_view()
                    .into_container()
                    .clone_from_slice(src.as_view().into_container());
                lwe_ciphertext_opposite_assign(&mut out);
                let pt = Plaintext(*cst as u64 * self.config.delta as u64);
                lwe_ciphertext_plaintext_add_assign(&mut out, pt);
            }
            MULC { dst, src, cst, .. } => {
                let mut out = self.get_big_dst_reg(*dst as usize);
                let src = self.get_big_src_reg(*src as usize);
                lwe_ciphertext_cleartext_mul(&mut out, &src, Cleartext(*cst as u64));
            }
            ADDS {
                dst,
                src,
                s_id,
                s_blk,
                ..
            } => {
                let mut out = self.get_big_dst_reg(*dst as usize);
                let src = self.get_big_src_reg(*src as usize);
                out.as_mut_view()
                    .into_container()
                    .clone_from_slice(src.as_view().into_container());
                let pt = self.get_pt_src(*s_id as usize, *s_blk as usize);
                let pt = Plaintext(pt * self.config.delta as u64);
                lwe_ciphertext_plaintext_add_assign(&mut out, pt);
            }
            SUBS {
                dst,
                src,
                s_id,
                s_blk,
                ..
            } => {
                let mut out = self.get_big_dst_reg(*dst as usize);
                let src = self.get_big_src_reg(*src as usize);
                out.as_mut_view()
                    .into_container()
                    .clone_from_slice(src.as_view().into_container());
                let pt = self.get_pt_src(*s_id as usize, *s_blk as usize);
                let pt = Plaintext(pt * self.config.delta as u64);
                lwe_ciphertext_plaintext_sub_assign(&mut out, pt);
            }
            SSUB {
                dst,
                src,
                s_id,
                s_blk,
                ..
            } => {
                let mut out = self.get_big_dst_reg(*dst as usize);
                let src = self.get_big_src_reg(*src as usize);
                out.as_mut_view()
                    .into_container()
                    .clone_from_slice(src.as_view().into_container());
                lwe_ciphertext_opposite_assign(&mut out);
                let pt = self.get_pt_src(*s_id as usize, *s_blk as usize);
                let pt = Plaintext(pt * self.config.delta as u64);
                lwe_ciphertext_plaintext_add_assign(&mut out, pt);
            }
            MULS {
                dst,
                src,
                s_id,
                s_blk,
                ..
            } => {
                let mut out = self.get_big_dst_reg(*dst as usize);
                let src = self.get_big_src_reg(*src as usize);
                let ct = self.get_pt_src(*s_id as usize, *s_blk as usize);
                lwe_ciphertext_cleartext_mul(&mut out, &src, Cleartext(ct));
            }
            LD {
                dst,
                src_id,
                src_blk,
                ..
            } => {
                let mut out = self.get_big_dst_reg(*dst as usize);
                let src = self.get_ct_src(*src_id as usize, *src_blk as usize);
                out.as_mut_view()
                    .into_container()
                    .clone_from_slice(src.as_view().into_container());
            }
            ST {
                dst_id,
                dst_blk,
                src,
                ..
            } => {
                let mut out = self.get_ct_dst(*dst_id as usize, *dst_blk as usize);
                let src = self.get_big_src_reg(*src as usize);
                out.as_mut_view()
                    .into_container()
                    .clone_from_slice(src.as_view().into_container());
            }
            KS { dst, src, .. } => {
                interval_begin(c"KS", self.wid.0 as u64);
                let mut out = self.get_small_dst_reg(*dst as usize);
                let src = self.get_big_src_reg(*src as usize);
                let ksk = self.get_ksk();
                keyswitch_lwe_ciphertext_with_scalar_change(&ksk, &src, &mut out);
                interval_end(c"KS", self.wid.0 as u64);
            }
            PBS { dst, src, lut, .. } => {
                interval_begin(c"PBS", self.wid.0 as u64);
                let out = self.get_big_dst_reg(*dst as usize);
                let src = self.get_small_src_reg(*src as usize);
                let bsk = self.get_bsk();
                let lut = self.get_lut(*lut as usize);
                bsk.bootstrap_ml(
                    &mut [out],
                    src,
                    lut,
                    self.state.storages[self.sid].fft.as_view(),
                    self.pbs_buffers.stack(),
                );
                interval_end(c"PBS", self.wid.0 as u64);
            }
            PBS_ML2 {
                dst1,
                dst2,
                src,
                lut,
                ..
            } => {
                interval_begin(c"PBS", self.wid.0 as u64);
                let out1 = self.get_big_dst_reg(*dst1 as usize);
                let out2 = self.get_big_dst_reg(*dst2 as usize);
                let src = self.get_small_src_reg(*src as usize);
                let bsk = self.get_bsk();
                let lut = self.get_lut(*lut as usize);
                bsk.bootstrap_ml(
                    &mut [out1, out2],
                    src,
                    lut,
                    self.state.storages[self.sid].fft.as_view(),
                    self.pbs_buffers.stack(),
                );
                interval_end(c"PBS", self.wid.0 as u64);
            }
            DEF { dst, cst, .. } => {
                let mut out = self.get_big_dst_reg(*dst as usize);
                out.as_mut_view().into_container().fill(0);
                *(out.get_mut_body().data) = *cst as u64 * self.config.delta as u64;
            }
        }
    }
}
