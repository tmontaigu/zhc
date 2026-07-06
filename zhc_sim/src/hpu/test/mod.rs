#![allow(non_snake_case)]

use super::*;
use crate::Cycle;
use crate::Simulator;

pub mod legacy;

macro_rules! test_hpu_simulation {
    ($($name: ident => $cycles: literal),+) => {
        $(
        #[test]
        #[allow(unused)]
        fn $name() {
            let mut config = HpuConfig::from(PhysicalConfig::gaussian_64b_fast());
            config.pbs_timeout = Cycle(100_000);
            let mut sim = Simulator::from_simulatable(config.freq, Hpu::new(&config, HpuId(0)), TracingLevel::None);
            let (stream, leg_lat) = legacy::$name();
            sim.dispatch(Events::UCorePushDOps(stream.collect()));
            sim.play_until_event(Events::UCoreStarved);
            // sim.dump_trace("test.json");

            // Check that there are no diff with previous execution
            // If small modification are made to the models those value must be updated
            println!("{} => {}, (legacy: {})", stringify!($name), sim.now().0, leg_lat.0);
            assert_eq!(sim.now(), Cycle($cycles));

            // Uncomment if you want to have trace dump of each operations
            // let filename = format!("/tmp/hpu_compiler/tests/hpu_{}.json", stringify!($name));
            // let path = std::path::Path::new(&filename);
            // if let Some(parent) = path.parent() {
            //     std::fs::create_dir_all(parent).expect("Issue while creating output folder");
            // }
        }
        )+
    }
}
test_hpu_simulation!(
    ADDS => 79875,
    SUBS => 88111,
    SSUB => 88123,
    MULS => 153211,
    DIVS => 2805519,
    MODS => 2752054,
    OVF_ADDS => 72214,
    OVF_SUBS => 80450,
    OVF_SSUB => 80462,
    OVF_MULS => 270777,
    SHIFTS_R => 14506,
    SHIFTS_L => 14506,
    ROTS_R => 14506,
    ROTS_L => 14506,
    ADD => 64480,
    SUB => 72213,
    MUL => 137593,
    DIV => 2651018,
    MOD => 2545098,
    OVF_ADD => 56818,
    OVF_SUB => 60452,
    OVF_MUL => 255442,
    SHIFT_R => 351360,
    SHIFT_L => 347065,
    ROT_R => 367987,
    ROT_L => 367922,
    BW_AND => 23101,
    BW_OR => 23101,
    BW_XOR => 23101,
    CMP_GT => 54959,
    CMP_GTE => 54959,
    CMP_LT => 54959,
    CMP_LTE => 54959,
    CMP_EQ => 54959,
    CMP_NEQ => 54959,
    IF_THEN_ZERO => 23065,
    IF_THEN_ELSE => 38212,
    ERC_20 => 160708,
    MEMCPY => 4288,
    ILOG2 => 271038,
    COUNT0 => 129093,
    COUNT1 => 129093,
    LEAD0 => 356219,
    LEAD1 => 368100,
    TRAIL0 => 356219,
    TRAIL1 => 358394,
    ADD_SIMD => 192420,
    ERC_20_SIMD => 891825
);
