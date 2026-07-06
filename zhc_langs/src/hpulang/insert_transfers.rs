use zhc_ir::{IR, OpMap, ValId, ValUse};
use zhc_utils::{iter::CollectInSmallVec, svec};

use crate::hpulang::{HpuId, HpuInstructionSet, HpuLang};

/// Inserts inter-HPU [`Transfer`](HpuInstructionSet::Transfer) operations for
/// cross-partition value uses.
///
/// Given `partitions` mapping each operation of `ir` to the [`HpuId`] it is
/// placed on, scans every value use and, for each use whose defining operation
/// sits on a different HPU than the using operation, splices a `Transfer` from
/// the defining HPU to the using HPU between them and rewrites the use to read
/// the transferred value. Uses whose defining instruction is replicable (see
/// [`is_replicable`](HpuInstructionSet::is_replicable)) are left untouched,
/// since the value can instead be re-materialized on the using HPU. `ir` is
/// mutated in place.
pub fn insert_transfers(ir: &mut IR<HpuLang>, partitions: &OpMap<HpuId>) {
    struct TransferToInsert {
        valid: ValId,
        uze: ValUse,
        from: HpuId,
        to: HpuId,
    }
    let val_uses_to_transfer = ir
        .walk_vals_linear()
        .flat_map(|val| val.get_uses_iter().map(move |uze| (val.clone(), uze)))
        .filter(|(val, uze)| {
            !val.get_origin().opref.get_instruction().is_replicable()
                && partitions[val.get_origin().opref] != partitions[*uze.opref]
        })
        .map(|(val, uze)| TransferToInsert {
            valid: val.get_id(),
            uze: ValUse {
                opid: uze.opref.get_id(),
                position: uze.position,
            },
            from: partitions[val.get_origin().opref],
            to: partitions[*uze.opref],
        })
        .cosvec();

    for transfer in val_uses_to_transfer.into_iter() {
        let TransferToInsert {
            valid,
            uze,
            from,
            to,
        } = transfer;
        let (_, valids) = ir.add_op(HpuInstructionSet::Transfer { from, to }, svec![valid]);
        ir.replace_val_use_at(uze, valids[0]);
    }
}
