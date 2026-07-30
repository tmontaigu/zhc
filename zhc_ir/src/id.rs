use std::{
    fmt::Display,
    ops::{Add, Sub},
};

use serde::Serialize;
use zhc_utils::{Dumpable, StoreIndex};

use crate::{Dialect, OpRef};

/// Trait for types that can provide an [`OpId`].
///
/// Implemented by [`OpId`] itself, as well as reference types like
/// [`OpRef`](crate::OpRef) and [`AnnOpRef`](crate::AnnOpRef), enabling
/// functions to accept any of these types when only the underlying
/// identifier is needed.
pub trait AsOpId {
    fn op_id(&self) -> OpId;
}

pub trait AsOpRef {
    type Dialect: Dialect;
    fn op_ref(&self) -> OpRef<'_, Self::Dialect>;
}

/// Trait for types that can provide a [`ValId`].
///
/// Implemented by [`ValId`] itself, as well as reference types like
/// [`ValRef`](crate::ValRef) and [`AnnValRef`](crate::AnnValRef), enabling
/// functions to accept any of these types when only the underlying
/// identifier is needed.
pub trait AsValId {
    fn val_id(&self) -> ValId;
}

/// Generates a typed identifier with arithmetic operations and store indexing support.
///
/// Creates a strongly-typed wrapper around a raw numeric type that can be used
/// as an index into stores while preventing mixing of different ID types.
/// The generated type supports basic arithmetic operations and range generation.
macro_rules! impl_index {
    ($name: ident, $raw: ident, $raw_type: ident, $doc: expr) => {
        pub type $raw = $raw_type;

        #[doc = $doc]
        #[derive(
            Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, StoreIndex,
        )]
        // Spelled as the primitive rather than through `$raw`: `StoreIndex` only derives for a
        // literal unsigned type, a derive macro seeing tokens and not resolved aliases.
        pub struct $name(pub $raw_type);

        impl Add<$raw> for $name {
            type Output = $name;

            fn add(self, rhs: $raw) -> Self::Output {
                $name(self.0 + rhs)
            }
        }

        impl Sub<$raw> for $name {
            type Output = $name;

            fn sub(self, rhs: $raw) -> Self::Output {
                $name(self.0 - rhs)
            }
        }

        impl $name {
            /// Creates an iterator over a range of identifiers from `start` to `end`.
            pub fn range(start: $raw, end: $raw) -> impl DoubleEndedIterator<Item = $name> {
                (start..end).map(|a| $name(a))
            }
        }

        impl From<$name> for usize {
            fn from(value: $name) -> Self {
                <$name as StoreIndex>::as_usize(&value)
            }
        }
    };
}

impl_index!(
    OpId,
    OpIdRaw,
    u32,
    "Identifier for operations within an IR."
);
impl_index!(ValId, ValIdRaw, u32, "Identifier for values within an IR.");

impl AsOpId for OpId {
    fn op_id(&self) -> OpId {
        *self
    }
}

impl AsOpId for &OpId {
    fn op_id(&self) -> OpId {
        **self
    }
}

impl AsOpId for &mut OpId {
    fn op_id(&self) -> OpId {
        **self
    }
}

impl AsValId for ValId {
    fn val_id(&self) -> ValId {
        *self
    }
}

impl AsValId for &ValId {
    fn val_id(&self) -> ValId {
        **self
    }
}

impl AsValId for &mut ValId {
    fn val_id(&self) -> ValId {
        **self
    }
}
impl_index!(
    ValueNumber,
    ValueNumberRaw,
    u32,
    "Identifier used in value numbering for optimization passes."
);

impl Display for ValId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            // Alternate is an inactive valid
            write!(f, "%_{}", self.0)
        } else {
            write!(f, "%{}", self.0)
        }
    }
}

impl Dumpable for ValId {
    fn dump_to_string(&self) -> String {
        self.to_string()
    }
}

impl Display for OpId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(width) = f.width() {
            write!(f, "@{:0width$}", self.0, width = width)
        } else {
            write!(f, "@{}", self.0)
        }
    }
}

impl Dumpable for OpId {
    fn dump_to_string(&self) -> String {
        self.to_string()
    }
}
