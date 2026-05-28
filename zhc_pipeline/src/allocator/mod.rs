use zhc_ir::{AnnIR, IR};
use zhc_langs::{doplang::DopLang, hpulang::HpuLang};
use zhc_sim::hpu::HpuConfig;

mod allocator;
mod batch_map;
mod heap;
mod live_range;
mod register_file;
mod register_state;
mod translator;
mod value_state;

/// Allocates physical registers to values in the scheduled IR.
///
/// Takes a scheduled intermediate representation `ir` containing HPU operations
/// and the hardware configuration `config` to produce a new IR in the device
/// operation language with physical register assignments for all values.
pub fn allocate_registers(ir: &IR<HpuLang>, config: &HpuConfig) -> IR<DopLang> {
    let allocator = allocator::Allocator::init(ir, config.regf_size);
    let allocation = allocator.allocate_registers();
    let annir = AnnIR::new(ir, allocation, ir.filled_valmap(()));
    translator::translate(&annir)
}

#[cfg(test)]
mod test {
    use zhc_builder::{
        Builder, CiphertextSpec, add, bitwise_and, bitwise_or, bitwise_xor, cmp_gt, div,
        if_then_else, if_then_zero, mul,
    };
    use zhc_ir::{IR, PrintWalker};
    use zhc_langs::{doplang::DopLang, ioplang::IopLang};
    use zhc_sim::hpu::{HpuConfig, PhysicalConfig};
    use zhc_utils::assert_display_is;

    use crate::{batcher::batch, test::check_iop_dop_equivalence, translation::lower_iop_to_hpu};

    use super::allocate_registers;

    fn pipeline(ir: &IR<IopLang>) -> IR<DopLang> {
        let ir = lower_iop_to_hpu(&ir);
        let config = HpuConfig::from(PhysicalConfig::gaussian_64b());
        let batched = batch(&ir, &config);
        let allocated = allocate_registers(&batched, &config);
        allocated
    }

