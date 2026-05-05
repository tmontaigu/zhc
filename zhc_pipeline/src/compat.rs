use std::str::FromStr;

use zhc_builder::{
    CiphertextSpec, add, cmp_eq, cmp_gt, cmp_gte, cmp_lt, cmp_lte, cmp_neq, count_0, count_1, div,
    if_then_else, if_then_zero, ilog2, lead0, lead1, mul_lsb, rem, sub, trail0, trail1,
};
use zhc_sim::hpu::HpuConfig;

use crate::{
    regular_pipeline,
    translation_table::{DOpRepr, generate_translation_table},
};

/// Iops supported by the pipeline.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum Iop {
    CmpGt,
    CmpGte,
    CmpLt,
    CmpLte,
    CmpEq,
    CmpNeq,
    IfThenElse,
    IfThenZero,
    /// Wrapping addition of two encrypted integers.
    Add,
    /// Wrapping subtraction of two encrypted integers.
    Sub,
    /// Wrapping multiplication of two encrypted integers (LSB result).
    Mul,
    Ilog2,
    CountZeros,
    CountOnes,
    LeadingZeros,
    LeadingOnes,
    TrailingZeros,
    TrailingOnes,
    /// Unsigned division of two encrypted integers (quotient and remainder).
    Div,
    /// Unsigned remainder of two encrypted integers.
    Mod,
    // AddPt,
    // SubPt,
    // PtSub,
    // MulPt,
    // DivPt,
    // ModPt,
    // OvfAddPt,
    // OvfSubPt,
    // OvfPtSub,
    // OvfMulPt,
    // RightShiftPt,
    // LeftShiftPt,
    // RightRotPt,
    // LeftRotPt,
    // OvfAdd,
    // OvfSub,
    // OvfMul,
    // BwAnd,
    // BwOr,
    // BwXor,
    // RightShift,
    // LeftShift,
    // RightRot,
    // LeftRot,
    // Erc20,
    // AddSimd,
    // Erc20Simd,
    // MemCpy,
}

impl FromStr for Iop {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ADD" => Ok(Iop::Add),
            "SUB" => Ok(Iop::Sub),
            "MUL" => Ok(Iop::Mul),
            "CMP_GT" => Ok(Iop::CmpGt),
            "CMP_GTE" => Ok(Iop::CmpGte),
            "CMP_LT" => Ok(Iop::CmpLt),
            "CMP_LTE" => Ok(Iop::CmpLte),
            "CMP_EQ" => Ok(Iop::CmpEq),
            "CMP_NEQ" => Ok(Iop::CmpNeq),
            "IF_THEN_ZERO" => Ok(Iop::IfThenZero),
            "IF_THEN_ELSE" => Ok(Iop::IfThenElse),
            "COUNT0" => Ok(Iop::CountZeros),
            "COUNT1" => Ok(Iop::CountOnes),
            "ILOG2" => Ok(Iop::Ilog2),
            "LEAD0" => Ok(Iop::LeadingZeros),
            "LEAD1" => Ok(Iop::LeadingOnes),
            "TRAIL0" => Ok(Iop::TrailingZeros),
            "TRAIL1" => Ok(Iop::TrailingOnes),
            "DIV" => Ok(Iop::Div),
            "MOD" => Ok(Iop::Mod),
            // "ADDS" => Ok(Iop::AddPt),
            // "SUBS" => Ok(Iop::SubPt),
            // "SSUB" => Ok(Iop::PtSub),
            // "MULS" => Ok(Iop::MulPt),
            // "DIVS" => Ok(Iop::DivPt),
            // "MODS" => Ok(Iop::ModPt),
            // "OVF_ADDS" => Ok(Iop::OvfAddPt),
            // "OVF_SUBS" => Ok(Iop::OvfSubPt),
            // "OVF_SSUB" => Ok(Iop::OvfPtSub),
            // "OVF_MULS" => Ok(Iop::OvfMulPt),
            // "SHIFTS_R" => Ok(Iop::RightShiftPt),
            // "SHIFTS_L" => Ok(Iop::LeftShiftPt),
            // "ROTS_R" => Ok(Iop::RightRotPt),
            // "ROTS_L" => Ok(Iop::LeftRotPt),
            // "OVF_ADD" => Ok(Iop::OvfAdd),
            // "OVF_SUB" => Ok(Iop::OvfSub),
            // "OVF_MUL" => Ok(Iop::OvfMul),
            // "BW_AND" => Ok(Iop::BwAnd),
            // "BW_OR" => Ok(Iop::BwOr),
            // "BW_XOR" => Ok(Iop::BwXor),
            // "SHIFT_R" => Ok(Iop::RightShift),
            // "SHIFT_L" => Ok(Iop::LeftShift),
            // "ROT_R" => Ok(Iop::RightRot),
            // "ROT_L" => Ok(Iop::LeftRot),
            // "ERC_20" => Ok(Iop::Erc20),
            // "ADD_SIMD" => Ok(Iop::AddSimd),
            // "ERC_20_SIMD" => Ok(Iop::Erc20Simd),
            // "MEMCPY" => Ok(Iop::MemCpy),
            _ => Err(()),
        }
    }
}

