//! render — the deterministic template transformer from spines to the
//! generated doc (zero LLM in the render path): per journey an overview
//! mermaid (scenes as nodes), per-scene mermaid zooms, per-scene NARRATIVE
//! (imperative steps, what-happens, fork outcomes, proof cells) and leg
//! tables, under an overview mermaid of the journeys themselves.
//!
//! The banner cites the profile-declared paths (source dir, grammar, hand
//! preamble) — the payload carries no project paths of its own. `check`
//! compares instead of writing (freshness: STALE = 1). `generate` refuses
//! to overwrite an existing target that does not carry the GENERATED marker
//! — a hand-authored file must never be clobbered silently (70, the
//! refusal is an environmental precondition, not a verdict).

use std::path::Path;

use crate::config::Profile;
use crate::spine::{self, Leg, Spine};

/// The marker every generated doc carries; `generate` refuses targets
/// without it. The first bytes of this module's own output.
pub const GENERATED_MARKER: &str = "GENERATED — DO NOT EDIT";

/// Everything the banner renders from: profile-declared paths and notes.
pub struct Banner<'a> {
    pub source_glob: String,
    pub schema: String,
    pub preamble: Option<String>,
    pub title: String,
    pub note: Option<String>,
    pub spines: &'a [(std::path::PathBuf, Spine)],
}

impl<'a> Banner<'a> {
    pub fn from_profile(profile: &'a Profile, spines: &'a [(std::path::PathBuf, Spine)]) -> Self {
        Self {
            source_glob: format!("{}/*.yaml", profile.journeys_dir.display()),
            schema: profile.schema_display(),
            preamble: profile
                .preamble
                .as_ref()
                .map(|p| p.display().to_string()),
            title: profile
                .doc_title
                .clone()
                .unwrap_or_else(|| "Journeys".to_string()),
            note: profile.doc_note.clone(),
            spines,
        }
    }
}

/// Pure render: spines plus banner data plus hand preamble in, document out.
/// Never reads the filesystem.
pub fn render(b: &Banner<'_>, preamble: &str) -> String {
    let mut d = String::new();
    d.push_str(&format!("# {}\n\n", b.title));
    d.push_str(&format!(
        "> **{GENERATED_MARKER}.** Source: `{}`, rendered by `journeyspec generate`\n",
        b.source_glob
    ));
    d.push_str("> (deterministic template transformer — zero LLM in the render path).\n");
    d.push_str(&format!(
        "> Grammar: `{}`. Verdict truth: the spine. Proof truth: the named check.\n",
        b.schema
    ));
    match (&b.preamble, &b.note) {
        (Some(p), Some(note)) => d.push_str(&format!(
            "> Hand prose that no spine field carries (the honesty-hole register, doctrine corrections) lives in\n> `{p}` and rides below verbatim. {note}\n\n"
        )),
        (Some(p), None) => d.push_str(&format!(
            "> Hand prose that no spine field carries (the honesty-hole register, doctrine corrections) lives in\n> `{p}` and rides below verbatim.\n\n"
        )),
        (None, Some(note)) => d.push_str(&format!("> {note}\n\n")),
        (None, None) => {}
    }
    if !preamble.is_empty() {
        d.push_str(preamble);
        d.push_str("\n\n---\n\n");
    }
    d.push_str("## Overview — journeys as nodes\n\n```mermaid\nflowchart LR\n");
    d.push_str(CLASSDEFS);
    for (i, (_, s)) in b.spines.iter().enumerate() {
        let statuses: Vec<&str> = s
            .legs
            .iter()
            .flat_map(|l| {
                std::iter::once(l.status.as_str()).chain(l.forks.iter().map(|f| f.status.as_str()))
            })
            .collect();
        d.push_str(&format!(
            "  j{i}[\"{jid} · {title}\"]::{cls}\n",
            i = i,
            jid = mlabel(&s.journey),
            title = mlabel(&s.title),
            cls = rollup(&statuses)
        ));
    }
    d.push_str("```\n\n");
    for (_, s) in b.spines {
        render_journey(s, &mut d);
    }
    d.push_str("*End of generated trace. Regenerate: `journeyspec generate`.*\n");
    d
}

