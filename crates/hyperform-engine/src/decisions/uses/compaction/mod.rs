//! Relevance-aware compaction (`decisions_compaction`): which old tool
//! results are still needed when context pressure clears some, and the
//! rereads that show a clear was wrong.

mod advise;
mod memory;
mod reread;
#[cfg(test)]
mod tests;

pub(crate) use advise::COMPACTION;
pub(crate) use memory::CompactionMemory;