impl Iop {
    /// Generates a translation table for the specified operation configuration.
    ///
    /// Takes the HPU hardware configuration in `hpu_config`, and an integer arithmetic
    /// configuration in `integer_config` to produce an hex stream.
    pub fn get_translation_table(
        &self,
        hpu_config: &HpuConfig,
        spec: CiphertextSpec,
    ) -> Vec<DOpRepr> {
        let ir = match self {
            Iop::CmpGt => cmp_gt(spec).optimize_ir(),
            Iop::CmpGte => cmp_gte(spec).optimize_ir(),
            Iop::CmpLt => cmp_lt(spec).optimize_ir(),
            Iop::CmpLte => cmp_lte(spec).optimize_ir(),
            Iop::CmpEq => cmp_eq(spec).optimize_ir(),
            Iop::CmpNeq => cmp_neq(spec).optimize_ir(),
            Iop::IfThenElse => if_then_else(spec).optimize_ir(),
            Iop::IfThenZero => if_then_zero(spec).optimize_ir(),
            Iop::Add => add(spec).optimize_ir(),
            Iop::Sub => sub(spec).optimize_ir(),
            Iop::Mul => mul_lsb(spec).optimize_ir(),
            Iop::Ilog2 => ilog2(spec).optimize_ir(),
            Iop::CountZeros => count_0(spec).optimize_ir(),
            Iop::CountOnes => count_1(spec).optimize_ir(),
            Iop::LeadingZeros => lead0(spec).optimize_ir(),
            Iop::LeadingOnes => lead1(spec).optimize_ir(),
            Iop::TrailingZeros => trail0(spec).optimize_ir(),
            Iop::TrailingOnes => trail1(spec).optimize_ir(),
            Iop::Div => div(spec).optimize_ir(),
            Iop::Mod => rem(spec).optimize_ir(),
            // Iop::AddPt => todo!(),
            // Iop::SubPt => todo!(),
            // Iop::PtSub => todo!(),
            // Iop::MulPt => todo!(),
            // Iop::DivPt => todo!(),
            // Iop::ModPt => todo!(),
            // Iop::OvfAddPt => todo!(),
            // Iop::OvfSubPt => todo!(),
            // Iop::OvfPtSub => todo!(),
            // Iop::OvfMulPt => todo!(),
            // Iop::RightShiftPt => todo!(),
            // Iop::LeftShiftPt => todo!(),
            // Iop::RightRotPt => todo!(),
            // Iop::LeftRotPt => todo!(),
            // Iop::OvfAdd => todo!(),
            // Iop::OvfSub => todo!(),
            // Iop::OvfMul => todo!(),
            // Iop::BwAnd => todo!(),
            // Iop::BwOr => todo!(),
            // Iop::BwXor => todo!(),
            // Iop::RightShift => todo!(),
            // Iop::LeftShift => todo!(),
            // Iop::RightRot => todo!(),
            // Iop::LeftRot => todo!(),
            // Iop::Erc20 => todo!(),
            // Iop::AddSimd => todo!(),
            // Iop::Erc20Simd => todo!(),
            // Iop::MemCpy => todo!(),
        };
        let allocated = regular_pipeline(ir, hpu_config);
        generate_translation_table(&allocated)
    }
}