fn render_journey(s: &Spine, d: &mut String) {
    d.push_str(&format!(
        "## {journey} — {title}\n\n",
        journey = s.journey,
        title = s.title
    ));
    if let Some(p) = &s.provenance {
        d.push_str(&format!("**Provenance.** {}\n\n", p.trim()));
    }
    for (name, w) in &s.worlds {
        d.push_str(&format!(
            "**World `{name}`** (seed {seed}): {desc}\n\n",
            seed = w.seed,
            desc = w.description
        ));
    }
    d.push_str("### Overview\n\n```mermaid\nflowchart LR\n");
    d.push_str(CLASSDEFS);
    if s.scenes.is_empty() {
        for leg in &s.legs {
            d.push_str(&format!(
                "  l{id}[\"leg {id} · {st}\"]::{cls}\n",
                id = leg.id,
                st = leg.status,
                cls = cls(&leg.status)
            ));
        }
        for pair in s.legs.windows(2) {
            d.push_str(&format!("  l{} --> l{}\n", pair[0].id, pair[1].id));
        }
    } else {
        for (i, sc) in s.scenes.iter().enumerate() {
            let count = sc.legs.len();
            let roll = scene_rollup(s, sc);
            d.push_str(&format!(
                "  s{i}[\"{id}: {title} · {count} leg(s)\"]::{roll}\n",
                id = sc.id,
                title = mlabel(&sc.title),
                roll = roll
            ));
            if i > 0 {
                d.push_str(&format!("  s{} --> s{i}\n", i - 1));
            }
        }
    }
    d.push_str("```\n\n");
    let mut step = 0usize;
    let scenes: Vec<(&str, &str, Vec<String>)> = if s.scenes.is_empty() {
        vec![(
            "legs",
            "All legs",
            s.legs.iter().map(|l| l.id.clone()).collect(),
        )]
    } else {
        s.scenes
            .iter()
            .map(|sc| (sc.id.as_str(), sc.title.as_str(), sc.legs.clone()))
            .collect()
    };
    for (id, title, leg_ids) in scenes {
        d.push_str(&format!("### Scene `{id}` — {title}\n\n"));
        d.push_str("```mermaid\nflowchart TD\n");
        d.push_str(CLASSDEFS);
        let mut prev: Option<String> = None;
        for lid in &leg_ids {
            let leg = s.legs.iter().find(|l| &l.id == lid).expect("scene legs validated");
            d.push_str(&format!(
                "  l{lid}[\"leg {lid} · {st}\"]::{cls}\n",
                lid = lid,
                st = leg.status,
                cls = cls(&leg.status)
            ));
            if let Some(p) = &prev {
                d.push_str(&format!("  l{p} --> l{lid}\n", p = p));
            }
            for f in &leg.forks {
                d.push_str(&format!(
                    "  l{lid} -.-> f{fid}[\"fork {fid} · {st}\"]::{cls}\n",
                    lid = lid,
                    fid = f.id,
                    st = f.status,
                    cls = cls(&f.status)
                ));
            }
            prev = Some(lid.clone());
        }
        d.push_str("```\n\n");
        d.push_str("**Narrative.** Deterministic template over the spine fields — imperative steps, what-happens, fork outcomes, proof cells.\n\n");
        for lid in &leg_ids {
            let leg = s.legs.iter().find(|l| &l.id == lid).expect("scene legs validated");
            step += 1;
            d.push_str(&format!(
                "{step}. **Leg {lid}** — {st}. Given {given} ",
                lid = lid,
                st = leg.status,
                given = leg.given
            ));
            match &leg.when.ui {
                Some(ui) => d.push_str(&format!("Walk: {ui}.\n")),
                None => d.push_str("Walk: agent-side doors only — no browser step.\n"),
            }
            d.push_str("   What happens:\n");
            for t in &leg.then {
                d.push_str(&format!("   - {t}\n"));
            }
            d.push_str(&format!("   {}", proof_cells(&leg.proof)));
            if let Some(n) = &leg.note {
                d.push_str(&format!(" Note: {n}"));
            }
            d.push('\n');
            for f in &leg.forks {
                d.push_str(&format!(
                    "   **Fork {fid}** — {st}. Given {given} ",
                    fid = f.id,
                    st = f.status,
                    given = f.given
                ));
                match &f.when.ui {
                    Some(ui) => d.push_str(&format!("Walk: {ui}.\n")),
                    None => d.push_str("Walk: agent-side doors only.\n"),
                }
                d.push_str("   - Outcome:");
                for t in &f.then {
                    d.push_str(&format!(" {t}"));
                }
                d.push('\n');
                d.push_str(&format!("   {}", proof_cells(&f.proof)));
                if let Some(n) = &f.note {
                    d.push_str(&format!(" Note: {n}"));
                }
                d.push('\n');
            }
        }
        d.push('\n');
        d.push_str("| leg | status | doors | ledger | proof web | proof tauri | proof android |\n|---|---|---|---|---|---|---|\n");
        for lid in &leg_ids {
            let leg = s.legs.iter().find(|l| &l.id == lid).expect("scene legs validated");
            row(leg, d);
        }
        d.push('\n');
    }
}

