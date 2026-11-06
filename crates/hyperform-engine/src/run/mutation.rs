//! Changes made by the main agent (and the children it absorbed), kept in
//! the session state so the verification badge sees them.

use super::spec::RunContext;

pub(super) fn note_mutation(ctx: &RunContext, at: u64) {
    if ctx.spec.is_main() {
        ctx.core.with_state(|state| state.mutation.touch(at));
    }
}
