//! Error types for fallible IR operations.
//!
//! This module defines [`Failure`], the error type returned by fallible operation construction
//! methods on [`IR`](crate::IR). Each variant represents a specific validation failure that can
//! occur when adding an operation: missing or inactive value references, type mismatches against
//! the operation signature, or arithmetic overflow in depth tracking.

use std::error::Error;
use std::fmt::Display;

use zhc_utils::small::SmallVec;

use crate::{Dialect, ValId};

/// Error returned by fallible operation construction methods on [`IR`](crate::IR).
///
/// Each variant corresponds to a validation check performed before an operation is added to the
/// IR. The error is generic over the dialect to capture type information in signature mismatches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure<D: Dialect> {
    UnknownValue(ValId),
    InactiveValue(ValId),
    SignatureMismatch {
        expected: SmallVec<D::TypeSystem>,
        actual: SmallVec<D::TypeSystem>,
    },
    DepthOverflow,
}

impl<D: Dialect> Display for Failure<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownValue(valid) => write!(f, "Unknown value: {valid}"),
            Self::InactiveValue(valid) => write!(f, "Inactive value: {valid}"),
            Self::SignatureMismatch { expected, actual } => {
                write!(
                    f,
                    "Signature error: received {actual} instead of {expected}"
                )
            }
            Self::DepthOverflow => {
                write!(
                    f,
                    "Overflow occurred while computing the depth of a new operation"
                )
            }
        }
    }
}

impl<D: Dialect> Error for Failure<D> {}
