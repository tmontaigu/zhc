use std::collections::HashMap;

use zhc_crypto::integer_semantics::CiphertextSpec;
use zhc_langs::ioplang::{Lut1Def, Lut2Def};
use zhc_utils::{
    iter::{ChunkIt, CollectInSmallVec, IterMapFirst, MultiZip, ReconcilerOf2, Slide, SliderExt},
    svec,
};

use crate::{
    CiphertextBlock,
    builder::{Builder, Ciphertext, ExtensionBehavior},
};

/// Creates an IR for the addition of two encrypted integers.
///
/// The returned [`Builder`] declares two ciphertext inputs and one ciphertext
/// output representing the wrapping sum of the operands. Internally the
/// addition uses [`Builder::iop_add_hillis_steele`] for carry propagation.
///
/// The `spec` parameter describes the integer encoding (bit-width, message
/// bits, carry bits) and determines the number of blocks in the
/// decomposition.
///
/// # Examples
///
/// ```rust,no_run
/// # use zhc_builder::{CiphertextSpec, add};
/// # let spec = CiphertextSpec::new(16, 2, 2);
/// let builder = add(spec);
/// let ir = builder.optimize_ir();
/// ```
pub fn add(spec: CiphertextSpec) -> Builder {
    let builder = Builder::new(spec.block_spec());
    let src_a = builder.ciphertext_input(spec.int_size());
    let src_b = builder.ciphertext_input(spec.int_size());
    let par_w = match spec.int_size() {
        8..16 => 1,
        16..24 => 7,
        24..256 => 12,
        _ => 1,
    };
    let res = match spec.int_size() {
        0..8 => builder.iop_ripple_carry_add(&src_a, &src_b, None).0,
        8..17 => builder.iop_add_hillis_steele(&src_a, &src_b, None),
        17..256 => builder.iop_add_kogge_stone(&src_a, &src_b, None, par_w),
        _ => todo!(),
    };
    builder.ciphertext_output(res);
    builder
}

/// Creates an IR for the subtraction of two encrypted integers.
///
/// The returned [`Builder`] declares two ciphertext inputs and one ciphertext output
/// representing the wrapping difference `a - b`. Internally the subtraction is computed
/// as `a + (!b) + 1` using two's complement, reusing the addition infrastructure with a
/// carry-in of one.
///
/// # Examples
///
/// ```rust,no_run
/// # use zhc_builder::{CiphertextSpec, sub};
/// # let spec = CiphertextSpec::new(16, 2, 2);
/// let builder = sub(spec);
/// let ir = builder.optimize_ir();
/// ```
pub fn sub(spec: CiphertextSpec) -> Builder {
    let builder = Builder::new(spec.block_spec());
    let src_a = builder.ciphertext_input(spec.int_size());
    let src_b = builder.ciphertext_input(spec.int_size());
    let par_w = match spec.int_size() {
        8..16 => 1,
        16..24 => 7,
        24..256 => 12,
        _ => 1,
    };
    let one = builder.block_let_ciphertext(1);
    let b_inv = builder.iop_bitwise_inv(&src_b);
    let res = match spec.int_size() {
        0..8 => builder.iop_ripple_carry_add(&src_a, &b_inv, Some(&one)).0,
        8..17 => builder.iop_add_hillis_steele(&src_a, &b_inv, Some(&one)),
        17..256 => builder.iop_add_kogge_stone(&src_a, &b_inv, Some(&one), par_w),
        _ => todo!(),
    };
    builder.ciphertext_output(res);
    builder
}

/// Creates an IR for the addition of two encrypted integers using Kogge-Stone
/// carry propagation.
///
/// The returned [`Builder`] declares two ciphertext inputs and one ciphertext
/// output representing the wrapping sum of the operands. Internally the
/// addition uses [`Builder::iop_add_kogge_stone`] for carry propagation.
pub fn add_kogge_stone(spec: CiphertextSpec, par_w: usize) -> Builder {
    let builder = Builder::new(spec.block_spec());
    let src_a = builder.ciphertext_input(spec.int_size());
    let src_b = builder.ciphertext_input(spec.int_size());
    let res = builder.iop_add_kogge_stone(&src_a, &src_b, None, par_w);
    builder.ciphertext_output(res);
    builder
}

/// Creates an IR for the addition of two encrypted integers using Hillis-Steele carry
/// propagation.
///
/// The returned [`Builder`] declares two ciphertext inputs and one ciphertext output
/// representing the wrapping sum of the operands. This variant explicitly selects the
/// Hillis-Steele algorithm, which groups blocks into fours and resolves carries with
/// logarithmic depth. Prefer [`add`] for automatic algorithm selection based on bit-width.
///
/// # Examples
///
/// ```rust,no_run
/// # use zhc_builder::{CiphertextSpec, add_hillis_steele};
/// # let spec = CiphertextSpec::new(16, 2, 2);
/// let builder = add_hillis_steele(spec);
/// let ir = builder.optimize_ir();
/// ```
pub fn add_hillis_steele(spec: CiphertextSpec) -> Builder {
    let builder = Builder::new(spec.block_spec());
    let src_a = builder.ciphertext_input(spec.int_size());
    let src_b = builder.ciphertext_input(spec.int_size());
    let res = builder.iop_add_hillis_steele(&src_a, &src_b, None);
    builder.ciphertext_output(res);
    builder
}

