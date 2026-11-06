//! Fifteen-minute in-memory cache of fetched pages (at most 100 entries).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::fetch::{FetchOptions, FetchedPage};

const TTL: Duration = Duration::from_secs(15 * 60);
const CAPACITY: usize = 100;

#[derive(Debug, Default)]
pub(super) struct PageCache {
    entries: HashMap<String, Entry>,
}

#[derive(Debug)]
struct Entry {
    page: FetchedPage,
    stored: Instant,
    /// Fetched with private-network access, so it may hold private data.
    private: bool,
    max_bytes: usize,
}

impl PageCache {
    /// A fresh entry compatible with `opts`: pages fetched with private
    /// access are only served to callers that allow it, and a truncated
    /// page only to callers asking for no more bytes.
    pub(super) fn get(&mut self, url: &str, opts: &FetchOptions) -> Option<FetchedPage> {
        self.entries.retain(|_, entry| entry.stored.elapsed() < TTL);
        let entry = self.entries.get(url)?;
        let access = !entry.private || opts.allow_private_network;
        let complete = !entry.page.truncated || opts.max_bytes <= entry.max_bytes;
        (access && complete).then(|| FetchedPage {
            from_cache: true,
            ..entry.page.clone()
        })
    }

    pub(super) fn put(&mut self, url: String, page: &FetchedPage, opts: &FetchOptions) {
        if self.entries.len() >= CAPACITY && !self.entries.contains_key(&url) {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.stored)
                .map(|(key, _)| key.clone());
            if let Some(oldest) = oldest {
                self.entries.remove(&oldest);
            }
        }
        self.entries.insert(
            url,
            Entry {
                page: page.clone(),
                stored: Instant::now(),
                private: opts.allow_private_network,
                max_bytes: opts.max_bytes,
            },
        );
    }
}
