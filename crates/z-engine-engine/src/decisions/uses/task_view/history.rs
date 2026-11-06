//! The history a task view is planned from: the main agent's full working
//! set (without any current view), the open todos, and whether the prompt
//! cache is cold anyway, which makes a rebuild free.

use z_engine_protocol::{AgentId, Message, TodoStatus, now_ms};

use crate::decisions::context::UseContext;
use crate::decisions::uses::digest::head;
use crate::settings::models;

/// How long providers keep a cached prompt prefix (Anthropic's default
/// ephemeral cache; OpenAI's lasts at least as long).
pub(super) const CACHE_LIFETIME_MS: u64 = 5 * 60 * 1_000;
const MAX_TODOS: usize = 5;
const TODO_CHARS: usize = 120;

pub(super) struct History {
    pub messages: Vec<Message>,
    /// Open todo items of the main agent, cut short.
    pub todos: Vec<String>,
    /// A boundary by rule: idle past the cache lifetime, or summarized
    /// since the last turn ended. The cached prefix is gone either way.
    pub boundary: bool,
    /// The model's pricing has no cheaper cache reads: nothing to keep warm.
    pub uncached: bool,
    pub root: String,
}

impl History {
    /// `None` before the first turn: nothing to set aside.
    pub(super) fn read(cx: &UseContext) -> Option<Self> {
        let settings = cx.core.settings();
        let catalog = cx.core.catalog();
        let (messages, todos, idle, compacted, model) = cx.core.with_state(|state| {
            let last = state.turns.last()?;
            let messages = state
                .full_working
                .as_ref()
                .unwrap_or(&state.working)
                .clone();
            let todos: Vec<String> = state
                .todos_of(&AgentId::main())
                .iter()
                .filter(|todo| todo.status != TodoStatus::Completed)
                .take(MAX_TODOS)
                .map(|todo| head(&todo.content, TODO_CHARS))
                .collect();
            let idle = now_ms().saturating_sub(last.finished_at);
            let compacted = state
                .compactions
                .last()
                .is_some_and(|marker| marker.created_at >= last.finished_at);
            Some((messages, todos, idle, compacted, state.model.clone()))
        })?;
        let pricing = models::pricing(&settings.settings, catalog.as_deref(), &model);
        Some(Self {
            messages,
            todos,
            boundary: idle > CACHE_LIFETIME_MS || compacted,
            uncached: pricing.is_some_and(|price| price.cache_read >= price.input),
            root: cx.core.root.to_string_lossy().into_owned(),
        })
    }
}
