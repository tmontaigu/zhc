use zhc_crypto::integer_semantics::CiphertextSpec;
use zhc_langs::ioplang::Lut1Def;

use crate::{
    CiphertextBlock,
    builder::{Builder, Ciphertext},
};

/// The kind of shift or rotate operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShiftRotKind {
    ShiftRight,
    ShiftLeft,
    RotateRight,
    RotateLeft,
}

/// Which bit position in the carry field to test as the swap condition.
#[derive(Debug, Clone, Copy)]
enum CondPos {
    /// Bit 0 of the carry (LSB).
    Pos0,
    /// Bit 1 of the carry.
    Pos1,
}

/// Creates an IR for logical right shift of an encrypted integer by an
/// encrypted amount.
pub fn shift_right(spec: CiphertextSpec) -> Builder {
    let builder = Builder::new(spec.block_spec());
    let src = builder.ciphertext_input(spec.int_size());
    let amount = builder.ciphertext_input(spec.int_size());
    let res = builder.iop_shiftrot(&src, &amount, ShiftRotKind::ShiftRight);
    builder.ciphertext_output(res);
    builder
}

/// Creates an IR for logical left shift of an encrypted integer by an
/// encrypted amount.
pub fn shift_left(spec: CiphertextSpec) -> Builder {
    let builder = Builder::new(spec.block_spec());
    let src = builder.ciphertext_input(spec.int_size());
    let amount = builder.ciphertext_input(spec.int_size());
    let res = builder.iop_shiftrot(&src, &amount, ShiftRotKind::ShiftLeft);
    builder.ciphertext_output(res);
    builder
}

/// Creates an IR for right rotation of an encrypted integer by an encrypted
/// amount.
pub fn rotate_right(spec: CiphertextSpec) -> Builder {
    let builder = Builder::new(spec.block_spec());
    let src = builder.ciphertext_input(spec.int_size());
    let amount = builder.ciphertext_input(spec.int_size());
    let res = builder.iop_shiftrot(&src, &amount, ShiftRotKind::RotateRight);
    builder.ciphertext_output(res);
    builder
}

/// Creates an IR for left rotation of an encrypted integer by an encrypted
/// amount.
pub fn rotate_left(spec: CiphertextSpec) -> Builder {
    let builder = Builder::new(spec.block_spec());
    let src = builder.ciphertext_input(spec.int_size());
    let amount = builder.ciphertext_input(spec.int_size());
    let res = builder.iop_shiftrot(&src, &amount, ShiftRotKind::RotateLeft);
    builder.ciphertext_output(res);
    builder
}

