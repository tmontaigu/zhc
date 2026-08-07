use std::collections::{HashMap, HashSet};

use rustc_hash::FxBuildHasher;

/// A hash map optimized for fast hashing performance, using `FxHasher`.
pub type FastMap<K, V> = HashMap<K, V, FxBuildHasher>;

/// A hash set optimized for fast hashing performance, using `FxHasher`.
pub type FastSet<K> = HashSet<K, FxBuildHasher>;