fn row(leg: &Leg, d: &mut String) {
    let opt = |x: &Option<String>| x.clone().unwrap_or_else(|| "—".into());
    d.push_str(&format!(
        "| {id} | {st} | {doors} | {ledger} | {web} | {tauri} | {android} |\n",
        id = leg.id,
        st = leg.status,
        doors = if leg.when.doors.is_empty() {
            "—".into()
        } else {
            leg.when.doors.join(" ")
        },
        ledger = if leg.ledger.is_empty() {
            "—".into()
        } else {
            leg.ledger.join(" ")
        },
        web = opt(&leg.proof.web),
        tauri = opt(&leg.proof.tauri),
        android = opt(&leg.proof.android),
    ));
}

fn proof_cells(p: &spine::Proof) -> String {
    let cell = |x: &Option<String>| match x {
        Some(c) => format!("`{c}`"),
        None => "HOLE".into(),
    };
    format!(
        "Proof cells: web {} · tauri {} · android {}.",
        cell(&p.web),
        cell(&p.tauri),
        cell(&p.android)
    )
}

fn scene_rollup(s: &Spine, sc: &spine::Scene) -> String {
    let mut statuses: Vec<&str> = Vec::new();
    for lid in &sc.legs {
        if let Some(leg) = s.legs.iter().find(|l| &l.id == lid) {
            statuses.push(&leg.status);
            for f in &leg.forks {
                statuses.push(&f.status);
            }
        }
    }
    rollup(&statuses)
}

fn rollup(statuses: &[&str]) -> String {
    if statuses.contains(&"RED") {
        "red".into()
    } else if statuses.contains(&"PARTIAL") {
        "partial".into()
    } else if statuses
        .iter()
        .all(|x| *x == "BLOCKED" || *x == "SKIP" || *x == "PARKED")
    {
        "grey".into()
    } else {
        "green".into()
    }
}

const CLASSDEFS: &str = "  classDef green fill:#d9f2d9,stroke:#2c7a2c;\n  classDef red fill:#f6d3d3,stroke:#b02a2a;\n  classDef partial fill:#f6e3c8,stroke:#b07d2a;\n  classDef grey fill:#e4e4e4,stroke:#777777;\n";

fn cls(status: &str) -> &'static str {
    match status {
        "GREEN" => "green",
        "RED" => "red",
        "PARTIAL" => "partial",
        _ => "grey",
    }
}

/// Keep mermaid labels safe: drop characters that break node syntax.
fn mlabel(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '"' | '[' | ']' | '(' | ')' | '{' | '}' | '<' | '>' | '#' | '|'))
        .map(|c| if c == '\'' { '’' } else { c })
        .collect()
}

/// The renderer refuses an ungrouped long journey: the composition law
/// lives in verify (composition-census); the render path must not silently
/// flatten what the gate forbids. Ok(()) when every spine may render.
pub fn renderable(spines: &[(std::path::PathBuf, Spine)]) -> Result<(), String> {
    for (_, s) in spines {
        if s.legs.len() > spine::SCENE_GROUPING_THRESHOLD && s.scenes.is_empty() {
            return Err(format!(
                "{} has {} legs (> {}) and no scenes — group the legs first",
                s.journey,
                s.legs.len(),
                spine::SCENE_GROUPING_THRESHOLD
            ));
        }
    }
    Ok(())
}

/// The rendered doc bytes for this checkout: banner from the profile,
/// hand preamble from the declared file (absent preamble renders empty).
pub fn render_doc(root: &Path, profile: &Profile, spines: &[(std::path::PathBuf, Spine)]) -> String {
    let banner = Banner::from_profile(profile, spines);
    let preamble = profile
        .preamble
        .as_ref()
        .map(|p| std::fs::read_to_string(root.join(p)).unwrap_or_default())
        .unwrap_or_default();
    render(&banner, preamble.trim())
}

