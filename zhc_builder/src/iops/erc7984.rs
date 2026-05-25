use zhc_crypto::integer_semantics::CiphertextSpec;

use crate::{Ciphertext, CmpKind, builder::Builder};

/// Creates an IR for a homomorphic encrypted fund transfer (ERC-7984).
///
/// The returned [`Builder`] declares three ciphertext inputs — `from` (sender
/// balance), `to` (receiver balance), and `amount` (transfer amount) — and two
/// ciphertext outputs: the updated sender balance and the updated receiver
/// balance.
///
/// When the sender has sufficient funds (`from >= amount`), the transfer
/// proceeds: `new_from = from - amount` and `new_to = to + amount`. When funds
/// are insufficient, both balances remain unchanged.
///
/// Uses Kogge-Stone carry propagation for the arithmetic, optimised for
/// single-transfer latency. For throughput-oriented batched transfers see
/// [`erc7984_simd`].
///
/// The `spec` parameter describes the integer encoding (bit-width, message
/// bits, carry bits) and determines the number of blocks in the decomposition.
///
/// # Examples
///
/// ```rust,no_run
/// # use zhc_builder::{CiphertextSpec, erc7984};
/// # let spec = CiphertextSpec::new(16, 2, 2);
/// let builder = erc7984(spec);
/// let ir = builder.optimize_ir();
/// ```
pub fn erc7984(spec: CiphertextSpec) -> Builder {
    let builder = Builder::new(spec.block_spec());
    let src_from = builder.ciphertext_input(spec.int_size());
    let src_to = builder.ciphertext_input(spec.int_size());
    let src_amount = builder.ciphertext_input(spec.int_size());
    let (new_from, new_to) = builder.iop_erc_7984_impl(&src_from, &src_to, &src_amount, spec);
    builder.ciphertext_output(new_from);
    builder.ciphertext_output(new_to);
    builder
}

/// Number of parallel transfers in a SIMD batch.
const SIMD_N: usize = 12;

/// Creates an IR for a batched homomorphic encrypted fund transfer (ERC-7984 SIMD).
///
/// The returned [`Builder`] declares `SIMD_N` (12) independent transfer
/// triplets as inputs — `(from_0, to_0, amount_0, from_1, to_1, amount_1,
/// …)` — and `SIMD_N` output pairs `(new_from_0, new_to_0, …)`.
///
/// Each transfer is independent and uses ripple-carry arithmetic, trading
/// per-transfer latency for maximum throughput when many transfers are
/// processed in parallel. For single-transfer latency see [`erc7984`].
///
/// # Examples
///
/// ```rust,no_run
/// # use zhc_builder::{CiphertextSpec, erc7984_simd};
/// # let spec = CiphertextSpec::new(16, 2, 2);
/// let builder = erc7984_simd(spec);
/// let ir = builder.optimize_ir();
/// ```
pub fn erc7984_simd(spec: CiphertextSpec) -> Builder {
    let builder = Builder::new(spec.block_spec());

    for _ in 0..SIMD_N {
        let src_from = builder.ciphertext_input(spec.int_size());
        let src_to = builder.ciphertext_input(spec.int_size());
        let src_amount = builder.ciphertext_input(spec.int_size());
        let (new_from, new_to) = builder.iop_erc_7984_ripple(&src_from, &src_to, &src_amount);
        builder.ciphertext_output(new_from);
        builder.ciphertext_output(new_to);
    }

    builder
}

