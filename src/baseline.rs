//! baseline — the append-only ratchet store the profile declares.
//!
//! One grandfathered gap per line: `gate<TAB>target<TAB>cite<TAB>date`.
//! `#` lines and blanks are ignored. Targets are STABLE KEYS (door ids,
//! ledger ids, edge keys, prose addresses) — never positional addresses,
//! so a regeneration or reorder cannot repoint an entry. A live gap that
//! matches a (gate, target) entry is BASELINED (INFO, verdict unchanged).
//! A gap that matches nothing is NEW (constraint-violation, exit 2). A
//! baseline entry that no longer matches any live gap is RESOLVED — stale
//! drift (exit 1) with a "shrink baseline" message: no baseline rot, the
//! ratchet law holds in both directions. Each entry carries a cite (the
//! audit/census that justified it) and a date.

use std::path::Path;

#[derive(Debug, Clone)]
pub struct Entry {
    pub gate: String,
    pub target: String,
    pub cite: String,
    pub date: String,
}

/// Load the store. A missing file is an empty store (all gaps are NEW);
/// a malformed line is a hard error string (fail closed, never skipped).
pub fn load(path: &Path) -> Result<Vec<Entry>, Vec<String>> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(vec![format!(
                "{}: cannot read baseline: {e}",
                path.display()
            )]);
        }
    };
    let mut out = Vec::new();
    let mut errors = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 4 || cols.iter().any(|c| c.trim().is_empty()) {
            errors.push(format!(
                "{}:{}: baseline wants gate<TAB>target<TAB>cite<TAB>date",
                path.display(),
                i + 1
            ));
            continue;
        }
        out.push(Entry {
            gate: cols[0].to_string(),
            target: cols[1].to_string(),
            cite: cols[2].to_string(),
            date: cols[3].to_string(),
        });
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
}

pub fn has<'a>(entries: &'a [Entry], gate: &str, target: &str) -> Option<&'a Entry> {
    entries
        .iter()
        .find(|e| e.gate == gate && e.target == target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_parses_and_matches_by_stable_key() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("b.tsv");
        assert!(load(&p).unwrap().is_empty(), "missing file is an empty store");
        std::fs::write(
            &p,
            "# comment\n\ndoor-census\tGET /board\t2026-01-01 audit\t2026-01-01\n",
        )
        .unwrap();
        let e = load(&p).unwrap();
        assert_eq!(e.len(), 1);
        assert!(has(&e, "door-census", "GET /board").is_some());
        assert!(has(&e, "door-census", "rt:1").is_none(), "positional keys never match");
    }

    #[test]
    fn malformed_line_is_a_hard_error() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("b.tsv");
        std::fs::write(&p, "gate\ttarget\n").unwrap();
        assert!(load(&p).unwrap_err()[0].contains("wants gate"));
    }
}
