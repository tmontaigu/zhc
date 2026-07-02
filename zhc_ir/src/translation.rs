//! Driver-based IR-to-IR translation framework.
//!
//! Provides a callback-driven translation mechanism with configurable traversal
//! order. The caller supplies:
//! - An [`Order`] specifying how operations should be visited
//! - A *driver* closure that receives each source operation and a translator handle, responsible
//!   for emitting output-dialect operations and registering value mappings
//!
//! |       |        [`IR`]       |         [`AnnIRView`]    |
//! |-------|---------------------|--------------------------|
//! | Plain | [`translate`]       | [`translate_ann`]        |

use crate::{
    AnnIRView, AnnOpRef, Annotation, AsOpId, AsValId, Dialect, IR, OpId, OpMap, OpRef, State,
    ValId, ValMap,
};
use std::{marker::PhantomData, ops::Index};
use zhc_utils::{
    SafeAs,
    iter::{CollectInSmallVec, MultiZip},
    small::SmallVec,
};

/// Specifies the order in which operations are visited during translation.
///
/// The choice of traversal order affects which values are available when the driver is called
/// for a given operation. [`Linear`](Order::Linear) matches IR construction order,
/// [`Topological`](Order::Topological) guarantees dependencies are visited first, and
/// [`Custom`](Order::Custom) allows caller-controlled scheduling for advanced use cases
/// like batching.
pub enum Order {
    /// Visit operations in insertion order.
    Linear,
    /// Visit operations in topological order (dependencies before dependents).
    Topological,
    /// Visit operations in a caller-specified order.
    Custom(Vec<OpId>),
}

/// The source-IR operation that a translated output operation originated from.
pub struct Provenance(pub OpId);

/// Maps each operation in a [`Translation`]'s output IR back to its source operation.
///
/// Indexing with an output-IR operation ID yields the [`Provenance`] recording which source
/// operation produced it. Use [`project_opmap`](Self::project_opmap) to re-key an [`OpMap`]
/// indexed by source operations into one indexed by the corresponding output operations.
pub struct ProvenanceMap(Vec<Provenance>);

impl ProvenanceMap {
    /// Re-keys `opmap`, indexed by source operations, into one indexed by output operations.
    ///
    /// For every operation in this translation's output IR, looks up the value `opmap` stores for
    /// the source operation it was translated from and clones it into the result at that
    /// position; if `opmap` has no value there, the resulting entry is left empty. The returned
    /// map has an active slot for every output operation and no inactive slots.
    pub fn project_opmap<T: Clone>(&self, opmap: &OpMap<T>) -> OpMap<T> {
        let store = self
            .0
            .iter()
            .map(|from| State::Active(opmap.get(from.0).cloned()))
            .collect();
        OpMap {
            store,
            n_stored: self.0.len().sas(),
            n_inactive: 0,
        }
    }
}

impl<A: AsOpId> Index<A> for ProvenanceMap {
    type Output = Provenance;

    fn index(&self, index: A) -> &Self::Output {
        &self.0[index.op_id().0 as usize]
    }
}

/// The result of translating an IR: the output IR together with its operation provenance.
///
/// Pairs the [`IR<OD>`] produced by [`translate`] or [`translate_ann`] with an
/// [`ProvenanceMap`] recording, for every operation in `output`, which source operation it was
/// translated from.
pub struct Translation<OD: Dialect> {
    /// The translated output IR.
    pub output: IR<OD>,
    /// Maps each operation in `output` back to the source operation it was translated from.
    pub provenance_map: ProvenanceMap,
}

/// Mutable translation state for dialect-to-dialect IR translation.
///
/// Passed to the driver callback by [`translate`] and [`translate_ann`]. The
/// driver uses this handle to look up already-translated values, emit
/// operations in the output dialect, and register value correspondences.
pub struct Translator<ID: Dialect, OD: Dialect> {
    output: IR<OD>,
    valmap: ValMap<ValId>,
    provenance_map: ProvenanceMap,
    current: Option<OpId>,
    phantom: PhantomData<ID>,
}

impl<ID: Dialect, OD: Dialect> Translator<ID, OD> {
    /// Returns the output-dialect [`ValId`] corresponding to `old`.
    ///
    /// # Panics
    ///
    /// Panics if no translation has been registered for `old`.
    pub fn translate_val(&self, old: impl AsValId) -> ValId {
        match self.valmap.get(old.val_id()) {
            Some(val) => val.clone(),
            None => panic!("Failed to translate val {}", old.val_id()),
        }
    }

    /// Emits an operation in the output [`IR`] and returns its newly created return values.
    ///
    /// The `args` must be output-dialect [`ValId`]s obtained from prior [`add_op`](Self::add_op)
    /// or [`translate_val`](Self::translate_val) calls. The number of returned values is
    /// determined by `instr`'s signature.
    pub fn add_op(&mut self, instr: OD::InstructionSet, args: SmallVec<ValId>) -> SmallVec<ValId> {
        let (_, valids) = self.output.add_op(instr, args);
        self.provenance_map
            .0
            .push(Provenance(self.current.unwrap()));
        valids
    }