/// Creates an IR for the addition of two encrypted integers using ripple-carry propagation.
///
/// The returned [`Builder`] declares two ciphertext inputs and one ciphertext output
/// representing the wrapping sum of the operands. This variant explicitly selects the
/// ripple-carry algorithm, which processes blocks sequentially from LSB to MSB. Suitable
/// for small bit-widths where the simpler structure outweighs parallelism benefits.
///
/// # Examples
///
/// ```rust,no_run
/// # use zhc_builder::{CiphertextSpec, add_ripple};
/// # let spec = CiphertextSpec::new(8, 2, 2);
/// let builder = add_ripple(spec);
/// let ir = builder.optimize_ir();
/// ```
pub fn add_ripple(spec: CiphertextSpec) -> Builder {
    let builder = Builder::new(spec.block_spec());
    let src_a = builder.ciphertext_input(spec.int_size());
    let src_b = builder.ciphertext_input(spec.int_size());
    let (res, _carry_out) = builder.iop_ripple_carry_add(&src_a, &src_b, None);
    builder.ciphertext_output(res);
    builder
}

/// Creates an IR for the addition of two encrypted integers with overflow detection.
///
/// The returned [`Builder`] declares two ciphertext inputs and two ciphertext outputs.
/// The first output is the wrapping sum.
/// The second output is an overflow flag in a bool ciphertext (1 block)
pub fn overflow_add(spec: CiphertextSpec) -> Builder {
    let builder = Builder::new(spec.block_spec());
    let src_a = builder.ciphertext_input(spec.int_size());
    let src_b = builder.ciphertext_input(spec.int_size());
    let par_w = match spec.int_size() {
        8..16 => 1,
        16..24 => 7,
        24..256 => 12,
        _ => 1,
    };
    let (res, carry_out) = match spec.int_size() {
        0..8 => builder.iop_ripple_carry_add(&src_a, &src_b, None),
        8..17 => {
            let lhs = builder.ciphertext_split(&src_a);
            let rhs = builder.ciphertext_split(&src_b);
            let (blocks, co) = builder.iop_add_hillis_steele_raw(lhs, rhs, None, true);
            (builder.comment("Join").ciphertext_join(blocks, None), co)
        }
        17..256 => {
            let lhs = builder.ciphertext_split(&src_a);
            let rhs = builder.ciphertext_split(&src_b);
            let (blocks, co) = builder.iop_add_kogge_stone_raw(lhs, rhs, None, par_w, false);
            (builder.comment("Join").ciphertext_join(blocks, None), co)
        }
        _ => todo!(),
    };

    let flag = builder.ciphertext_join(&[carry_out], None);

    builder.ciphertext_output(res);
    builder.ciphertext_output(flag);
    builder
}

/// Creates an IR for the subtraction of two encrypted integers with overflow detection.
///
/// The returned [`Builder`] declares two ciphertext inputs and two ciphertext outputs.
/// The first output is the wrapping difference `a - b`.
/// The second output is an overflow (borrow) flag in a bool ciphertext (1 block)
///
/// Overflow means `b > a` (unsigned underflow). Internally computes `a + (!b) + 1` and
/// inverts the carry-out: carry=1 means no borrow, carry=0 means borrow.
pub fn overflow_sub(spec: CiphertextSpec) -> Builder {
    let builder = Builder::new(spec.block_spec());
    let src_a = builder.ciphertext_input(spec.int_size());
    let src_b = builder.ciphertext_input(spec.int_size());
    let par_w = match spec.int_size() {
        8..16 => 1,
        16..24 => 7,
        24..256 => 12,
        _ => 1,
    };
    let one = builder.block_let_ciphertext(1);
    let b_inv = builder.iop_bitwise_inv(&src_b);
    let (res, carry_out) = match spec.int_size() {
        0..8 => builder.iop_ripple_carry_add(&src_a, &b_inv, Some(&one)),
        8..17 => {
            let lhs = builder.ciphertext_split(&src_a);
            let rhs = builder.ciphertext_split(&b_inv);
            let (blocks, co) = builder.iop_add_hillis_steele_raw(lhs, rhs, Some(&one), true);
            (builder.comment("Join").ciphertext_join(blocks, None), co)
        }
        17..256 => {
            let lhs = builder.ciphertext_split(&src_a);
            let rhs = builder.ciphertext_split(&b_inv);
            let (blocks, co) = builder.iop_add_kogge_stone_raw(lhs, rhs, Some(&one), par_w, false);
            (builder.comment("Join").ciphertext_join(blocks, None), co)
        }
        _ => todo!(),
    };

    // For sub: carry_out=1 means NO overflow (a >= b), carry_out=0 means overflow (a < b).
    let overflow_flag = builder.block_lookup(&carry_out, Lut1Def::IsNull);
    let flag = builder.ciphertext_join(&[overflow_flag], None);

    builder.ciphertext_output(res);
    builder.ciphertext_output(flag);
    builder
}

impl Builder {
    /// Adds two encrypted integers using sequential ripple-carry propagation.
    ///
    /// Processes blocks from LSB to MSB, computing each block's sum and carry in turn.
    /// The optional `cin` injects an initial carry (useful for subtraction via two's
    /// complement). Each block requires two PBS operations: one to extract the message
    /// and one to extract the carry.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_builder::{CiphertextSpec, Builder};
    /// # let spec = CiphertextSpec::new(8, 2, 2);
    /// # let builder = Builder::new(spec.block_spec());
    /// # let a = builder.ciphertext_input(spec.int_size());
    /// # let b = builder.ciphertext_input(spec.int_size());
    /// let (sum, carry_out) = builder.iop_ripple_carry_add(&a, &b, None);
    /// ```
    pub fn iop_ripple_carry_add(
        &self,
        lhs: &Ciphertext,
        rhs: &Ciphertext,
        cin: Option<&CiphertextBlock>,
    ) -> (Ciphertext, CiphertextBlock) {
        let lhs_blocks = self.ciphertext_split(lhs);
        let rhs_blocks = self.ciphertext_split(rhs);

        let mut carry = cin.cloned().unwrap_or_else(|| self.block_let_ciphertext(0));
        let mut output_blocks = Vec::new();
        for i in 0..lhs_blocks.iter().len() {
            self.push_comment(format!("{i}-th"));
            let raw_sum = self.block_add(lhs_blocks[i], rhs_blocks[i]);
            let sum = self.block_add(raw_sum, carry);
            let message = self.block_lookup(sum, Lut1Def::MsgOnly);
            carry = self.block_lookup(sum, Lut1Def::CarryInMsg);
            output_blocks.push(message);
            self.pop_comment();
        }

        // carry is now the carry-out of the last block (clean 0/1 via CarryInMsg)
        let joined = self.comment("Join").ciphertext_join(output_blocks, None);
        (joined, carry)
    }