    #[test]
    fn test_allocate_add_ir() {
        let ir = pipeline(&add(CiphertextSpec::new(16, 2, 2)).optimize_ir());
        assert_display_is!(
            ir.format(),
            r#"
                %0 = _INIT();
                %1 = LD<R(0), TC(0, 0)>(%0);
                %2 = LD<R(1), TC(0, 1)>(%1);
                %3 = LD<R(2), TC(0, 2)>(%2);
                %4 = LD<R(3), TC(0, 3)>(%3);
                %5 = LD<R(4), TC(0, 4)>(%4);
                %6 = LD<R(5), TC(0, 5)>(%5);
                %7 = LD<R(6), TC(0, 6)>(%6);
                %8 = LD<R(7), TC(0, 7)>(%7);
                %9 = LD<R(8), TC(1, 0)>(%8);
                %10 = LD<R(9), TC(1, 1)>(%9);
                %11 = LD<R(10), TC(1, 2)>(%10);
                %12 = LD<R(11), TC(1, 3)>(%11);
                %13 = LD<R(12), TC(1, 4)>(%12);
                %14 = LD<R(13), TC(1, 5)>(%13);
                %15 = LD<R(14), TC(1, 6)>(%14);
                %16 = LD<R(15), TC(1, 7)>(%15);
                %17 = ADD<R(0), R(0), R(8)>(%16);
                %18 = ADD<R(1), R(1), R(9)>(%17);
                %19 = ADD<R(2), R(2), R(10)>(%18);
                %20 = ADD<R(3), R(3), R(11)>(%19);
                %21 = ADD<R(4), R(4), R(12)>(%20);
                %22 = ADD<R(5), R(5), R(13)>(%21);
                %23 = ADD<R(6), R(6), R(14)>(%22);
                %24 = ADD<R(7), R(7), R(15)>(%23);
                %25 = PBS<R(8), R(1), LUT(47)>(%24);
                %26 = PBS2<R(10, 2), R(0), LUT(26)>(%25);
                %27 = PBS<R(9), R(2), LUT(48)>(%26);
                %28 = PBS<R(12), R(3), LUT(49)>(%27);
                %29 = PBS<R(13), R(5), LUT(48)>(%28);
                %30 = PBS<R(14), R(4), LUT(47)>(%29);
                %31 = PBSF<R(15), R(6), LUT(49)>(%30);
                %32 = ADD<R(0), R(11), R(8)>(%31);
                %33 = ADD<R(1), R(1), R(11)>(%32);
                %34 = ADD<R(8), R(14), R(13)>(%33);
                %35 = ADD<R(9), R(0), R(9)>(%34);
                %36 = ADD<R(11), R(8), R(15)>(%35);
                %37 = ADD<R(12), R(9), R(12)>(%36);
                %38 = PBS<R(13), R(12), LUT(46)>(%37);
                %39 = PBS<R(15), R(9), LUT(45)>(%38);
                %40 = PBS<R(16), R(0), LUT(44)>(%39);
                %41 = PBS<R(17), R(1), LUT(1)>(%40);
                %42 = PBSF<R(18), R(10), LUT(1)>(%41);
                %43 = ADD<R(0), R(14), R(13)>(%42);
                %44 = ADD<R(1), R(8), R(13)>(%43);
                %45 = ADD<R(8), R(11), R(13)>(%44);
                %46 = ADD<R(4), R(4), R(13)>(%45);
                %47 = ADD<R(3), R(3), R(15)>(%46);
                %48 = ADD<R(2), R(2), R(16)>(%47);
                %49 = ST<TC(0, 1), R(17)>(%48);
                %50 = ST<TC(0, 0), R(18)>(%49);
                %51 = PBS<R(9), R(8), LUT(46)>(%50);
                %52 = PBS<R(10), R(1), LUT(45)>(%51);
                %53 = PBS<R(11), R(0), LUT(44)>(%52);
                %54 = PBS<R(12), R(2), LUT(1)>(%53);
                %55 = PBS<R(13), R(3), LUT(1)>(%54);
                %56 = PBSF<R(14), R(4), LUT(1)>(%55);
                %57 = ADD<R(0), R(7), R(9)>(%56);
                %58 = ADD<R(1), R(6), R(10)>(%57);
                %59 = ADD<R(2), R(5), R(11)>(%58);
                %60 = ST<TC(0, 2), R(12)>(%59);
                %61 = ST<TC(0, 3), R(13)>(%60);
                %62 = ST<TC(0, 4), R(14)>(%61);
                %63 = PBS<R(3), R(2), LUT(1)>(%62);
                %64 = PBS<R(4), R(1), LUT(1)>(%63);
                %65 = PBSF<R(5), R(0), LUT(1)>(%64);
                %66 = ST<TC(0, 5), R(3)>(%65);
                %67 = ST<TC(0, 6), R(4)>(%66);
                %68 = ST<TC(0, 7), R(5)>(%67);
            "#
        );
    }

