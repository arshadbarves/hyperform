//! Network: guarded page fetches with a short-lived cache, and web search.

mod cache;
mod client;
mod convert;
mod fetch;
mod guard;
mod search;

pub use client::{USER_AGENT, WebClient};
pub use fetch::{DEFAULT_FETCH_MAX_BYTES, FetchOptions, FetchedPage};
pub use search::{SearchBackend, SearchHit};
