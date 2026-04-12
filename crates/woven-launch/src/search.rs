//! Fuzzy search scoring for launcher entries.

use crate::desktop::DesktopEntry;

/// A scored search result.
pub struct SearchResult {
    pub index: usize,
    pub score: i32,
}

/// Score all entries against `query`, return sorted by score (highest first).
/// Empty query returns all entries (alphabetical order, score 0).
pub fn fuzzy_search(entries: &[DesktopEntry], query: &str) -> Vec<SearchResult> {
    if query.is_empty() {
        return (0..entries.len())
            .map(|i| SearchResult { index: i, score: 0 })
            .collect();
    }

    let q = query.to_lowercase();
    let mut results: Vec<SearchResult> = entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let score = score_entry(e, &q);
            if score > 0 { Some(SearchResult { index: i, score }) } else { None }
        })
        .collect();

    results.sort_by(|a, b| b.score.cmp(&a.score));
    results
}

fn score_entry(entry: &DesktopEntry, query: &str) -> i32 {
    let name = entry.name.to_lowercase();
    let exec = entry.exec.to_lowercase();
    let comment = entry.comment.to_lowercase();

    let mut score = 0i32;

    // exact name match — highest
    if name == query { return 1000; }

    // name starts with query
    if name.starts_with(query) { score += 200; }

    // name contains query
    if name.contains(query) { score += 100; }

    // exec starts with or contains query
    if exec.starts_with(query) { score += 80; }
    if exec.contains(query) { score += 40; }

    // comment contains query
    if comment.contains(query) { score += 20; }

    // fuzzy char-by-char match on name
    if score == 0 {
        if let Some(s) = fuzzy_score(&name, query) {
            score += s;
        }
    }

    score
}

/// Simple fuzzy matching: each query char must appear in order in the target.
/// Score based on consecutive matches and early matches.
fn fuzzy_score(target: &str, query: &str) -> Option<i32> {
    let target: Vec<char> = target.chars().collect();
    let query: Vec<char> = query.chars().collect();

    let mut ti = 0;
    let mut score = 0i32;
    let mut prev_match = false;

    for &qc in &query {
        let mut found = false;
        while ti < target.len() {
            if target[ti] == qc {
                // bonus for consecutive matches
                if prev_match { score += 5; } else { score += 2; }
                // bonus for early matches
                if ti < 5 { score += 3; }
                prev_match = true;
                ti += 1;
                found = true;
                break;
            }
            prev_match = false;
            ti += 1;
        }
        if !found { return None; }
    }

    Some(score)
}