    /// Returns whether a translation has been registered for `old`.
    pub fn has_translation(&self, old: impl AsValId) -> bool {
        self.valmap.contains_key(old)
    }

    /// Records a mapping from source value `old` to output value `new`.
    ///
    /// # Panics
    ///
    /// Panics if a translation has already been registered for `old`.
    pub fn register_translation(&mut self, old: impl AsValId, new: impl AsValId) {
        let old = old.val_id();
        let new = new.val_id();
        assert!(
            self.valmap.insert(old, new).is_none(),
            "Tried to register a translation twice for {old}"
        );
    }

    /// Performs a one-to-one operation translation.
    ///
    /// Translates every argument of `op` via [`translate_val`](Self::translate_val),
    /// emits a single output operation with instruction `instr` and those
    /// translated arguments, then registers the return-value correspondences.
    ///
    /// # Panics
    ///
    /// Panics if any argument lacks a registered translation, if the return
    /// arity differs, or if any return value already has a translation.
    pub fn direct_translation<'a>(&mut self, op: &OpRef<'a, ID>, instr: OD::InstructionSet) {
        let new_args = op
            .get_arg_valids()
            .iter()
            .map(|v| self.translate_val(*v))
            .cosvec();
        let new_rets = self.add_op(instr, new_args);
        assert_eq!(new_rets.len(), op.get_return_arity());
        (new_rets.into_iter(), op.get_return_valids().iter())
            .mzip()
            .for_each(|(new, old)| self.register_translation(*old, new));
    }

    fn into_translation(self) -> Translation<OD> {
        Translation {
            output: self.output,
            provenance_map: self.provenance_map,
        }
    }
}

/// Translates an [`IR<ID>`] into an [`IR<OD>`] by visiting operations in the specified order.
///
/// The `driver` is invoked once per operation. It receives an [`OpRef`] into the source IR and a
/// mutable [`Translator`] handle, and must emit corresponding output operations and register all
/// value translations. Returns a [`Translation`] pairing the output IR with an
/// [`ProvenanceMap`] tracing each output operation back to the source operation that produced
/// it.
pub fn translate<'a, ID: Dialect, OD: Dialect>(
    ir: &'a IR<ID>,
    order: Order,
    driver: impl Fn(OpRef<'a, ID>, &mut Translator<ID, OD>),
) -> Translation<OD> {
    let output = IR::empty();
    let valmap = ir.empty_valmap();
    let op_provenance_map = ProvenanceMap(Vec::new());
    let current = None;
    let mut translator = Translator {
        output,
        valmap,
        provenance_map: op_provenance_map,
        current,
        phantom: PhantomData,
    };
    match order {
        Order::Linear => {
            for op in ir.walk_ops_linear() {
                translator.current = Some(op.get_id());
                driver(op, &mut translator);
            }
        }
        Order::Topological => {
            for op in ir.walk_ops_topological() {
                translator.current = Some(op.get_id());
                driver(op, &mut translator);
            }
        }
        Order::Custom(ids) => {
            for op in ir.walk_ops_with(ids.into_iter()) {
                translator.current = Some(op.get_id());
                driver(op, &mut translator);
            }
        }
    }
    translator.into_translation()
}

/// Translates an [`AnnIRView`] into an [`IR<OD>`] by visiting operations in the specified order.
///
/// Annotation-aware variant of [`translate`]. The `driver` receives [`AnnOpRef`]s carrying
/// per-operation and per-value annotations. Returns a [`Translation`] pairing the output IR with
/// an [`ProvenanceMap`] tracing each output operation back to the source operation that
/// produced it.
pub fn translate_ann<'a, 'b, ID: Dialect, OpAnn: Annotation, ValAnn: Annotation, OD: Dialect>(
    ir: AnnIRView<'a, 'b, ID, OpAnn, ValAnn>,
    order: Order,
    driver: impl Fn(AnnOpRef<'a, 'b, ID, OpAnn, ValAnn>, &mut Translator<ID, OD>),
) -> Translation<OD> {
    let output = IR::empty();
    let valmap = ir.empty_valmap();
    let op_provenance_map = ProvenanceMap(Vec::new());
    let current = None;
    let mut translator = Translator {
        output,
        valmap,
        provenance_map: op_provenance_map,
        current,
        phantom: PhantomData,
    };
    match order {
        Order::Linear => {
            for op in ir.walk_ops_linear() {
                translator.current = Some(op.get_id());
                driver(op, &mut translator);
            }
        }
        Order::Topological => {
            for op in ir.walk_ops_topological() {
                translator.current = Some(op.get_id());
                driver(op, &mut translator);
            }
        }
        Order::Custom(ids) => {
            for op in ir.walk_ops_with(ids.into_iter()) {
                translator.current = Some(op.get_id());
                driver(op, &mut translator);
            }
        }
    }
    translator.into_translation()
}