    /// Adds two encrypted integers using Hillis-Steele carry propagation.
    ///
    /// Groups blocks into fours, computes per-group propagation states, then resolves
    /// inter-group carries with a parallel prefix scan. The optional `cin` injects an
    /// initial carry into the LSB position. This algorithm offers O(log n) depth for
    /// n groups, making it efficient for medium-width integers (roughly 8–16 blocks).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_builder::{CiphertextSpec, Builder};
    /// # let spec = CiphertextSpec::new(16, 2, 2);
    /// # let builder = Builder::new(spec.block_spec());
    /// # let a = builder.ciphertext_input(spec.int_size());
    /// # let b = builder.ciphertext_input(spec.int_size());
    /// let sum = builder.iop_add_hillis_steele(&a, &b, None);
    /// ```
    pub fn iop_add_hillis_steele(
        &self,
        lhs: &Ciphertext,
        rhs: &Ciphertext,
        cin: Option<&CiphertextBlock>,
    ) -> Ciphertext {
        let lhs_blocks = self.ciphertext_split(lhs);
        let rhs_blocks = self.ciphertext_split(rhs);

        let (output_blocks, _carry_out) =
            self.iop_add_hillis_steele_raw(lhs_blocks, rhs_blocks, cin, true);

        self.comment("Join").ciphertext_join(output_blocks, None)
    }

