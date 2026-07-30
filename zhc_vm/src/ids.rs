use zhc_utils::StoreIndex;

/// Zero-based index into worker tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, StoreIndex)]
pub struct WorkerId(pub u16);

/// Zero-based index into storage tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, StoreIndex)]
pub struct StorageId(pub u16);

#[derive(Debug, Copy, Clone, StoreIndex)]
pub struct RegId(pub u16);
