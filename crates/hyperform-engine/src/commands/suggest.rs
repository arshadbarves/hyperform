//! Close matches for an unknown command name: prefix or substring matches
//! first, then names within a small edit distance.

const MAX_SUGGESTIONS: usize = 3;

pub(crate) fn suggest<'a>(name: &str, known: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let name = name.to_lowercase();
    let mut scored: Vec<(usize, &str)> = known
        .into_iter()
        .filter_map(|candidate| {
            let lower = candidate.to_lowercase();
            let score = if lower.starts_with(&name) || name.starts_with(&lower) {
                0
            } else if lower.contains(&name) && name.len() >= 3 {
                1
            } else {
                let distance = edit_distance(&name, &lower);
                let limit = (name.chars().count() / 3).clamp(1, 3);
                if distance > limit {
                    return None;
                }
                1 + distance
            };
            Some((score, candidate))
        })
        .collect();
    scored.sort();
    scored.dedup_by(|a, b| a.1 == b.1);
    scored
        .into_iter()
        .take(MAX_SUGGESTIONS)
        .map(|(_, candidate)| candidate.to_string())
        .collect()
}

/// Levenshtein distance over characters.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut current = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let substitution = previous[j] + usize::from(ca != *cb);
            current.push(substitution.min(previous[j + 1] + 1).min(current[j] + 1));
        }
        previous = current;
    }
    previous[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typos_and_prefixes_are_suggested() {
        let known = ["review", "remember", "status", "security-review", "compact"];
        assert_eq!(suggest("reveiw", known), ["review"]);
        assert_eq!(suggest("stat", known), ["status"]);
        assert_eq!(suggest("rev", known)[0], "review");
        assert!(suggest("frobnicate", known).is_empty());
        assert_eq!(edit_distance("kitten", "sitting"), 3);
    }
}