    pub fn iop_add_hillis_steele_raw(
        &self,
        lhs_blocks: impl AsRef<[CiphertextBlock]>,
        rhs_blocks: impl AsRef<[CiphertextBlock]>,
        cin: Option<&CiphertextBlock>,
        clean: bool,
    ) -> (Vec<CiphertextBlock>, CiphertextBlock) {
        // Implements the addition with carry-propagation using the hillis-steele resolution and
        // group of size 4. The encoding of propagation status is the same as the one used
        // in TFHE-RS. The carry is resolved as soon as possible.
        //
        // Dead code elimination
        // =====================
        //
        // Depending on the size of the input integer, the computation may require some care to
        // handle edge cases:
        // + Non multiple of 4 number of blocks (due to 4-grouping for status computation)
        // + Non power-of-two number of groups (due to the hillis-steele resolution)
        //
        // Hopefully, thanks to dead-code elimination happening down the pipeline, we can describe
        // the computation in a larger, more favorable case, and let DCE cut the un-necessary
        // computation. This improves code readability.

        let mut sums = self.comment("Raw sum").vector_add(
            &lhs_blocks,
            &rhs_blocks,
            ExtensionBehavior::Passthrough,
        );
        if let Some(c) = cin {
            sums[0] = self.block_add(&sums[0], c);
        }

        let output_size = sums.len();
        let compute_size = sums.len().next_multiple_of(4).next_power_of_two();
        let sums = self
            .comment("Extend sum")
            .vector_unsigned_extension(sums, compute_size);

        self.push_comment("Block States");
        let block_states = sums
            .iter()
            .chunk(4)
            .map(|c| c.unwrap_complete())
            .map_first(|sum| {
                [
                    self.comment("G0-B0")
                        .block_lookup2(sum[0], Lut2Def::ManyCarryMsg)
                        .1,
                    self.comment("G0-B1")
                        .block_lookup(sum[1], Lut1Def::ExtractPropGroup0),
                    self.comment("G0-B2")
                        .block_lookup(sum[2], Lut1Def::ExtractPropGroup1),
                    self.comment("G0-B3")
                        .block_lookup(sum[3], Lut1Def::ExtractPropGroup2),
                ]
            })
            .map_rest(|sum| {
                [
                    self.comment("GN-B0")
                        .block_lookup(sum[0], Lut1Def::ExtractPropGroup0),
                    self.comment("GN-B1")
                        .block_lookup(sum[1], Lut1Def::ExtractPropGroup1),
                    self.comment("GN-B2")
                        .block_lookup(sum[2], Lut1Def::ExtractPropGroup2),
                    self.comment("GN-B3")
                        .block_padding_lookup(sum[3], Lut1Def::ExtractPropGroup3),
                ]
            })
            .cosvec();
        self.pop_comment();

        self.push_comment("Group states");
        let group_states = block_states
            .iter()
            .map_first(|states| {
                // NB: group #0 is particular, since the status is actually
                // the carry value => This group is directly solved
                let b0 = states[0];
                let b1 = self.block_add(&b0, &states[1]);
                let b2 = self.block_add(&b1, &states[2]);
                let b3 = self.block_temper_add(&b2, &states[3]);
                let b3 = self.block_lookup(&b3, Lut1Def::SolvePropGroupFinal2);
                [
                    self.comment("G0-B0").block_inspect(b0),
                    self.comment("G0-B1").block_inspect(b1),
                    self.comment("G0-B2").block_inspect(b2),
                    self.comment("G0-B3").block_inspect(b3),
                ]
            })
            .map_rest(|states| {
                let b0 = states[0];
                let b1 = self.block_add(&b0, &states[1]);
                let b2 = self.block_add(&b1, &states[2]);
                let b3 = self.block_temper_add(&b2, &states[3]);
                let b3 = self.block_wrapping_lookup(&b3, Lut1Def::ReduceCarryPad);
                let b3 = self.block_wrapping_add_plaintext(&b3, &self.block_let_plaintext(1));
                [
                    self.comment("GN-B0").block_inspect(b0),
                    self.comment("GN-B1").block_inspect(b1),
                    self.comment("GN-B2").block_inspect(b2),
                    self.comment("GN-B3").block_inspect(b3),
                ]
            })
            .cosvec();
        self.pop_comment();

        self.push_comment("Group carries");
        let mut group_carries = group_states.iter().map(|group| group[3]).cosvec();
        let nb_groups = group_carries.len();
        let nb_stages = (nb_groups as f32).log2().ceil() as usize;
        for stage in 0..nb_stages {
            self.push_comment(format!("HS {stage}-th stage"));
            let stride = 1usize << stage;
            group_carries = group_carries
                .into_iter()
                // We chunk by increasing stride, and assume complete chunks.
                .chunk(stride)
                .map(|c| c.unwrap_complete())
                // We need to assemble data from two chunks later down the pipe.
                // Prelude will be useful for the first chunk, as we will see,
                // but Postlude is not needed.
                .slide::<2>()
                .skip_postludes()
                // The first chunk of the result is already solved at the previous level.
                // We get it from the prelude of the slide, and call it a day.
                .map_first(|slider| {
                    let sv = slider.unwrap_prelude();
                    sv[0].clone().into_iter().reconcile_1_of_2()
                })
                // The next chunk combines two chunks of the previous stage with the carry lut.
                .map_first(|slider| {
                    let [prev_carry, status] = slider.unwrap_complete().into_array();
                    self.vector_zip_then_lookup(
                        status,
                        prev_carry,
                        Lut1Def::SolvePropCarry,
                        ExtensionBehavior::Panic,
                    )
                    .into_iter()
                    .reconcile_2_of_2()
                })
                // The rest of the chunks combine chunks of the previous stage with the prop lut.
                .map_rest(|slider| {
                    let [prev_carry, status] = slider.unwrap_complete().into_array();
                    self.vector_zip_then_lookup(
                        status,
                        prev_carry,
                        Lut1Def::SolveProp,
                        ExtensionBehavior::Panic,
                    )
                    .into_iter()
                    .reconcile_2_of_2()
                })
                .flatten()
                // We only take enough to build the new iterate.
                .take(nb_groups)
                .collect();
            assert_eq!(group_carries.len(), nb_groups);
            self.pop_comment();
        }
        self.pop_comment();

        self.push_comment("Final resolution");
        let carries = (group_states.into_iter(), group_carries.into_iter())
            .mzip()
            .slide::<2>()
            .skip_postludes()
            .map_first(|slider| {
                let (states, carry) = slider.unwrap_prelude()[0];
                let b1 = self.block_lookup(&states[1], Lut1Def::SolvePropGroupFinal0);
                let b2 = self.block_lookup(&states[2], Lut1Def::SolvePropGroupFinal1);
                [
                    self.comment("G0-B0").block_inspect(states[0]),
                    self.comment("G0-B1").block_inspect(b1),
                    self.comment("G0-B2").block_inspect(b2),
                    self.comment("G0-B3").block_inspect(carry),
                ]
            })
            .map_rest(|slider| {
                let [(_, previous_carry), (states, carry)] = slider.unwrap_complete().into_array();
                let b0 = self.block_add(&states[0], &previous_carry);
                let b0 = self.block_lookup(&b0, Lut1Def::SolvePropGroupFinal0);
                let b1 = self.block_add(&states[1], &previous_carry);
                let b1 = self.block_lookup(&b1, Lut1Def::SolvePropGroupFinal1);
                let b2 = self.block_add(&states[2], &previous_carry);
                let b2 = self.block_lookup(&b2, Lut1Def::SolvePropGroupFinal2);
                [
                    self.comment("GN-B0").block_inspect(b0),
                    self.comment("GN-B1").block_inspect(b1),
                    self.comment("GN-B2").block_inspect(b2),
                    self.comment("GN-B3").block_inspect(carry),
                ]
            })
            .flatten()
            .cosvec();
        self.pop_comment();

        self.push_comment("Carry propagation");
        let mut result = svec![self.block_lookup2(&sums[0], Lut2Def::ManyCarryMsg).0];
        result.extend(
            (sums.into_iter().skip(1), carries.into_iter())
                .mzip()
                .map(|(sum, carry)| self.block_add(&sum, &carry)),
        );
        self.pop_comment();

        // Carry-out: the last result block (before cleanup) has the carry-out
        // in its carry field. Extract it before MsgOnly strips carry info.
        let carry_out = self.block_lookup(&result[output_size - 1], Lut1Def::CarryIsSome);

        if clean {
            self.push_comment("Cleanup");
            result = result
                .into_iter()
                .map(|ct| self.block_lookup(&ct, Lut1Def::MsgOnly))
                .cosvec();
            self.pop_comment();
        }

        (result.as_slice()[..output_size].into(), carry_out)
    }
}

// ---------------------------------------------------------------------------
// Kogge-Stone carry propagation
// ---------------------------------------------------------------------------

/// A single entry in the Kogge tree, holding both the raw accumulated value
/// and a reduced (fresh) PG-encoded version.
#[derive(Copy, Clone, Debug)]
struct KoggeEntry {
    /// Raw accumulated MAC value (may span multiple PG positions).
    block: CiphertextBlock,
    /// Bit-width of the raw value (number of PG positions accumulated).
    cpos: usize,
    /// Reduced PG-encoded value (cpos conceptually == 1).
    fresh: CiphertextBlock,
}