/// True when the committed doc matches a fresh render byte for byte.
/// A missing target is stale (its source declares it must exist).
pub fn is_fresh(root: &Path, profile: &Profile, spines: &[(std::path::PathBuf, Spine)]) -> bool {
    let target = root.join(profile.doc_target());
    match std::fs::read_to_string(&target) {
        Ok(current) => current == render_doc(root, profile, spines),
        Err(_) => false,
    }
}

/// True when an existing target carries the GENERATED marker — the only
/// file `generate` may overwrite. Unreadable existing files are unsafe.
pub fn target_is_generated(path: &Path) -> bool {
    if !path.exists() {
        return true;
    }
    match std::fs::read_to_string(path) {
        Ok(text) => text.contains(GENERATED_MARKER),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spine() -> Spine {
        serde_yaml::from_str(
            r#"
journey: UJ-A
title: Render check
scenes: [{ id: one, title: First scene, legs: ["1"] }]
legs:
  - id: "1"
    scene: one
    given: The tool is built.
    when: { ui: Open the page, doors: ["rt:1"] }
    then: ["The board renders."]
    ledger: []
    proof: { web: "ju-1#2", tauri: null, android: null }
    edges: []
    forks: [{ id: "1a", given: The cache is warm., when: { ui: null, doors: [] }, then: ["The page reloads fast."], ledger: [], proof: { web: null, tauri: null, android: null }, edges: [], status: GREEN }]
    status: GREEN
"#,
        )
        .unwrap()
    }

    #[test]
    fn banner_renders_declared_paths_and_marker() {
        let spines = vec![(std::path::PathBuf::from("x.yaml"), spine())];
        let profile = Profile {
            journeys_dir: std::path::PathBuf::from("data/j"),
            schema: Some(std::path::PathBuf::from("data/j/g.json")),
            doc_title: Some("18 — Journeys".into()),
            doc_note: Some("History note.".into()),
            preamble: Some(std::path::PathBuf::from("data/j/p.md")),
            ..Default::default()
        };
        let banner = Banner::from_profile(&profile, &spines);
        let doc = render(&banner, "HAND PROSE");
        assert!(doc.contains(GENERATED_MARKER));
        assert!(doc.contains("# 18 — Journeys"));
        assert!(doc.contains("`data/j/*.yaml`"));
        assert!(doc.contains("`data/j/g.json`"));
        assert!(doc.contains("`data/j/p.md`"));
        assert!(doc.contains("History note."));
        assert!(doc.contains("HAND PROSE"));
        assert!(doc.contains("```mermaid"));
        assert!(doc.contains("Fork 1a"));
        assert!(doc.contains("Regenerate: `journeyspec generate`"));
    }

    #[test]
    fn mermaid_labels_are_sanitized() {
        assert_eq!(mlabel("a<b\"c[d]"), "abcd");
        assert_eq!(mlabel("it's"), "it’s");
    }

    #[test]
    fn rollup_prefers_red_then_partial() {
        assert_eq!(rollup(&["GREEN", "RED"]), "red");
        assert_eq!(rollup(&["GREEN", "PARTIAL"]), "partial");
        assert_eq!(rollup(&["SKIP", "PARKED"]), "grey");
        assert_eq!(rollup(&["GREEN"]), "green");
    }

    #[test]
    fn ungrouped_long_journey_is_not_renderable() {
        let mut s = spine();
        s.legs = (1..=13)
            .map(|i| Leg {
                id: i.to_string(),
                scene: None,
                given: "ok ok ok".into(),
                when: spine::When { ui: None, doors: vec![] },
                then: vec!["ok ok ok".into()],
                ledger: vec![],
                proof: spine::Proof { web: None, tauri: None, android: None },
                edges: vec![],
                forks: vec![],
                world: None,
                status: "RED".into(),
                note: None,
                waits_on: vec![],
            })
            .collect();
        s.scenes.clear();
        assert!(renderable(&[(std::path::PathBuf::from("x"), s)]).is_err());
    }

    #[test]
    fn generated_marker_decides_target_safety() {
        let dir = tempfile::tempdir().unwrap();
        let mine = dir.path().join("gen.md");
        assert!(target_is_generated(&mine), "absent target is safe");
        std::fs::write(&mine, format!("# Doc\n\n> **{GENERATED_MARKER}.**\n")).unwrap();
        assert!(target_is_generated(&mine));
        let hand = dir.path().join("hand.md");
        std::fs::write(&hand, "# Hand authored\n\nprose\n").unwrap();
        assert!(!target_is_generated(&hand), "hand-authored targets are refused");
    }
}
