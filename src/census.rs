//! census — the committed door census artifact and the committed edge
//! census artifact (the Census law in the family doctrine).
//!
//! The door census is a COMMITTED TSV artifact: one door per line, fields
//! tab-separated. The row's key columns (fields 1 and 2, for example a
//! method and a path) form the door's STABLE ID — every gap and baseline
//! entry reports a door by that ID, never by a positional row address, so
//! id-keyed reporting is reorder-stable.
//!
//! KNOWN DEVIATION (TODO: cite-grammar work — no stable-key cite form
//! yet): a door CITE in a spine (`rt:NN`) is POSITIONAL — it addresses
//! the NN-th census row. Resolution maps the address to the row's stable
//! ID only AFTER addressing, so positional `rt:` cites are
//! reorder-sensitive: a census reorder silently repoints such a cite
//! whenever the cited/uncited coverage is unchanged. A reorder that
//! changes WHICH doors are cited is loud (the ID-keyed ratchet answers
//! it as NEW/RESOLVED gaps), and an out-of-range address fails loud at
//! once (the cite cannot resolve).
//!
//! The zero-census floor: an empty census FAILS unless the profile declares
//! zero. Zero divided by zero never passes.
//!
//! A census file the profile declares but cannot be read is OPERATIONAL
//! (70) — a missing artifact is never answered as a verdict. Malformed
//! census lines are CONSTRAINT (2): the committed artifact violates what
//! the format declares.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// One census row: its 1-based row address and its stable door ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Door {
    /// The 1-based address among the census's non-empty rows (`rt:NN`).
    pub addr: usize,
    /// The stable ID: the row's key columns joined by one space.
    pub id: String,
}

/// The committed door census.
#[derive(Debug, Clone)]
pub struct Census {
    doors: Vec<Door>,
}

/// Why a census could not be judged.
#[derive(Debug)]
pub enum CensusError {
    /// The declared file cannot be read: OPERATIONAL (70).
    Unreadable(PathBuf),
    /// The committed artifact violates the format: CONSTRAINT (2).
    Malformed(Vec<String>),
}

impl Census {
    /// Read the committed artifact. An absent/unreadable file is Unreadable.
    pub fn load(path: &Path) -> Result<Self, CensusError> {
        let text = std::fs::read_to_string(path)
            .map_err(|_| CensusError::Unreadable(path.to_path_buf()))?;
        Self::parse(&text).map_err(CensusError::Malformed)
    }

    /// Parse the artifact. Door identity comes from content, so row order
    /// carries no meaning for the ID; addresses stay positional for cite
    /// resolution only.
    pub fn parse(text: &str) -> Result<Self, Vec<String>> {
        let mut doors = Vec::new();
        let mut errors = Vec::new();
        let mut addr = 0usize;
        for (i, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            addr += 1;
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 2 || cols[0].trim().is_empty() || cols[1].trim().is_empty() {
                errors.push(format!(
                    "census line {}: a door row needs at least two key columns (tab-separated)",
                    i + 1
                ));
                continue;
            }
            let id = format!("{} {}", cols[0].trim(), cols[1].trim());
            if doors.iter().any(|d: &Door| d.id == id) {
                errors.push(format!(
                    "census line {}: door id {id:?} is declared twice — ids must be unique",
                    i + 1
                ));
                continue;
            }
            doors.push(Door { addr, id });
        }
        if errors.is_empty() {
            Ok(Self { doors })
        } else {
            Err(errors)
        }
    }

    pub fn len(&self) -> usize {
        self.doors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.doors.is_empty()
    }

    /// Resolve one door cite `rt:NN` to its stable door ID. None when the
    /// address falls outside the committed census — the cite cannot resolve.
    pub fn resolve_cite(&self, cite: &str) -> Option<&Door> {
        let n: usize = cite.strip_prefix("rt:")?.parse().ok()?;
        if n >= 1 {
            self.doors.iter().find(|d| d.addr == n)
        } else {
            None
        }
    }

    /// The stable IDs of every census row, in row order.
    pub fn ids(&self) -> Vec<&str> {
        self.doors.iter().map(|d| d.id.as_str()).collect()
    }
}