/// Kogge-Stone prefix tree over PG-encoded carry values.
///
/// Mirrors the `KoggeTree` in `tfhe-rs/.../kogge.rs`. The tree lazily
/// computes prefix reductions using MAC (multiply-accumulate via doubling)
/// and PBS reduction operations (`ReduceCarry2`, `ReduceCarry3`,
/// `ReduceCarryPad`).
struct KoggeTree<'a> {
    builder: &'a Builder,
    cache: HashMap<(usize, usize), KoggeEntry>,
    /// `carry_size + message_size` for the block spec (4 for (2,2) params).
    total_width: usize,
}

impl<'a> KoggeTree<'a> {
    fn new(builder: &'a Builder, inputs: Vec<KoggeEntry>) -> Self {
        let total_width = builder.spec().data_size() as usize;
        let mut cache = HashMap::new();
        for (i, block) in inputs.into_iter().enumerate() {
            cache.insert((i, i), block);
        }
        KoggeTree {
            builder,
            cache,
            total_width,
        }
    }

    /// Splits a range into two sub-ranges using Kogge decomposition.
    /// Identical to the `get_subindex` logic in the tfhe-rs reference.
    fn get_subindex(start: usize, end: usize) -> ((usize, usize), (usize, usize)) {
        let range = end - start + 1;
        let pow = 1usize << range.ilog2();
        let mid = if pow == range {
            start + (pow >> 1)
        } else {
            start + pow
        };
        ((start, mid - 1), (mid, end))
    }

    /// Recursively builds the subtree for the range `[start, end]` and
    /// caches the result.
    fn insert_subtree(&mut self, start: usize, end: usize) {
        if self.cache.contains_key(&(start, end)) {
            return;
        }

        let ((ls, le), (ms, me)) = Self::get_subindex(start, end);
        self.insert_subtree(ls, le);
        self.insert_subtree(ms, me);

        let lsb = &self.cache[&(ls, le)];
        let msb = &self.cache[&(ms, me)];

        let cpos_trial = lsb.cpos + msb.cpos;

        // Choose which values to combine and the resulting cpos / shift.
        let (lsb_val, msb_val, cpos, msb_shift) = if cpos_trial > self.total_width {
            if msb.cpos + 1 > self.total_width {
                // Both sides must be reduced.
                (&lsb.fresh, &msb.fresh, 2usize, 2u8)
            } else {
                // Only lsb side needs reduction.
                (&lsb.fresh, &msb.block, msb.cpos + 1, 2u8)
            }
        } else {
            // Raw values fit without reduction.
            (&lsb.block, &msb.block, cpos_trial, 1 << lsb.cpos)
        };

        // MAC: lsb_val + (2^log_shift) * msb_val — implemented via doubling.
        let mac = self.builder.block_mac(msb_val, lsb_val, msb_shift);

        // Reduce via PBS based on cpos.
        let fresh = match cpos {
            2 => self.builder.block_lookup(&mac, Lut1Def::ReduceCarry2),
            3 => self.builder.block_lookup(&mac, Lut1Def::ReduceCarry3),
            tw if tw == self.total_width => {
                let r = self
                    .builder
                    .block_wrapping_lookup(&mac, Lut1Def::ReduceCarryPad);
                self.builder
                    .block_wrapping_add_plaintext(&r, &self.builder.block_let_plaintext(1))
            }
            _ => unreachable!(
                "Unexpected cpos={cpos} with total_width={}",
                self.total_width
            ),
        };

        // all carry processed are inserted in the cache
        self.cache.insert(
            (start, end),
            KoggeEntry {
                block: mac,
                cpos,
                fresh,
            },
        );
    }

    /// Returns the prefix entry for the range `[start, end]`.
    fn get_prefix(&mut self, start: usize, end: usize) -> &KoggeEntry {
        self.insert_subtree(start, end);
        // here returning only the needed carry
        // a part of the cached carry will be removed
        // by dead code removal
        &self.cache[&(start, end)]
    }
}

impl Builder {
    /// Adds two encrypted integers using Kogge-Stone carry propagation.
    ///
    /// Builds a prefix tree over generate-propagate (PG) encoded carries, lazily computing
    /// and reducing intermediate MAC values. The `par_w` parameter controls the chunk width:
    /// carries are resolved within each chunk, then chained across chunks. Larger `par_w`
    /// reduces PBS count at the cost of deeper trees; values around 7–12 work well for
    /// typical 16–64 bit integers.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_builder::{CiphertextSpec, Builder};
    /// # let spec = CiphertextSpec::new(32, 2, 2);
    /// # let builder = Builder::new(spec.block_spec());
    /// # let a = builder.ciphertext_input(spec.int_size());
    /// # let b = builder.ciphertext_input(spec.int_size());
    /// let sum = builder.iop_add_kogge_stone(&a, &b, None, 12);
    /// ```
    pub fn iop_add_kogge_stone(
        &self,
        lhs: &Ciphertext,
        rhs: &Ciphertext,
        cin: Option<&CiphertextBlock>,
        par_w: usize,
    ) -> Ciphertext {
        let lhs_blocks = self.ciphertext_split(lhs);
        let rhs_blocks = self.ciphertext_split(rhs);
        let (output_blocks, _carry_out) =
            self.iop_add_kogge_stone_raw(lhs_blocks, rhs_blocks, cin, par_w, false);
        self.comment("Join").ciphertext_join(output_blocks, None)
    }

