use tfhe::shortint::{ClassicPBSParameters, parameters::KeySwitch32PBSParameters};
use zhc_config::vm::VmConfig;

/// Extension trait for constructing [`VmConfig`] from TFHE parameter sets.
///
/// TFHE parameter structs encode key dimensions, decomposition levels, and moduli, but
/// [`VmConfig`] needs those values unpacked into flat fields the VM can use directly.
/// These constructors handle the translation, including computing the plaintext encoding
/// delta from the message and carry bit widths.
///
/// # Examples
///
/// ```rust,no_run
/// # use zhc_config::vm::VmConfig;
/// # use tfhe::shortint::parameters::v1_6::V1_6_PARAM_MESSAGE_2_CARRY_2_KS32_PBS_TUNIFORM_2M128;
/// use zhc_vm::VmConfigExt;
///
/// let config = VmConfig::from_ks32_params(
///     V1_6_PARAM_MESSAGE_2_CARRY_2_KS32_PBS_TUNIFORM_2M128,
///     256, // register file size
/// );
/// ```
pub trait VmConfigExt {
    /// Creates a VM configuration from classic PBS parameters.
    ///
    /// The `regf_size` argument sets the total number of ciphertext registers available
    /// to execution plans. It must be a multiple of the number of NUMA memory domains on
    /// the target machine, because registers are partitioned evenly across storages.
    fn from_params(p: ClassicPBSParameters, regf_size: usize) -> Self;

    /// Creates a VM configuration from 32-bit keyswitch PBS parameters.
    ///
    /// Behaves identically to [`from_params`](Self::from_params) but accepts the
    /// `KeySwitch32PBSParameters` variant used by newer TFHE parameter sets such as
    /// `V1_6_PARAM_MESSAGE_2_CARRY_2_KS32_PBS_TUNIFORM_2M128`.
    fn from_ks32_params(p: KeySwitch32PBSParameters, regf_size: usize) -> Self;
}

impl VmConfigExt for VmConfig {
    fn from_params(p: ClassicPBSParameters, regf_size: usize) -> Self {
        let msg_bits = p.message_modulus.0.ilog2() as usize;
        let carry_bits = p.carry_modulus.0.ilog2() as usize;
        VmConfig {
            lwe_dim: p.lwe_dimension.0,
            bsk_polynomial_size: p.polynomial_size.0,
            bsk_glwe_dim: p.glwe_dimension.0,
            bsk_dec_levels: p.pbs_level.0,
            bsk_dec_base_log: p.pbs_base_log.0,
            ksk_dec_levels: p.ks_level.0,
            ksk_dec_base_log: p.ks_base_log.0,
            delta: 1 << (64 - msg_bits - carry_bits - 1),
            message_size: msg_bits,
            carry_size: carry_bits,
            regf_size,
        }
    }

    fn from_ks32_params(p: KeySwitch32PBSParameters, regf_size: usize) -> Self {
        let msg_bits = p.message_modulus.0.ilog2() as usize;
        let carry_bits = p.carry_modulus.0.ilog2() as usize;
        VmConfig {
            lwe_dim: p.lwe_dimension.0,
            bsk_polynomial_size: p.polynomial_size.0,
            bsk_glwe_dim: p.glwe_dimension.0,
            bsk_dec_levels: p.pbs_level.0,
            bsk_dec_base_log: p.pbs_base_log.0,
            ksk_dec_levels: p.ks_level.0,
            ksk_dec_base_log: p.ks_base_log.0,
            delta: 1 << (64 - msg_bits - carry_bits - 1),
            message_size: msg_bits,
            carry_size: carry_bits,
            regf_size,
        }
    }
}