    #[test]
    fn test_allocate_cmp_ir() {
        let ir = pipeline(&cmp_gt(CiphertextSpec::new(16, 2, 2)).optimize_ir());
        assert_display_is!(
            ir.format().with_walker(PrintWalker::Linear),
            r#"
                %0 = _INIT();
                %1 = LD<R(0), TC(0, 0)>(%0);
                %2 = LD<R(1), TC(0, 1)>(%1);
                %3 = LD<R(2), TC(0, 2)>(%2);
                %4 = LD<R(3), TC(0, 3)>(%3);
                %5 = LD<R(4), TC(0, 4)>(%4);
                %6 = LD<R(5), TC(0, 5)>(%5);
                %7 = LD<R(6), TC(0, 6)>(%6);
                %8 = LD<R(7), TC(0, 7)>(%7);
                %9 = LD<R(8), TC(1, 0)>(%8);
                %10 = LD<R(9), TC(1, 1)>(%9);
                %11 = LD<R(10), TC(1, 2)>(%10);
                %12 = LD<R(11), TC(1, 3)>(%11);
                %13 = LD<R(12), TC(1, 4)>(%12);
                %14 = LD<R(13), TC(1, 5)>(%13);
                %15 = LD<R(14), TC(1, 6)>(%14);
                %16 = LD<R(15), TC(1, 7)>(%15);
                %17 = MAC<R(0), R(1), R(0), PT_I(4)>(%16);
                %18 = MAC<R(1), R(3), R(2), PT_I(4)>(%17);
                %19 = MAC<R(2), R(5), R(4), PT_I(4)>(%18);
                %20 = MAC<R(3), R(7), R(6), PT_I(4)>(%19);
                %21 = MAC<R(4), R(9), R(8), PT_I(4)>(%20);
                %22 = MAC<R(5), R(11), R(10), PT_I(4)>(%21);
                %23 = MAC<R(6), R(13), R(12), PT_I(4)>(%22);
                %24 = MAC<R(7), R(15), R(14), PT_I(4)>(%23);
                %25 = PBS<R(8), R(7), LUT(0)>(%24);
                %26 = PBS<R(9), R(6), LUT(0)>(%25);
                %27 = PBS<R(10), R(5), LUT(0)>(%26);
                %28 = PBS<R(11), R(4), LUT(0)>(%27);
                %29 = PBS<R(12), R(3), LUT(0)>(%28);
                %30 = PBS<R(13), R(2), LUT(0)>(%29);
                %31 = PBS<R(14), R(1), LUT(0)>(%30);
                %32 = PBSF<R(15), R(0), LUT(0)>(%31);
                %33 = SUB<R(0), R(12), R(8)>(%32);
                %34 = SUB<R(1), R(13), R(9)>(%33);
                %35 = SUB<R(2), R(14), R(10)>(%34);
                %36 = SUB<R(3), R(15), R(11)>(%35);
                %37 = PBS<R(4), R(3), LUT(40)>(%36);
                %38 = PBS<R(5), R(2), LUT(40)>(%37);
                %39 = PBS<R(6), R(1), LUT(40)>(%38);
                %40 = PBSF<R(7), R(0), LUT(40)>(%39);
                %41 = ADDS<R(0), R(4), PT_I(1)>(%40);
                %42 = ADDS<R(1), R(5), PT_I(1)>(%41);
                %43 = ADDS<R(2), R(6), PT_I(1)>(%42);
                %44 = ADDS<R(3), R(7), PT_I(1)>(%43);
                %45 = MAC<R(0), R(1), R(0), PT_I(4)>(%44);
                %46 = MAC<R(1), R(3), R(2), PT_I(4)>(%45);
                %47 = PBS<R(2), R(1), LUT(51)>(%46);
                %48 = PBSF<R(3), R(0), LUT(51)>(%47);
                %49 = MAC<R(0), R(2), R(3), PT_I(4)>(%48);
                %50 = PBSF<R(1), R(0), LUT(27)>(%49);
                %51 = ST<TC(0, 0), R(1)>(%50);
            "#
        );
    }

    #[test]
    fn allocator_correctness() {
        let config = HpuConfig::from(PhysicalConfig::gaussian_64b());
        let check = |b: Builder| {
            let spec = *b.spec();
            let iop_ir = b.optimize_ir();
            let dop_ir = pipeline(&iop_ir);
            check_iop_dop_equivalence(&iop_ir, &dop_ir, spec, config.regf_size, 100);
        };
        for size in 2..=64 {
            let spec = CiphertextSpec::new(size, 2, 2);
            check(add(spec));
            check(bitwise_and(spec));
            check(bitwise_or(spec));
            check(bitwise_xor(spec));
            check(if_then_else(spec));
            check(if_then_zero(spec));
            check(mul(spec));
            check(div(spec));
        }
    }
}