/// Read the committed schema-edge census: the markdown section headed
/// "Foreign key details", where each "### child" table contributes the
/// "->parent" crossings its rows declare. Ok((solid, dashed)); the dashed
/// set resolves CLAIMS only (the allow-list source), enforcement covers
/// solid edges. Err(path) when the artifact is unreadable — OPERATIONAL.
pub fn edge_census(
    doc: &Path,
    allow_list: Option<&Path>,
) -> Result<(BTreeSet<String>, BTreeSet<String>), PathBuf> {
    let text = std::fs::read_to_string(doc).map_err(|_| doc.to_path_buf())?;
    let mut solid = BTreeSet::new();
    let mut child = String::new();
    let mut in_section = false;
    for line in text.lines() {
        if let Some(head) = line.strip_prefix("## ") {
            in_section = head.starts_with("Foreign key details");
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some(t) = line.strip_prefix("### ") {
            child = t.trim().to_string();
        } else if let Some(rest) = line.trim().strip_prefix("- ")
            && let Some((_, tail)) = rest.split_once("->")
        {
            let parent: String = tail
                .trim()
                .chars()
                .take_while(|c| c.is_alphabetic() || *c == '_')
                .collect();
            if !child.is_empty() && !parent.is_empty() {
                solid.insert(format!("{child}->{parent}"));
            }
        }
    }
    let mut dashed = BTreeSet::new();
    if let Some(list) = allow_list
        && let Ok(sh) = std::fs::read_to_string(list)
    {
        for line in sh.lines() {
            // A shell-list assignment rides on the assignment line itself
            // (NAME='a|b|c|cite'): the payload starts after the first quote.
            let line = match line.split_once('\'') {
                Some((_, rest)) if rest.contains('|') => rest.trim_end_matches('\''),
                _ => line,
            };
            let cols: Vec<&str> = line.trim().split('|').collect();
            if cols.len() == 4
                && cols[..3].iter().all(|c| {
                    !c.is_empty()
                        && c.chars()
                            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
                })
            {
                dashed.insert(format!("{}->{}", cols[0], cols[2]));
            }
        }
    }
    Ok((solid, dashed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_key_by_stable_id_not_position() {
        let c = Census::parse("GET\t/a\tok\nPOST\t/b\tok\n").unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c.resolve_cite("rt:1").unwrap().id, "GET /a");
        // The same rows in a different order keep their IDs; the addresses
        // that map to them are positional and nothing else changes.
        let r = Census::parse("POST\t/b\tok\nGET\t/a\tok\n").unwrap();
        assert_eq!(r.resolve_cite("rt:1").unwrap().id, "POST /b");
        assert_eq!(r.resolve_cite("rt:2").unwrap().id, "GET /a");
        assert_eq!(c.resolve_cite("rt:9").map(|d| d.id.as_str()), None);
        assert_eq!(c.resolve_cite("rt:0").map(|d| d.id.as_str()), None);
        assert_eq!(c.resolve_cite("nonsense").map(|d| d.id.as_str()), None);
    }

    #[test]
    fn empty_census_parses_to_zero_rows() {
        assert!(Census::parse("").unwrap().is_empty());
        assert!(Census::parse("\n \n").unwrap().is_empty());
    }

    #[test]
    fn malformed_rows_fail_closed() {
        let e = Census::parse("GET\n").unwrap_err();
        assert!(e[0].contains("two key columns"), "{e:?}");
        let e = Census::parse("GET\t/a\nGET\t/a\n").unwrap_err();
        assert!(e[0].contains("declared twice"), "{e:?}");
    }

    #[test]
    fn edge_census_reads_solid_section_and_allow_list() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("schema.md");
        std::fs::write(
            &doc,
            "# Doc\n\n## Foreign key details (PRAGMA foreign_key_list)\n\n### job\n- persona_id -> persona  (x)\n- project_id -> project\n\n## Other section\n### fake\n- x -> nope\n",
        )
        .unwrap();
        let list = dir.path().join("edges.sh");
        std::fs::write(
            &list,
            "ALLOW_LIST='job|board_ref|board|cite1'\nother lines\n  board|owner_id|persona|cite2\n",
        )
        .unwrap();
        let (solid, dashed) = edge_census(&doc, Some(&list)).unwrap();
        assert!(solid.contains("job->persona"));
        assert!(solid.contains("job->project"));
        assert!(!solid.contains("fake->nope"), "other sections must not count");
        assert!(dashed.contains("job->board"));
        assert!(dashed.contains("board->persona"));
        assert!(edge_census(&dir.path().join("absent.md"), None).is_err());
    }
}