impl Builder {
    /// Computes a homomorphic encrypted fund transfer (latency-optimised).
    ///
    /// Selects the best carry-propagation strategy based on integer size:
    /// ripple carry for small integers, Hillis-Steele for medium, and
    /// Kogge-Stone for large.
    ///
    /// See [`iop_erc_7984_ripple`](Self::iop_erc_7984_ripple) for
    /// throughput-oriented variant.
    fn iop_erc_7984_impl(
        &self,
        src_from: &Ciphertext,
        src_to: &Ciphertext,
        src_amount: &Ciphertext,
        spec: CiphertextSpec,
    ) -> (Ciphertext, Ciphertext) {
        // Step 1: Check if sender has sufficient funds.
        let enough_fund = self.iop_cmp(src_from, src_amount, CmpKind::GreaterOrEqual);

        // Step 2: Compute conditional transfer amount.
        // iop_if_then_zero uses IfFalseZeroed internally:
        //   enough_fund=1 (sufficient) -> actual_amount = src_amount
        //   enough_fund=0 (insufficient) -> actual_amount = 0
        let actual_amount = self.iop_if_then_zero(src_amount, &enough_fund);

        // Step 3: Arithmetic strategy selection (matching add.rs / sub.rs).
        let par_w = match spec.int_size() {
            8..16 => 1,
            16..24 => 7,
            24..256 => 12,
            _ => 1,
        };

        // Step 4: new_to = src_to + actual_amount
        let new_to = match spec.int_size() {
            0..8 => self.iop_ripple_carry_add(src_to, &actual_amount, None).0,
            8..256 => self.iop_add_kogge_stone(src_to, &actual_amount, None, par_w),
            _ => todo!(),
        };

        // Step 5: new_from = src_from - actual_amount (two's complement)
        let actual_amount_inv = self.iop_bitwise_inv(&actual_amount);
        let one = self.block_let_ciphertext(1);
        let new_from = match spec.int_size() {
            0..8 => {
                self.iop_ripple_carry_add(src_from, &actual_amount_inv, Some(&one))
                    .0
            }
            8..256 => self.iop_add_kogge_stone(src_from, &actual_amount_inv, Some(&one), par_w),
            _ => todo!(),
        };

        (new_from, new_to)
    }

    /// Computes a homomorphic encrypted fund transfer using ripple carry.
    ///
    /// Uses sequential ripple-carry propagation for both the addition and
    /// subtraction steps. This variant has higher per-operation latency but is
    /// more area-efficient, making it suitable for SIMD batching where many
    /// independent transfers run in parallel.
    pub fn iop_erc_7984_ripple(
        &self,
        src_from: &Ciphertext,
        src_to: &Ciphertext,
        src_amount: &Ciphertext,
    ) -> (Ciphertext, Ciphertext) {
        // Step 1: Check if sender has sufficient funds.
        let enough_fund = self.iop_cmp(src_from, src_amount, CmpKind::GreaterOrEqual);

        // Step 2: Compute conditional transfer amount.
        let actual_amount = self.iop_if_then_zero(src_amount, &enough_fund);

        // Step 3: new_to = src_to + actual_amount (ripple carry)
        let (new_to, _) = self.iop_ripple_carry_add(src_to, &actual_amount, None);

        // Step 4: new_from = src_from - actual_amount (two's complement, ripple carry)
        let actual_amount_inv = self.iop_bitwise_inv(&actual_amount);
        let one = self.block_let_ciphertext(1);
        let (new_from, _) = self.iop_ripple_carry_add(src_from, &actual_amount_inv, Some(&one));

        (new_from, new_to)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use zhc_langs::ioplang::IopValue;

    fn erc7984_semantic(inp: &[IopValue]) -> Option<Vec<IopValue>> {
        let [
            IopValue::Ciphertext(from),
            IopValue::Ciphertext(to),
            IopValue::Ciphertext(amount),
        ] = inp
        else {
            unreachable!()
        };
        if from >= amount {
            Some(vec![
                IopValue::Ciphertext(from.sub(*amount)),
                IopValue::Ciphertext(to.add(*amount)),
            ])
        } else {
            Some(vec![IopValue::Ciphertext(*from), IopValue::Ciphertext(*to)])
        }
    }

    #[test]
    fn correctness_erc7984() {
        for size in (2..64).step_by(2) {
            erc7984(CiphertextSpec::new(size, 2, 2)).test_random(100, erc7984_semantic);
        }
    }

    #[test]
     fn correctness_erc7984_simd() {
        fn semantic(inp: &[IopValue]) -> Option<Vec<IopValue>> {
            inp.chunks(3).flat_map(|chunk| {
                let [IopValue::Ciphertext(from), IopValue::Ciphertext(to),
     IopValue::Ciphertext(amount)] =                chunk
                else {
                    unreachable!()
                };
                if from >= amount {
                    vec![
                        IopValue::Ciphertext(from.sub(*amount)),
                        IopValue::Ciphertext(to.add(*amount)),
                    ]
                } else {
                    vec![
                        IopValue::Ciphertext(*from),
                        IopValue::Ciphertext(*to),
                    ]
                }
            }).collect::<Vec<_>>().into()
        }
        for size in (2..64).step_by(2) {
            erc7984_simd(CiphertextSpec::new(size, 2, 2)).test_random(100, semantic);
        }
    }
}