impl Builder {
    /// Shifts or rotates an encrypted integer by an encrypted amount.
    ///
    /// Implements a barrel shifter with three stages:
    /// 1. **Inner shift** — handles bit 0 of the amount (intra-block shift by 0 or 1 position
    ///    within each block's message bits).
    /// 2. **Merge** — combines each block's shifted message with the overflow from the neighboring
    ///    block (direction depends on shift kind).
    /// 3. **Block swap** — log₂ butterfly stages that conditionally swap whole blocks based on
    ///    higher bits of the amount.
    ///
    /// The effective shift amount is `amount mod int_size` (for power-of-two
    /// integer sizes).
    pub fn iop_shiftrot(
        &self,
        src: &Ciphertext,
        amount: &Ciphertext,
        kind: ShiftRotKind,
    ) -> Ciphertext {
        let src_blocks = self.ciphertext_split(src);
        let amount_blocks = self.ciphertext_split(amount);
        let blk_w = src_blocks.len();
        let msg_w = self.spec().message_size() as usize;

        // Stage 1: Inner shift — process bit 0 of amount.
        self.push_comment("Inner shift");
        let (shiftrot_msg, shiftrot_next): (Vec<_>, Vec<_>) = src_blocks
            .iter()
            .enumerate()
            .map(|(i, block)| {
                self.push_comment(format!("Block {i}"));
                let r = self.shiftrot_inner(kind, block, &amount_blocks[0]);
                self.pop_comment();
                r
            })
            .unzip();
        self.pop_comment();

        // Stage 2: Fuse msg and msg_next from neighboring blocks.
        self.push_comment("Merge");
        let mut merged: Vec<CiphertextBlock> = (0..blk_w)
            .map(|i| {
                let neighbor = match kind {
                    ShiftRotKind::ShiftRight => {
                        if i + 1 < blk_w {
                            Some(&shiftrot_next[i + 1])
                        } else {
                            None
                        }
                    }
                    ShiftRotKind::ShiftLeft => {
                        if i > 0 {
                            Some(&shiftrot_next[i - 1])
                        } else {
                            None
                        }
                    }
                    ShiftRotKind::RotateRight => Some(&shiftrot_next[(i + 1) % blk_w]),
                    ShiftRotKind::RotateLeft => Some(&shiftrot_next[(i + blk_w - 1) % blk_w]),
                };
                match neighbor {
                    Some(n) => self.block_add(&shiftrot_msg[i], n),
                    None => shiftrot_msg[i],
                }
            })
            .collect();
        self.pop_comment();

        // Stage 3: Block swap — butterfly stages for higher amount bits.
        // Each stage handles one bit of the shift amount. Stage `stg` tests
        // bit `stg` of the amount (spread across amount blocks, 2 bits per
        // block for (2,2) params).
        let num_stages = (2 * blk_w).ilog2() as usize;
        for stg in 1..num_stages {
            self.push_comment(format!("Swap stage {stg}"));
            let stride = 1usize << (stg - 1);
            let cond_block = &amount_blocks[stg / msg_w];
            let cond_pos = if stg % 2 == 1 {
                CondPos::Pos1
            } else {
                CondPos::Pos0
            };

            let prev = merged.clone();
            merged = (0..blk_w)
                .map(|i| {
                    let swap = match kind {
                        ShiftRotKind::ShiftRight => prev.get(i + stride),
                        ShiftRotKind::ShiftLeft => {
                            if i >= stride {
                                prev.get(i - stride)
                            } else {
                                None
                            }
                        }
                        ShiftRotKind::RotateRight => Some(&prev[(i + stride) % blk_w]),
                        ShiftRotKind::RotateLeft => Some(&prev[(i + blk_w - stride) % blk_w]),
                    };
                    self.shiftrot_block_swap(&prev[i], swap, cond_block, cond_pos)
                })
                .collect();
            self.pop_comment();
        }

        self.comment("Join").ciphertext_join(merged, None)
    }

    /// Computes the intra-block shift for a single block.
    ///
    /// Packs the block value (message) with the LSB amount block (carry) and
    /// applies the appropriate shift LUTs. Returns `(msg, msg_next)` where
    /// `msg` is the portion that stays in this block and `msg_next` is the
    /// overflow that contributes to the neighboring block.
    fn shiftrot_inner(
        &self,
        kind: ShiftRotKind,
        src: &CiphertextBlock,
        amount_lsb: &CiphertextBlock,
    ) -> (CiphertextBlock, CiphertextBlock) {
        let (lut_msg, lut_next) = match kind {
            ShiftRotKind::ShiftRight | ShiftRotKind::RotateRight => (
                Lut1Def::ShiftRightByCarryPos0Msg,
                Lut1Def::ShiftRightByCarryPos0MsgNext,
            ),
            ShiftRotKind::ShiftLeft | ShiftRotKind::RotateLeft => (
                Lut1Def::ShiftLeftByCarryPos0Msg,
                Lut1Def::ShiftLeftByCarryPos0MsgNext,
            ),
        };

        // Pack: amount_lsb in carry (high), src in message (low).
        let packed = self.block_pack(amount_lsb, src);
        let msg = self.block_lookup(&packed, lut_msg);
        let msg_next = self.block_lookup(&packed, lut_next);
        (msg, msg_next)
    }

