//! Pure-Rust fallback engine: the shared walk plus the `regex` crate,
//! following ripgrep's defaults (binary files skipped, `.git` excluded,
//! ripgrep glob and type semantics). Files over 2 MiB are skipped.

use regex::Regex;
use tokio_util::sync::CancellationToken;

use super::format::{Collected, FileMatches, HitLine, preview};
use super::plan::{Plan, build_regex, glob_filter, type_filter};
use super::query::{GrepMode, GrepQuery};
use crate::HostError;
use crate::search::walk::walker;

const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_COLLECTED_BYTES: usize = 32 * 1024 * 1024;

/// Blocking: walks and reads files.
pub(super) fn search(plan: &Plan, cancel: &CancellationToken) -> Result<Collected, HostError> {
    let query = &plan.query;
    let regex = build_regex(query)?;
    let mut builder = walker(&plan.target);
    if let Some(glob) = &query.glob {
        builder.overrides(glob_filter(&plan.root, glob)?);
    }
    if let Some(name) = &query.file_type {
        builder.types(type_filter(name)?);
    }
    let mut collected = Collected::default();
    let mut budget = MAX_COLLECTED_BYTES;
    for entry in builder.build() {
        if cancel.is_cancelled() {
            return Err(HostError::Cancelled);
        }
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_some_and(|t| t.is_file())
            || entry.metadata().is_ok_and(|m| m.len() > MAX_FILE_BYTES)
        {
            continue;
        }
        let Ok(bytes) = std::fs::read(entry.path()) else {
            continue;
        };
        if bytes.contains(&0) {
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        let Some((lines, count)) = scan(&text, &regex, query) else {
            continue;
        };
        let path = plan.display(entry.path());
        let size = path.len() + lines.iter().map(|l| l.text.len() + 16).sum::<usize>();
        if size > budget {
            collected.overflow = true;
            break;
        }
        budget -= size;
        collected.files.push(FileMatches { path, lines, count });
    }
    Ok(collected)
}

/// Printed lines (content mode only) and the match count, or `None`.
fn scan(text: &str, regex: &Regex, query: &GrepQuery) -> Option<(Vec<HitLine>, usize)> {
    if !regex.is_match(text) {
        return None;
    }
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let regions = if query.multiline {
        multiline_regions(text, &lines, regex)
    } else {
        lines
            .iter()
            .enumerate()
            .filter(|(_, line)| regex.is_match(line.strip_suffix('\n').unwrap_or(line)))
            .map(|(i, _)| (i, i))
            .collect()
    };
    if regions.is_empty() {
        return None;
    }
    let count = regions.len();
    if query.mode != GrepMode::Content {
        return Some((Vec::new(), count));
    }
    Some((select_lines(&lines, &regions, query), count))
}

/// Line ranges covered by matches; ranges sharing a line are merged (so a
/// region counts once, as ripgrep's `--count --multiline` does).
fn multiline_regions(text: &str, lines: &[&str], regex: &Regex) -> Vec<(usize, usize)> {
    let mut starts = Vec::with_capacity(lines.len());
    let mut offset = 0;
    for line in lines {
        starts.push(offset);
        offset += line.len();
    }
    let line_of = |at: usize| {
        starts
            .partition_point(|&start| start <= at)
            .saturating_sub(1)
    };
    let mut regions: Vec<(usize, usize)> = Vec::new();
    for found in regex.find_iter(text) {
        if found.start() >= text.len() && (text.is_empty() || text.ends_with('\n')) {
            continue;
        }
        let first = line_of(found.start());
        let last = if found.end() > found.start() {
            line_of(found.end() - 1)
        } else {
            first
        };
        match regions.last_mut() {
            Some(previous) if first <= previous.1 => previous.1 = previous.1.max(last),
            _ => regions.push((first, last)),
        }
    }
    regions
}

fn select_lines(lines: &[&str], regions: &[(usize, usize)], query: &GrepQuery) -> Vec<HitLine> {
    let mut matched = vec![false; lines.len()];
    let mut shown = vec![false; lines.len()];
    for &(first, last) in regions {
        for flag in &mut matched[first..=last] {
            *flag = true;
        }
        let low = first.saturating_sub(query.before);
        let high = (last + query.after).min(lines.len() - 1);
        for flag in &mut shown[low..=high] {
            *flag = true;
        }
    }
    lines
        .iter()
        .enumerate()
        .filter(|(i, _)| shown[*i])
        .map(|(i, line)| {
            let body = line.strip_suffix('\n');
            HitLine {
                number: i as u64 + 1,
                text: preview(body.unwrap_or(line), body.is_some()),
                is_match: matched[i],
            }
        })
        .collect()
}
