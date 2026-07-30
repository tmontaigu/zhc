//! Configuration and derived storage sizing for the virtual machine's cryptographic backend.
//!
//! [`VmConfig`] holds the TFHE key and decomposition parameters a virtual machine instance
//! runs under, flattened into the numeric form its allocators and instruction dispatch consume
//! directly. Its methods derive the element counts needed to allocate the register file, the
//! bootstrapping key, the keyswitch key, and the lookup-table registry, letting callers size
//! backing storage before constructing a virtual machine. [`LUTS_REGISTRY_SIZE`] fixes the
//! number of lookup tables held by the registry, independent of any particular `VmConfig`.

/// Number of lookup tables held in the lookup-table registry.
pub const LUTS_REGISTRY_SIZE: usize = 76;

/// Flattened cryptographic and memory-layout parameters for one virtual machine instance.
///
/// Combines the TFHE key and decomposition parameters the virtual machine's homomorphic
/// operations run under with the register file size, and exposes derived sizing methods used
/// to allocate its ciphertext, key, and lookup-table storage.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct VmConfig {
    /// Dimension of the LWE key used for small ciphertexts.
    pub lwe_dim: usize,
    /// Polynomial size of the GLWE key backing the bootstrapping key.
    pub bsk_polynomial_size: usize,
    /// GLWE dimension of the key backing the bootstrapping key.
    pub bsk_glwe_dim: usize,
    /// Number of decomposition levels used when applying the bootstrapping key.
    pub bsk_dec_levels: usize,
    /// Base-2 logarithm of the decomposition base used when applying the bootstrapping key.
    pub bsk_dec_base_log: usize,
    /// Number of decomposition levels used when applying the keyswitch key.
    pub ksk_dec_levels: usize,
    /// Base-2 logarithm of the decomposition base used when applying the keyswitch key.
    pub ksk_dec_base_log: usize,
    /// Plaintext encoding scaling factor applied when encoding and decoding message values.
    pub delta: usize,
    /// Number of bits reserved for the carry space of an encoded message.
    pub carry_size: usize,
    /// Number of bits used to encode a message value.
    pub message_size: usize,
    /// Number of ciphertext registers available in the register file.
    pub regf_size: usize,
}

impl VmConfig {
    /// Returns the number of elements in a ciphertext encrypted under the bootstrapping key's
    /// GLWE parameters.
    ///
    /// Computed from [`bsk_glwe_dim`](Self::bsk_glwe_dim) and
    /// [`bsk_polynomial_size`](Self::bsk_polynomial_size); used to size buffers holding such
    /// ciphertexts, such as the register file.
    pub fn big_ciphertext_size(&self) -> usize {
        self.bsk_glwe_dim * self.bsk_polynomial_size + 1
    }

    /// Returns the number of elements in a ciphertext encrypted under the LWE key.
    ///
    /// Computed from [`lwe_dim`](Self::lwe_dim); used to size buffers holding such
    /// ciphertexts.
    pub fn small_ciphertext_size(&self) -> usize {
        self.lwe_dim + 1
    }

    /// Returns the number of elements required to allocate the register file.
    ///
    /// Equal to [`big_ciphertext_size`](Self::big_ciphertext_size) multiplied by
    /// [`regf_size`](Self::regf_size): the register file holds one ciphertext of that size per
    /// register.
    pub fn register_alloc_size(&self) -> usize {
        self.big_ciphertext_size() * self.regf_size
    }

    /// Returns the number of elements required to allocate the keyswitch key.
    ///
    /// Derived from [`bsk_glwe_dim`](Self::bsk_glwe_dim),
    /// [`bsk_polynomial_size`](Self::bsk_polynomial_size),
    /// [`ksk_dec_levels`](Self::ksk_dec_levels), and [`lwe_dim`](Self::lwe_dim).
    pub fn ksk_alloc_size(&self) -> usize {
        self.bsk_glwe_dim * self.bsk_polynomial_size * self.ksk_dec_levels * (self.lwe_dim + 1)
    }

    /// Returns the number of elements required to allocate the bootstrapping key.
    ///
    /// Derived from [`lwe_dim`](Self::lwe_dim), [`bsk_dec_levels`](Self::bsk_dec_levels),
    /// [`bsk_glwe_dim`](Self::bsk_glwe_dim), and
    /// [`bsk_polynomial_size`](Self::bsk_polynomial_size).
    pub fn bsk_alloc_size(&self) -> usize {
        self.lwe_dim
            * self.bsk_dec_levels
            * (self.bsk_glwe_dim + 1)
            * (self.bsk_glwe_dim + 1)
            * self.bsk_polynomial_size
            / 2
    }

    /// Returns the number of elements required to allocate the lookup-table registry.
    ///
    /// Equal to [`LUTS_REGISTRY_SIZE`] multiplied by [`lut_alloc_size`](Self::lut_alloc_size):
    /// the registry holds [`LUTS_REGISTRY_SIZE`] lookup tables of that size.
    pub fn lut_registry_alloc_size(&self) -> usize {
        LUTS_REGISTRY_SIZE * self.lut_alloc_size()
    }

    /// Returns the number of elements required to allocate a single lookup table.
    ///
    /// Derived from [`bsk_glwe_dim`](Self::bsk_glwe_dim) and
    /// [`bsk_polynomial_size`](Self::bsk_polynomial_size).
    pub fn lut_alloc_size(&self) -> usize {
        (self.bsk_glwe_dim + 1) * self.bsk_polynomial_size
    }
}
