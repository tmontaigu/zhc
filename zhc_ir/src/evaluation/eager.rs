//! Push-based IR evaluation.
//!
//! Hosts [`EagerEvaluator`], the driver that evaluates operations as the caller hands them over,
//! and is the scheduling counterpart of [`LazyEvaluator`].

use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

use zhc_utils::iter::{CollectInSmallVec, Intermediate, MultiZip};

use crate::{
    AnnIR, AnnIRView, AsOpId, AsValId, Dialect, DialectInstructionSet, IR, OpId, OpMap, ValMap,
};

/// Push-based driver evaluating operations as the caller advances them.
///
/// Holds the evaluation state of every active operation and value of the borrowed [`IR`], and
/// advances an operation only when it is pushed and its arguments have already settled.
/// Operations must therefore be pushed in dependency order, which [`push_all`](Self::push_all)
/// does for the whole IR.
pub struct EagerEvaluator<'ir, D: Dialect, V: Evaluation>
where
    D::InstructionSet: Evaluable<V>,
    D::TypeSystem: EvaluatesTo<V>,
{
    valmap: ValMap<ValState<V>>,
    opmap: OpMap<OpState>,
    ir: &'ir IR<D>,
}

impl<'ir, D: Dialect, V: Evaluation> EagerEvaluator<'ir, D, V>
where
    D::InstructionSet: Evaluable<V>,
    D::TypeSystem: EvaluatesTo<V>,
{
    /// Consumes the evaluator into an annotated IR holding the values themselves.
    ///
    /// Strips the state wrappers, leaving every operation annotated with `()` and every value with
    /// the value it evaluated to. Use [`into_eval_ir`](Self::into_eval_ir) instead when the run
    /// may have failed.
    ///
    /// # Panics
    ///
    /// Panics if any operation or value of the IR has not settled in its evaluated state.
    pub fn into_value_ir(self) -> AnnIR<'ir, D, (), V> {
        let annotated = self.into_eval_ir();
        annotated
            .map_opann(|opref| opref.get_annotation().unwrap_evaluated())
            .map_valann(|valref| valref.get_annotation().clone().unwrap_evaluated())
    }

    /// Consumes the evaluator into an annotated IR holding the evaluation states.
    ///
    /// Preserves the complete outcome of the run, panic messages and poison markers included,
    /// making it the form to inspect after a failure.
    pub fn into_eval_ir(self) -> AnnIR<'ir, D, OpState, ValState<V>> {
        AnnIR::new(self.ir, self.opmap, self.valmap)
    }

    /// Returns a read-only annotated view of the current evaluation state.
    ///
    /// Borrows the evaluator rather than consuming it, so the states can be inspected in between
    /// pushes.
    pub fn as_view(&self) -> AnnIRView<'ir, '_, D, OpState, ValState<V>> {
        AnnIRView::new(self.ir, &self.opmap, &self.valmap)
    }

    /// Creates an evaluator over an IR, with every operation and value pending.
    ///
    /// Borrows `ir` for the evaluator's lifetime; nothing is evaluated until an operation is
    /// pushed.
    pub fn from_ir(ir: &'ir IR<D>) -> Self {
        EagerEvaluator {
            valmap: ir.filled_valmap(ValState::Pending),
            opmap: ir.filled_opmap(OpState::Pending),
            ir,
        }
    }

    /// Returns the value bound to the given value id.
    ///
    /// # Errors
    ///
    /// Returns [`EvalError::PendingValId`] if the operation producing `valid` has not been pushed
    /// yet, [`EvalError::PoisonedValId`] if that operation or one of its predecessors failed, and
    /// [`EvalError::UnknownValId`] if the evaluator holds no state for `valid`.
    ///
    /// # Panics
    ///
    /// Panics if `valid` is out of bounds for the IR or refers to an inactive value.
    pub fn get_val(&self, valid: impl AsValId) -> Result<&V, EvalError> {
        match self.valmap.get(valid) {
            None => Err(EvalError::UnknownValId),
            Some(ValState::Pending) => Err(EvalError::PendingValId),
            Some(ValState::PoisonedBy(opid)) => Err(EvalError::PoisonedValId(*opid)),
            Some(ValState::Evaluated(v)) => Ok(v),
        }
    }

    /// Returns `true` if every active value of the IR has been evaluated.
    ///
    /// Reports that no value is left pending or poisoned. Operation states are not inspected, so
    /// an operation that declares no result can have failed while this still returns `true`.
    pub fn is_ok(&self) -> bool {
        self.as_view()
            .walk_vals_linear()
            .all(|val| val.get_annotation().is_evaluated())
    }

    /// Pushes every active operation of the IR in topological order.
    ///
    /// Drives a complete run against `context`, the traversal order guaranteeing that each
    /// operation's arguments have settled before it is pushed. Failures do not interrupt the walk:
    /// a panicking operation poisons its results and the remaining operations are pushed anyway,
    /// so the resulting state records every independent failure of the IR rather than just the
    /// first one.
    ///
    /// # Panics
    ///
    /// Panics if any operation of the IR has already been pushed, and for the reasons listed on
    /// [`push_op`](Self::push_op).
    pub fn push_all(&mut self, context: &mut <D::InstructionSet as Evaluable<V>>::Context) {
        for opid in self
            .ir
            .walk_ops_topological()
            .map(|op| op.get_id())
            .intermediate()
        {
            self.push_op(context, opid);
        }
    }

    /// Pushes a single operation, evaluating it against the given context.
    ///
    /// Reads the arguments of `op` from the current state and, when all of them are evaluated,
    /// checks them against the operation's signature, calls [`eval`](Evaluable::eval) with
    /// `context`, then checks the results and binds them to the operation's return values. A panic
    /// raised by `eval` is caught: the operation settles as [`OpState::Panicked`] carrying the
    /// message and its results are poisoned by the operation's own id. If instead some argument is
    /// already poisoned, `eval` is not called and both the operation and its results inherit the
    /// id carried by the first poisoned argument in signature order.
    ///
    /// # Panics
    ///
    /// Panics if `op` does not refer to an active operation of the IR, if `op` is not pending, or
    /// if any argument of `op` is still pending — that is, if its producing operation has not been
    /// pushed yet. Also panics if an argument or a result does not inhabit the type the signature
    /// declares for it; unlike an `eval` panic, such a mismatch propagates out of the driver
    /// rather than being recorded, as it indicates a faulty [`Evaluable`] implementation.
    pub fn push_op(
        &mut self,
        context: &mut <D::InstructionSet as Evaluable<V>>::Context,
        op: impl AsOpId,
    ) {
        let ir = self.ir;
        let opid = op.op_id();

        assert!(
            self.opmap.get(opid).unwrap().is_pending(),
            "Pushed the non-pending operation {}.",
            ir.get_op(opid).format()
        );

        let arg_valids = ir.get_op(opid).get_arg_valids().iter().cloned().cosvec();
        let return_valids = ir.get_op(opid).get_return_valids().iter().cloned().cosvec();

        // We check if there are some upstream failures.
        let mut poison: Option<OpId> = None;
        for arg_valid in arg_valids.iter() {
            match self.valmap.get(arg_valid).unwrap() {
                ValState::Pending => unreachable!(),
                ValState::PoisonedBy(poison_opid) => poison = poison.or(Some(*poison_opid)),
                ValState::Evaluated(_) => {
                    // Good to go
                }
            }
        }
        if let Some(poison_opid) = poison {
            *(self.opmap.get_mut(opid).unwrap()) = OpState::PoisonedBy(poison_opid);
            for ret_valid in return_valids.iter() {
                *(self.valmap.get_mut(ret_valid).unwrap()) = ValState::PoisonedBy(poison_opid)
            }
            return;
        }

        // All predecessors are healthy. We can evaluate the current op.
        let arg_evals = arg_valids
            .iter()
            .map(|a| self.valmap.get(a).unwrap().as_evaluated().unwrap())
            .cosvec();
        let sig = ir.get_op(opid).get_instruction().get_signature();

        // Typecheck the arguments
        for (i, (arg, expected_type)) in
            (arg_evals.iter(), sig.get_args().iter()).mzip().enumerate()
        {
            if !expected_type.is_inhabited_by(arg) {
                panic!(
                    "Unexpected argument type encountered while evaluating {}. \
                     At position {i}, expected type {expected_type}, but encountered {}.",
                    ir.get_op(opid).format(),
                    D::TypeSystem::type_of(arg)
                )
            }
        }

        // Eval with panic catching
        let evaluation = catch_unwind(AssertUnwindSafe(|| {
            ir.get_op(opid).get_instruction().eval(context, arg_evals)
        }));
        match evaluation {
            Err(payload) => {
                let msg = extract_panic_message(payload);
                *(self.opmap.get_mut(opid).unwrap()) = OpState::Panicked(msg);
                for ret_valid in return_valids.iter() {
                    *(self.valmap.get_mut(ret_valid).unwrap()) = ValState::PoisonedBy(opid)
                }
            }
            Ok(ret_evals) => {
                // Typechecks the returns
                for (i, (ret, expected_type)) in (ret_evals.iter(), sig.get_returns().iter())
                    .mzip()
                    .enumerate()
                {
                    if !expected_type.is_inhabited_by(ret) {
                        panic!(
                            "Unexpected return type encountered while evaluating {}. \
                             At position {i}, expected type {expected_type}, but encountered {}.",
                            ir.get_op(opid).format(),
                            D::TypeSystem::type_of(ret)
                        )
                    }
                }
                *(self.opmap.get_mut(opid).unwrap()) = OpState::Evaluated;
                for (ret_valid, ret_eval) in (return_valids.iter(), ret_evals.into_iter()).mzip() {
                    *(self.valmap.get_mut(ret_valid).unwrap()) = ValState::Evaluated(ret_eval)
                }
            }
        }
    }
}
