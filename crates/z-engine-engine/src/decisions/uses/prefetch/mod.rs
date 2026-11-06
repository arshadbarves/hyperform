//! File prefetch (`decisions_prefetch`): when a task starts, files the
//! request will very likely need are attached to the opening message, the
//! way user attachments are, so the agent spends fewer tool rounds finding
//! them. Candidates come from the request, the repository map and (on a
//! chat's first turn) the working tree; the model says yes or no per
//! candidate; the `[decisions.prefetch]` budget caps files and tokens. The
//! files are appended as reminders, so the cached prompt prefix is
//! untouched. At turn end a trace record gives the tool rounds before the
//! first edit, the measure this feature should lower.

mod attach;
mod candidates;
mod metrics;
mod pick;
#[cfg(test)]
mod tests;

pub(crate) use pick::PREFETCH;