    /// Conditionally selects between the original block and a swap block based
    /// on a condition bit.
    ///
    /// When the tested bit of `cond` is 0, returns `src_orig`. When 1, returns
    /// `src_swap` (or zero if `src_swap` is `None`).
    fn shiftrot_block_swap(
        &self,
        src_orig: &CiphertextBlock,
        src_swap: Option<&CiphertextBlock>,
        cond: &CiphertextBlock,
        cond_pos: CondPos,
    ) -> CiphertextBlock {
        let (lut_true_zeroed, lut_false_zeroed) = match cond_pos {
            CondPos::Pos0 => (Lut1Def::IfPos0TrueZeroed, Lut1Def::IfPos0FalseZeroed),
            CondPos::Pos1 => (Lut1Def::IfPos1TrueZeroed, Lut1Def::IfPos1FalseZeroed),
        };

        // Pack: cond in carry (high), value in message (low).
        let pack_orig = self.block_pack(cond, src_orig);
        if let Some(swap) = src_swap {
            let pack_swap = self.block_pack(cond, swap);
            // TrueZeroed: if cond bit = 1 → zero (suppress orig)
            // FalseZeroed: if cond bit = 0 → zero (suppress swap)
            // Sum gives: cond=0 → orig+0, cond=1 → 0+swap
            let orig_part = self.block_lookup(&pack_orig, lut_true_zeroed);
            let swap_part = self.block_lookup(&pack_swap, lut_false_zeroed);
            self.block_add(&orig_part, &swap_part)
        } else {
            // No swap source — zero the block when condition is true.
            self.block_lookup(&pack_orig, lut_true_zeroed)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use zhc_langs::ioplang::IopValue;

    #[test]
    fn correctness_shift_right() {
        fn semantic(inp: &[IopValue]) -> Option<Vec<IopValue>> {
            let [IopValue::Ciphertext(src), IopValue::Ciphertext(amount)] = inp else {
                unreachable!()
            };
            Some(vec![IopValue::Ciphertext(src.shift_right(*amount))])
        }
        for size in [4, 8, 16, 32, 64] {
            shift_right(CiphertextSpec::new(size, 2, 2)).test_random(100, semantic);
        }
    }

    #[test]
    fn correctness_shift_left() {
        fn semantic(inp: &[IopValue]) -> Option<Vec<IopValue>> {
            let [IopValue::Ciphertext(src), IopValue::Ciphertext(amount)] = inp else {
                unreachable!()
            };
            Some(vec![IopValue::Ciphertext(src.shift_left(*amount))])
        }
        for size in [4, 8, 16, 32, 64] {
            shift_left(CiphertextSpec::new(size, 2, 2)).test_random(100, semantic);
        }
    }

    #[test]
    fn correctness_rotate_right() {
        fn semantic(inp: &[IopValue]) -> Option<Vec<IopValue>> {
            let [IopValue::Ciphertext(src), IopValue::Ciphertext(amount)] = inp else {
                unreachable!()
            };
            Some(vec![IopValue::Ciphertext(src.rotate_right(*amount))])
        }
        for size in [4, 8, 16, 32, 64] {
            rotate_right(CiphertextSpec::new(size, 2, 2)).test_random(100, semantic);
        }
    }

    #[test]
    fn correctness_rotate_left() {
        fn semantic(inp: &[IopValue]) -> Option<Vec<IopValue>> {
            let [IopValue::Ciphertext(src), IopValue::Ciphertext(amount)] = inp else {
                unreachable!()
            };
            Some(vec![IopValue::Ciphertext(src.rotate_left(*amount))])
        }
        for size in [4, 8, 16, 32, 64] {
            rotate_left(CiphertextSpec::new(size, 2, 2)).test_random(100, semantic);
        }
    }
}