    /// Raw Kogge-Stone addition on block slices, with optional carry-in and
    /// parallel-width chunking.
    pub(super) fn iop_add_kogge_stone_raw(
        &self,
        lhs_blocks: impl AsRef<[CiphertextBlock]>,
        rhs_blocks: impl AsRef<[CiphertextBlock]>,
        cin: Option<&CiphertextBlock>,
        par_w: usize,
        clean: bool,
    ) -> (Vec<CiphertextBlock>, CiphertextBlock) {
        let sums = self.comment("Raw sum").vector_add(
            &lhs_blocks,
            &rhs_blocks,
            ExtensionBehavior::Passthrough,
        );

        // Convert cin to PG encoding (or zero if absent).
        let cin_pg = match cin {
            Some(c) => self.block_lookup(c, Lut1Def::Ripple2GenProp),
            None => self.block_let_ciphertext(0),
        };
        let mut cin_pg_kogge_entry = KoggeEntry {
            block: cin_pg,
            cpos: 1,
            fresh: cin_pg,
        };

        let n = sums.len();
        let mut result = Vec::with_capacity(n);

        // Process chunks of par_w, chaining carry-out → carry-in.
        let mut pos = 0;
        while pos < n {
            let end = (pos + par_w).min(n);
            let chunk = &sums[pos..end];

            self.push_comment(format!("Kogge chunk [{pos}..{end})"));
            let (chunk_result, carry_out) = self.kogge_propagate_carry(chunk, &cin_pg_kogge_entry);
            self.pop_comment();

            result.extend(chunk_result);
            cin_pg_kogge_entry = carry_out.clone();
            pos = end;
        }

        // Carry-out: the final PG entry spans cin through all blocks.
        // After full resolution, PG is {0=kill, 1=generate} — directly 0 or 1.
        let carry_out = cin_pg_kogge_entry.fresh;

        if clean {
            self.push_comment("Cleanup");
            result = result
                .into_iter()
                .map(|ct| self.block_lookup(&ct, Lut1Def::MsgOnly))
                .collect();
            self.pop_comment();
        }

        (result, carry_out)
    }

    /// Propagates carries through a slice of carry-save sums using a Kogge
    /// tree. Returns `(output_blocks, carry_out_pg)`.
    fn kogge_propagate_carry(
        &self,
        sums: &[CiphertextBlock],
        cin_pg: &KoggeEntry,
    ) -> (Vec<CiphertextBlock>, KoggeEntry) {
        let n = sums.len();

        // Split each sum into (PG, msg) via ManyGenProp.
        let mut carry_vec = Vec::with_capacity(n + 1);
        let mut msgs = Vec::with_capacity(n);
        for (i, sum) in sums.iter().enumerate() {
            let (pg, msg) = self
                .comment(format!("GenProp {i}"))
                .block_lookup2(sum, Lut2Def::ManyGenProp);
            let pg_ke = KoggeEntry {
                block: pg,
                cpos: 1,
                fresh: pg,
            };
            carry_vec.push(pg_ke);
            msgs.push(msg);
        }

        // Build carry chain: [cin_pg, pg_0, pg_1, ..., pg_{n-1}]
        carry_vec.insert(0, cin_pg.clone());

        // Build Kogge carry_tree.
        let mut carry_tree = KoggeTree::new(self, carry_vec);

        // For each block, query the resolved carry and combine with msg.
        let mut output = Vec::with_capacity(n);
        for i in 0..n {
            let carry_fresh = carry_tree.get_prefix(0, i).fresh;
            // Pack carry_fresh (carry/high) + msg (message/low), then apply GenPropAdd.
            let resolved = self.block_pack_then_lookup(&carry_fresh, &msgs[i], Lut1Def::GenPropAdd);
            output.push(resolved);
        }

        // Carry-out is the prefix over the full chain.
        let cout = carry_tree.get_prefix(0, n);

        (output, *cout)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use zhc_langs::ioplang::IopValue;
    use zhc_utils::assert_display_is;

    #[test]
    fn test_add() {
        let spec = CiphertextSpec::new(18, 2, 2);
        let ir = add(spec).optimize_ir();
        assert_display_is!(
            ir.format()
                .with_walker(zhc_ir::PrintWalker::Linear)
                .show_comments(true)
                .show_types(false),
            r#"
                                                    | %0 = input_ciphertext<0, 18>();
                                                    | %1 = input_ciphertext<1, 18>();
                                                    | %2 = extract_ct_block<0>(%0);
                                                    | %3 = extract_ct_block<1>(%0);
                                                    | %4 = extract_ct_block<2>(%0);
                                                    | %5 = extract_ct_block<3>(%0);
                                                    | %6 = extract_ct_block<4>(%0);
                                                    | %7 = extract_ct_block<5>(%0);
                                                    | %8 = extract_ct_block<6>(%0);
                                                    | %9 = extract_ct_block<7>(%0);
                                                    | %10 = extract_ct_block<8>(%0);
                                                    | %11 = extract_ct_block<0>(%1);
                                                    | %12 = extract_ct_block<1>(%1);
                                                    | %13 = extract_ct_block<2>(%1);
                                                    | %14 = extract_ct_block<3>(%1);
                                                    | %15 = extract_ct_block<4>(%1);
                                                    | %16 = extract_ct_block<5>(%1);
                                                    | %17 = extract_ct_block<6>(%1);
                                                    | %18 = extract_ct_block<7>(%1);
                                                    | %19 = extract_ct_block<8>(%1);
                // Raw sum                          | %20 = add_ct(%2, %11);
                // Raw sum                          | %21 = add_ct(%3, %12);
                // Raw sum                          | %22 = add_ct(%4, %13);
                // Raw sum                          | %23 = add_ct(%5, %14);
                // Raw sum                          | %24 = add_ct(%6, %15);
                // Raw sum                          | %25 = add_ct(%7, %16);
                // Raw sum                          | %26 = add_ct(%8, %17);
                // Raw sum                          | %27 = add_ct(%9, %18);
                // Raw sum                          | %28 = add_ct(%10, %19);
                                                    | %29 = let_ct_block<0>();
                // Kogge chunk [0..7) / GenProp 0   | %30, %31 = pbs2<Protect, Lut2("ManyGenProp")>(%20);
                // Kogge chunk [0..7) / GenProp 1   | %32, %33 = pbs2<Protect, Lut2("ManyGenProp")>(%21);
                // Kogge chunk [0..7) / GenProp 2   | %34, %35 = pbs2<Protect, Lut2("ManyGenProp")>(%22);
                // Kogge chunk [0..7) / GenProp 3   | %36, %37 = pbs2<Protect, Lut2("ManyGenProp")>(%23);
                // Kogge chunk [0..7) / GenProp 4   | %38, %39 = pbs2<Protect, Lut2("ManyGenProp")>(%24);
                // Kogge chunk [0..7) / GenProp 5   | %40, %41 = pbs2<Protect, Lut2("ManyGenProp")>(%25);
                // Kogge chunk [0..7) / GenProp 6   | %42, %43 = pbs2<Protect, Lut2("ManyGenProp")>(%26);
                // Kogge chunk [0..7)               | %44 = pack_ct<4>(%29, %31);
                // Kogge chunk [0..7)               | %45 = pbs<Protect, Lut1("GenPropAdd")>(%44);
                // Kogge chunk [0..7)               | %46 = pack_ct<2>(%30, %29);
                // Kogge chunk [0..7)               | %47 = pbs<Protect, Lut1("ReduceCarry2")>(%46);
                // Kogge chunk [0..7)               | %48 = pack_ct<4>(%47, %33);
                // Kogge chunk [0..7)               | %49 = pbs<Protect, Lut1("GenPropAdd")>(%48);
                // Kogge chunk [0..7)               | %50 = pack_ct<4>(%32, %46);
                // Kogge chunk [0..7)               | %51 = pbs<Protect, Lut1("ReduceCarry3")>(%50);
                // Kogge chunk [0..7)               | %52 = pack_ct<4>(%51, %35);
                // Kogge chunk [0..7)               | %53 = pbs<Protect, Lut1("GenPropAdd")>(%52);
                // Kogge chunk [0..7)               | %54 = pack_ct<2>(%34, %32);
                // Kogge chunk [0..7)               | %56 = pack_ct<4>(%54, %46);
                // Kogge chunk [0..7)               | %57 = pbs<AllowBothPadding, Lut1("ReduceCarryPad")>(%56);
                // Kogge chunk [0..7)               | %58 = let_pt_block<1>();
                // Kogge chunk [0..7)               | %59 = wrapping_add_pt(%57, %58);
                // Kogge chunk [0..7)               | %60 = pack_ct<4>(%59, %37);
                // Kogge chunk [0..7)               | %61 = pbs<Protect, Lut1("GenPropAdd")>(%60);
                // Kogge chunk [0..7)               | %62 = pack_ct<2>(%36, %59);
                // Kogge chunk [0..7)               | %63 = pbs<Protect, Lut1("ReduceCarry2")>(%62);
                // Kogge chunk [0..7)               | %64 = pack_ct<4>(%63, %39);
                // Kogge chunk [0..7)               | %65 = pbs<Protect, Lut1("GenPropAdd")>(%64);
                // Kogge chunk [0..7)               | %66 = pack_ct<2>(%38, %36);
                // Kogge chunk [0..7)               | %68 = pack_ct<2>(%66, %59);
                // Kogge chunk [0..7)               | %69 = pbs<Protect, Lut1("ReduceCarry3")>(%68);
                // Kogge chunk [0..7)               | %70 = pack_ct<4>(%69, %41);
                // Kogge chunk [0..7)               | %71 = pbs<Protect, Lut1("GenPropAdd")>(%70);
                // Kogge chunk [0..7)               | %72 = pack_ct<4>(%40, %66);
                // Kogge chunk [0..7)               | %74 = pack_ct<2>(%72, %59);
                // Kogge chunk [0..7)               | %75 = pbs<AllowBothPadding, Lut1("ReduceCarryPad")>(%74);
                // Kogge chunk [0..7)               | %77 = wrapping_add_pt(%75, %58);
                // Kogge chunk [0..7)               | %78 = pack_ct<4>(%77, %43);
                // Kogge chunk [0..7)               | %79 = pbs<Protect, Lut1("GenPropAdd")>(%78);
                // Kogge chunk [0..7)               | %80 = pack_ct<2>(%42, %40);
                // Kogge chunk [0..7)               | %82 = pack_ct<4>(%80, %66);
                // Kogge chunk [0..7)               | %83 = pbs<AllowBothPadding, Lut1("ReduceCarryPad")>(%82);
                // Kogge chunk [0..7)               | %85 = wrapping_add_pt(%83, %58);
                // Kogge chunk [0..7)               | %86 = pack_ct<2>(%85, %59);
                // Kogge chunk [0..7)               | %87 = pbs<Protect, Lut1("ReduceCarry2")>(%86);
                // Kogge chunk [7..9) / GenProp 0   | %88, %89 = pbs2<Protect, Lut2("ManyGenProp")>(%27);
                // Kogge chunk [7..9) / GenProp 1   | %90, %91 = pbs2<Protect, Lut2("ManyGenProp")>(%28);
                // Kogge chunk [7..9)               | %92 = pack_ct<4>(%87, %89);
                // Kogge chunk [7..9)               | %93 = pbs<Protect, Lut1("GenPropAdd")>(%92);
                // Kogge chunk [7..9)               | %94 = pack_ct<4>(%88, %86);
                // Kogge chunk [7..9)               | %95 = pbs<Protect, Lut1("ReduceCarry3")>(%94);
                // Kogge chunk [7..9)               | %96 = pack_ct<4>(%95, %91);
                // Kogge chunk [7..9)               | %97 = pbs<Protect, Lut1("GenPropAdd")>(%96);
                // Join                             | %102 = decl_ct<18>();
                // Join                             | %113 = store_ct_block<0>(%45, %102);
                // Join                             | %114 = store_ct_block<1>(%49, %113);
                // Join                             | %115 = store_ct_block<2>(%53, %114);
                // Join                             | %116 = store_ct_block<3>(%61, %115);
                // Join                             | %117 = store_ct_block<4>(%65, %116);
                // Join                             | %118 = store_ct_block<5>(%71, %117);
                // Join                             | %119 = store_ct_block<6>(%79, %118);
                // Join                             | %120 = store_ct_block<7>(%93, %119);
                // Join                             | %121 = store_ct_block<8>(%97, %120);
                                                    | output<0>(%121);
            "#
        );
    }

    #[test]
    fn correctness_add_hillis_steele() {
        fn semantic(inp: &[IopValue]) -> Option<Vec<IopValue>> {
            let [IopValue::Ciphertext(lhs), IopValue::Ciphertext(rhs)] = inp else {
                unreachable!()
            };
            Some(vec![IopValue::Ciphertext(lhs.add(*rhs))])
        }
        for size in (2..128).step_by(2) {
            add_hillis_steele(CiphertextSpec::new(size, 2, 2)).test_random(100, semantic);
        }
    }

    #[test]
    fn correctness_add_ripple() {
        fn semantic(inp: &[IopValue]) -> Option<Vec<IopValue>> {
            let [IopValue::Ciphertext(lhs), IopValue::Ciphertext(rhs)] = inp else {
                unreachable!()
            };
            Some(vec![IopValue::Ciphertext(lhs.add(*rhs))])
        }
        for size in (2..128).step_by(2) {
            add_ripple(CiphertextSpec::new(size, 2, 2)).test_random(100, semantic);
        }
    }

    #[test]
    fn correctness_add() {
        fn semantic(inp: &[IopValue]) -> Option<Vec<IopValue>> {
            let [IopValue::Ciphertext(lhs), IopValue::Ciphertext(rhs)] = inp else {
                unreachable!()
            };
            Some(vec![IopValue::Ciphertext(lhs.add(*rhs))])
        }
        for size in (2..128).step_by(2) {
            add(CiphertextSpec::new(size, 2, 2)).test_random(100, semantic);
        }
    }

    #[test]
    fn correctness_sub() {
        fn semantic(inp: &[IopValue]) -> Option<Vec<IopValue>> {
            let [IopValue::Ciphertext(lhs), IopValue::Ciphertext(rhs)] = inp else {
                unreachable!()
            };
            Some(vec![IopValue::Ciphertext(lhs.sub(*rhs))])
        }
        for size in (2..128).step_by(2) {
            sub(CiphertextSpec::new(size, 2, 2)).test_random(100, semantic);
        }
    }

    #[test]
    fn correctness_add_kogge_stone() {
        fn semantic(inp: &[IopValue]) -> Option<Vec<IopValue>> {
            let [IopValue::Ciphertext(lhs), IopValue::Ciphertext(rhs)] = inp else {
                unreachable!()
            };
            Some(vec![IopValue::Ciphertext(lhs.add(*rhs))])
        }
        for size in (2..128).step_by(2) {
            add_kogge_stone(CiphertextSpec::new(size, 2, 2), 12).test_random(100, semantic);
        }
    }

    #[test]
    fn correctness_add_kogge_stone_par_w() {
        fn semantic(inp: &[IopValue]) -> Option<Vec<IopValue>> {
            let [IopValue::Ciphertext(lhs), IopValue::Ciphertext(rhs)] = inp else {
                unreachable!()
            };
            Some(vec![IopValue::Ciphertext(lhs.add(*rhs))])
        }
        for par_w in [1, 2, 4, 8, 10, 12] {
            let spec = CiphertextSpec::new(32, 2, 2);
            let builder = Builder::new(spec.block_spec());
            let a = builder.ciphertext_input(spec.int_size());
            let b = builder.ciphertext_input(spec.int_size());
            let a_blocks = builder.ciphertext_split(&a);
            let b_blocks = builder.ciphertext_split(&b);
            let (res, _carry_out) =
                builder.iop_add_kogge_stone_raw(a_blocks, b_blocks, None, par_w, true);
            let out = builder.ciphertext_join(res, None);
            builder.ciphertext_output(out);
            builder.test_random(100, semantic);
        }
    }

    #[test]
    fn correctness_overflow_add() {
        fn semantic(inp: &[IopValue]) -> Option<Vec<IopValue>> {
            let [IopValue::Ciphertext(lhs), IopValue::Ciphertext(rhs)] = inp else {
                unreachable!()
            };
            let (sum, flag) = lhs.overflow_add(*rhs);
            Some(vec![IopValue::Ciphertext(sum), IopValue::Ciphertext(flag)])
        }
        for size in (2..128).step_by(2) {
            overflow_add(CiphertextSpec::new(size, 2, 2)).test_random(100, semantic);
        }
    }

    #[test]
    fn correctness_overflow_sub() {
        fn semantic(inp: &[IopValue]) -> Option<Vec<IopValue>> {
            let [IopValue::Ciphertext(lhs), IopValue::Ciphertext(rhs)] = inp else {
                unreachable!()
            };
            let (diff, flag) = lhs.overflow_sub(*rhs);
            Some(vec![IopValue::Ciphertext(diff), IopValue::Ciphertext(flag)])
        }
        for size in (2..128).step_by(2) {
            overflow_sub(CiphertextSpec::new(size, 2, 2)).test_random(100, semantic);
        }
    }
}
