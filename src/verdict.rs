//! verdict — the gate set over the spines and the committed artifacts.
//!
//! Every gate is named, every gap carries the stable key of its target
//! (door id, ledger id, edge key, prose address), and the ratchet baseline
//! keys entries by the same stable keys (baseline.rs). The exit CLASS is
//! the family taxonomy, not a shared "gate red":
//!
//! - a live gap that matches no baseline entry is a broken declared
//!   constraint: CONSTRAINT (2);
//! - a baseline entry that matches nothing live is a stale ratchet
//!   artifact — "shrink baseline": STALE (1);
//! - a declared artifact that cannot be read, or a source a spine needs
//!   that the profile does not declare, is OPERATIONAL (70) — the verdict
//!   did not run, which is never reported as a verdict;
//! - the zero-census floor is a hard CONSTRAINT, never ratchetable.
//!
//! Gates:
//!   cite-resolution     — door cites resolve to census rows (by stable id),
//!                         ledger cites to the ledger doc, proof cites to
//!                         the declared proof sources
//!   verdict-freshness   — a GREEN leg needs a named check that exists
//!   door-census         — every census door is cited by ≥1 leg (committed
//!                         artifact; the gate never execs a census command)
//!   ledger-homing       — every ledger id is cited or carries the
//!                         pile/parked marker on a doc line
//!   composition-census  — journeys over the threshold group legs in scenes
//!   forks-budget        — the labelled proposed fork budget per leg
//!   edge-cites          — edge claims resolve against the committed edge
//!                         census, and every solid edge is cited by ≥1 leg
//!   asd-lint            — deterministic ASD prose lint over when.ui/given/then
//!   stale-open          — every non-GREEN leg/fork declares waits_on; a
//!                         mile unblock that now resolves fails the wait
//!                         open (unblock-resolved); a PARTIAL whose own
//!                         proof.web check exists while it waits on no
//!                         landable mile is stale (check-exists). Mile
//!                         resolution is "the named check exists in the
//!                         declared proof source" — the same predicate as
//!                         verdict-freshness; the gate never claims the
//!                         mile passed.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::census::{self, Census, CensusError};
use crate::config::Profile;
use crate::spine::{self, Spine};
use crate::{baseline, exit};

/// Deterministic ASD-approved imperative verbs for the first word of when.ui
/// (approved forms curated to the verbs the spine plane uses; the first word
/// is lowercased and stripped of leading punctuation/backticks before
/// matching). A miss is an asd-lint gap, never a silent pass.
pub const ASD_VERBS: [&str; 58] = [
    "add", "birth", "browse", "check", "choose", "click", "close", "create", "delete", "discard",
    "edit", "enter", "fill", "fold", "follow", "forget", "grant", "hide", "hire", "hover", "load",
    "narrow", "observe", "open", "paste", "pick", "post", "press", "probe", "read", "refresh",
    "reload", "remove", "reopen", "re-shoot", "rest", "review", "run", "say", "scroll", "search",
    "select", "send", "set", "settle", "show", "stop", "submit", "tail", "toggle", "type",
    "verify", "visit", "wait", "wake", "walk", "watch", "write",
];

/// Banned words (ASD: one word, one meaning; these add nothing). Token-boundary,
/// case-insensitive match.
pub const ASD_BANNED: [&str; 10] = [
    "utilize",
    "merely",
    "easily",
    "just",
    "simply",
    "very",
    "quite",
    "basically",
    "actually",
    "literally",
];

/// Longest allowed sentence in spine prose (words).
pub const ASD_MAX_SENTENCE_WORDS: usize = 25;

/// One gate's counts for the report table.
#[derive(Debug, Clone)]
pub struct GateRow {
    pub name: &'static str,
    pub pass: usize,
    pub baselined: usize,
    pub fail: usize,
    /// The denominator when the gate counts over a census (0: no census).
    pub of: usize,
}

/// The full verify outcome; cli.rs renders it as lines or JSON.
#[derive(Debug)]
pub struct Outcome {
    pub exit_code: i32,
    pub gates: Vec<GateRow>,
    pub gaps: Vec<Value>,
    pub resolved: Vec<Value>,
    pub info: Vec<Value>,
    /// Gates skipped because the profile declares no source for them.
    pub skipped: Vec<&'static str>,
    /// True when the zero-census floor failed (hard, never ratchetable).
    pub zero_floor: bool,
    /// Prose strings checked by the lint (reported on the asd-lint row).
    pub prose_checked: usize,
    /// The proposed fork budget carried by the forks-budget row label.
    pub fork_budget: usize,
}

/// Why the verdict could not run at all — the caller prints and exits.
pub struct Abort {
    pub exit_code: i32,
    pub messages: Vec<String>,
}

fn operational(messages: impl IntoIterator<Item = String>) -> Abort {
    Abort {
        exit_code: exit::OPERATIONAL,
        messages: messages.into_iter().collect(),
    }
}

/// Run the full verify gate set. Ok(Outcome) when the verdicts ran;
/// Err(Abort) when a precondition was missing or an artifact malformed.
pub fn run(root: &Path, profile: &Profile, spines: &[(PathBuf, Spine)]) -> Result<Outcome, Abort> {
    let cites_doors = spines.iter().any(|(_, s)| {
        spine::walk(s)
            .iter()
            .any(|(_, _, _, _, doors, _)| !doors.is_empty())
    });
    let cites_ledgers = spines
        .iter()
        .any(|(_, s)| spine::walk(s).iter().any(|(_, _, _, _, _, l)| !l.is_empty()));

    // Precondition pass: every declared artifact the verdict reads must be
    // readable NOW. A missing file is class 70, never a silent zero.
    let census_path = match profile.census.as_ref().map(|p| root.join(p)) {
        None => {
            if cites_doors {
                return Err(operational([
                    "profile declares no census, but spines cite doors".to_string(),
                ]));
            }
            None
        }
        Some(p) if p.is_file() => Some(p),
        Some(p) => {
            return Err(operational([format!(
                "{}: declared door census is not readable",
                p.display()
            )]));
        }
    };
    let ledger_path = match profile.ledger.as_ref().map(|p| root.join(p)) {
        None => {
            if cites_ledgers {
                return Err(operational([
                    "profile declares no ledger, but spines cite ledger rows".to_string(),
                ]));
            }
            None
        }
        Some(p) if p.is_file() => Some(p),
        Some(p) => {
            return Err(operational([format!(
                "{}: declared ledger is not readable",
                p.display()
            )]));
        }
    };

    let censored = match &census_path {
        None => None,
        Some(p) => match Census::load(p) {
            Ok(c) => Some(c),
            Err(CensusError::Unreadable(p)) => {
                return Err(operational([format!(
                    "{}: declared door census is not readable",
                    p.display()
                )]));
            }
            Err(CensusError::Malformed(errors)) => {
                return Err(Abort {
                    exit_code: exit::CONSTRAINT,
                    messages: errors,
                });
            }
        },
    };
    let zero_floor = censored.as_ref().is_some_and(|c| c.is_empty())
        && profile.census.is_some()
        && !profile.census_may_be_zero;

    let edge_path = profile.edge_census.as_ref().map(|p| root.join(p));
    if let Some(p) = &edge_path
        && !p.is_file()
    {
        return Err(Abort {
            exit_code: exit::OPERATIONAL,
            messages: vec![format!(
                "{}: declared edge census is not readable",
                p.display()
            )],
        });
    }

    let mut gaps: Vec<Value> = Vec::new();
    let mut info: Vec<Value> = Vec::new();
    let mut cite_pass = 0usize;
    let mut verdict_pass = 0usize;
    let mut stale_units: BTreeSet<String> = BTreeSet::new();
    let mut cited_door_ids: BTreeSet<String> = BTreeSet::new();
    let mut cited_ledgers: BTreeSet<String> = BTreeSet::new();
    let mut cited_edges: BTreeSet<String> = BTreeSet::new();
    let mut proof_specs: BTreeMap<String, Option<String>> = BTreeMap::new();

    let ledger_text = match &ledger_path {
        Some(p) => Some(
            std::fs::read_to_string(p)
                .map(|t| (p.clone(), t))
                .map_err(|e| Abort {
                    exit_code: exit::OPERATIONAL,
                    messages: vec![format!("{}: cannot read ledger: {e}", p.display())],
                })?,
        ),
        None => None,
    };

    let edges = match &edge_path {
        None => None,
        Some(doc) => Some(
            census::edge_census(doc, profile.edge_allow_list.as_ref().map(|p| root.join(p)).as_deref())
                .map_err(|p| Abort {
                    exit_code: exit::OPERATIONAL,
                    messages: vec![format!(
                        "{}: declared edge census is not readable",
                        p.display()
                    )],
                })?,
        ),
    };

    for (path, s) in spines {
        let jid = s.journey.clone();
        let _ = path;
        for (leg_id, fork_id, status, proof, doors, ledgers) in spine::walk(s) {
            let who = match &fork_id {
                Some(f) => format!("{jid}/{leg_id}#{f}"),
                None => format!("{jid}/{leg_id}"),
            };
            for d in &doors {
                match censored.as_ref().and_then(|c| c.resolve_cite(d)) {
                    Some(door) => {
                        cited_door_ids.insert(door.id.clone());
                        cite_pass += 1;
                    }
                    None => gap(&mut gaps, "cite-resolution", &who, "door", d),
                }
            }
            for l in &ledgers {
                match &ledger_text {
                    Some((_, text)) if contains_id(text, l) => {
                        cited_ledgers.insert(l.clone());
                        cite_pass += 1;
                    }
                    _ => gap(&mut gaps, "cite-resolution", &who, "ledger-row", l),
                }
            }
            for (plat, cite) in [
                ("web", &proof.web),
                ("tauri", &proof.tauri),
                ("android", &proof.android),
            ] {
                let Some(c) = cite else { continue };
                match check_exists(root, profile, c, &mut proof_specs) {
                    CheckState::Exists => cite_pass += 1,
                    CheckState::Missing => {
                        if status == "GREEN" {
                            gap(
                                &mut gaps,
                                "verdict-freshness",
                                &who,
                                "green-check-missing",
                                &format!("{plat}:{c}"),
                            );
                        } else {
                            gap(
                                &mut gaps,
                                "cite-resolution",
                                &who,
                                "proof-check-missing",
                                &format!("{plat}:{c}"),
                            );
                        }
                    }
                    CheckState::SpecAbsent => gap(
                        &mut gaps,
                        "cite-resolution",
                        &who,
                        "check-spec-absent",
                        &format!("{plat}:{c}"),
                    ),
                    CheckState::Undeclared => {
                        return Err(Abort {
                            exit_code: exit::OPERATIONAL,
                            messages: vec![
                                "profile declares no proof-glob, but spines cite proof checks"
                                    .into(),
                            ],
                        });
                    }
                }
            }
            if status == "GREEN" {
                if proof.web.is_none() {
                    gap(
                        &mut gaps,
                        "verdict-freshness",
                        &who,
                        "green-without-named-check",
                        "proof.web",
                    );
                } else {
                    verdict_pass += 1;
                }
            } else if let Some(c) = &proof.web
                && matches!(
                    check_exists(root, profile, c, &mut proof_specs),
                    CheckState::Exists
                )
            {
                info.push(json!({ "leg": who, "kind": "candidate-staleness", "detail": format!("status {status} but the named check {c} exists in the spec — doc verdict may be older than the walk") }));
            }
        }
        structure_gates(s, edges.as_ref(), &mut gaps, &mut cited_edges);

        // stale-open: open units (non-GREEN legs and forks) must name what
        // unblocks them. Kinds (each keyed into the ratchet target, so a
        // grandfathered row never masks another kind at the same unit):
        // - missing-waits-on: open unit declares no waits_on at all;
        // - unblock-resolved: a mile token resolves — the wait is over,
        //   the unit must be walked or re-declared (the predicate is
        //   check-exists on the declared proof source, same as
        //   verdict-freshness; it never claims the mile passed);
        // - check-exists: PARTIAL whose own proof.web check exists while
        //   it waits on no landable (unresolved) mile — waiting on a
        //   decision alone cannot defer this kind.
        for u in spine::stale_walk(s) {
            if u.status == "GREEN" {
                continue; // GREEN units cannot declare waits (grammar)
            }
            stale_units.insert(u.who.clone());
            if u.waits_on.is_empty() {
                gap(
                    &mut gaps,
                    "stale-open",
                    &u.who,
                    "missing-waits-on",
                    &format!("{}.missing-waits-on", u.who),
                );
            }
            let mut pending_mile = false;
            for token in &u.waits_on {
                let Some(mile) = token.strip_prefix("mile ") else {
                    continue; // dependency: declaration only, never resolves
                };
                match check_exists(root, profile, mile, &mut proof_specs) {
                    CheckState::Exists => gap(
                        &mut gaps,
                        "stale-open",
                        &u.who,
                        "unblock-resolved",
                        &format!("{}.unblock-resolved", u.who),
                    ),
                    CheckState::Undeclared => {
                        return Err(Abort {
                            exit_code: exit::OPERATIONAL,
                            messages: vec![
                                "profile declares no proof-glob, but spines cite mile waits_on"
                                    .into(),
                            ],
                        });
                    }
                    CheckState::Missing | CheckState::SpecAbsent => pending_mile = true,
                }
            }
            if u.status == "PARTIAL"
                && !pending_mile
                && let Some(c) = &u.proof_web
                && matches!(
                    check_exists(root, profile, c, &mut proof_specs),
                    CheckState::Exists
                )
            {
                gap(
                    &mut gaps,
                    "stale-open",
                    &u.who,
                    "check-exists",
                    &format!("{}.check-exists", u.who),
                );
            }
        }
    }

    // Census coverage half: coverage is judged against the COMMITTED census
    // artifact only — a verdict never execs a census command.
    let mut skipped: Vec<&'static str> = Vec::new();
    let door_total = censored.as_ref().map_or(0, |c| c.len());
    match &censored {
        Some(c) => {
            for door_id in c.ids() {
                if !cited_door_ids.contains(door_id) {
                    gap(&mut gaps, "door-census", "census", "door-uncited", door_id);
                }
            }
        }
        None => skipped.push("door-census"),
    }
    let mut ledger_doc_ids = 0usize;
    let mut ledger_homed = 0usize;
    if let Some((_, doc)) = &ledger_text {
        let ids = ledger_ids(doc);
        ledger_doc_ids = ids.len();
        for id in &ids {
            if cited_ledgers.contains(id) || marker_marked(doc, id) {
                ledger_homed += 1;
            } else {
                gap(&mut gaps, "ledger-homing", "census", "ledger-homeless", id);
            }
        }
    } else {
        skipped.push("ledger-homing");
    }
    let mut edge_total = 0usize;
    match &edges {
        Some((solid, _dashed)) => {
            edge_total = solid.len();
            for e in solid {
                if !cited_edges.contains(e) {
                    gap(&mut gaps, "edge-cites", "census", "edge-uncited", e);
                }
            }
        }
        None => skipped.push("edge-cites"),
    }
    let mut prose_checked = 0usize;
    let mut legs_total = 0usize;
    for (_, s) in spines {
        prose_checked += prose_count(s);
        legs_total += s.legs.len();
    }

    // Ratchet: NEW gaps fail as constraint, BASELINED print INFO, RESOLVED
    // entries fail as drift (shrink — the ratchet law holds both ways).
    let mut resolved: Vec<Value> = Vec::new();
    let mut new_gaps: Vec<Value> = Vec::new();
    let base_entries = baseline::load(&root.join(profile.baseline_path())).map_err(|errors| {
        Abort {
            exit_code: exit::CONSTRAINT,
            messages: errors,
        }
    })?;
    let live_keys: BTreeSet<(String, String)> = gap_keys(&gaps);
    for g in gaps {
        let gate = g["gate"].as_str().unwrap_or_default().to_string();
        let target = g["target"].as_str().unwrap_or_default().to_string();
        match baseline::has(&base_entries, &gate, &target) {
            Some(e) => info.push(json!({ "gate": gate, "target": target, "kind": "BASELINED", "detail": format!("grandfathered: {} ({})", e.cite, e.date) })),
            None => new_gaps.push(g),
        }
    }
    for e in &base_entries {
        if !live_keys.contains(&(e.gate.clone(), e.target.clone())) {
            resolved.push(json!({ "gate": e.gate, "target": e.target, "kind": "baseline-resolved", "detail": format!("baseline entry no longer matches any gap — shrink baseline: {} ({})", e.cite, e.date) }));
        }
    }

    let baselined_per = |gate: &str| -> usize {
        info.iter()
            .filter(|i| i["kind"] == "BASELINED" && i["gate"] == gate)
            .count()
    };
    let fail = |gate: &str| -> usize {
        new_gaps
            .iter()
            .filter(|g| g["gate"] == gate)
            .count()
    };
    // Distinct units carrying any stale-open gap (new or baselined): the
    // row denominator counts units, while fail/baselined count gaps.
    let stale_problem_units: BTreeSet<&str> = new_gaps
        .iter()
        .filter(|g| g["gate"] == "stale-open")
        .filter_map(|g| g["leg"].as_str())
        .chain(info.iter().filter_map(|i| {
            if i["kind"] == "BASELINED" && i["gate"] == "stale-open" {
                i["target"].as_str().and_then(|t| t.rsplit_once('.')).map(|(w, _)| w)
            } else {
                None
            }
        }))
        .collect();

    let exit_code = if zero_floor || !new_gaps.is_empty() {
        exit::CONSTRAINT
    } else if !resolved.is_empty() {
        exit::STALE
    } else {
        exit::GREEN
    };

    let gates = vec![
        GateRow {
            name: "cite-resolution",
            pass: cite_pass,
            baselined: baselined_per("cite-resolution"),
            fail: fail("cite-resolution"),
            of: 0,
        },
        GateRow {
            name: "verdict-freshness",
            pass: verdict_pass,
            baselined: baselined_per("verdict-freshness"),
            fail: fail("verdict-freshness"),
            of: 0,
        },
        GateRow {
            name: "door-census",
            pass: door_total - fail("door-census") - baselined_per("door-census"),
            baselined: baselined_per("door-census"),
            fail: fail("door-census"),
            of: door_total,
        },
        GateRow {
            name: "ledger-homing",
            pass: ledger_homed - fail("ledger-homing"),
            baselined: baselined_per("ledger-homing"),
            fail: fail("ledger-homing"),
            of: ledger_doc_ids,
        },
        GateRow {
            name: "composition-census",
            pass: legs_total - fail("composition-census"),
            baselined: baselined_per("composition-census"),
            fail: fail("composition-census"),
            of: 0,
        },
        GateRow {
            name: "forks-budget",
            pass: legs_total - fail("forks-budget"),
            baselined: baselined_per("forks-budget"),
            fail: fail("forks-budget"),
            of: 0,
        },
        GateRow {
            name: "edge-cites",
            pass: edge_total - fail("edge-cites") - baselined_per("edge-cites"),
            baselined: baselined_per("edge-cites"),
            fail: fail("edge-cites"),
            of: edge_total,
        },
        GateRow {
            name: "asd-lint",
            pass: prose_checked - fail("asd-lint") - baselined_per("asd-lint"),
            baselined: baselined_per("asd-lint"),
            fail: fail("asd-lint"),
            of: prose_checked,
        },
        GateRow {
            name: "stale-open",
            // Units, not gaps: one unit can carry two kinds at once.
            pass: stale_units.len() - stale_problem_units.len(),
            baselined: baselined_per("stale-open"),
            fail: fail("stale-open"),
            of: stale_units.len(),
        },
    ];

    Ok(Outcome {
        exit_code,
        gates,
        gaps: new_gaps,
        resolved,
        info,
        skipped,
        zero_floor,
        prose_checked,
        fork_budget: spine::FORK_BUDGET_PROPOSED,
    })
}

fn gap_keys(gaps: &[Value]) -> BTreeSet<(String, String)> {
    gaps.iter()
        .map(|g| {
            (
                g["gate"].as_str().unwrap_or_default().to_string(),
                g["target"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

/// The structural gates that inspect one spine: composition grouping, the
/// fork budget, edge CLAIM resolution, and the ASD prose lint.
fn structure_gates(
    s: &Spine,
    edges: Option<&(BTreeSet<String>, BTreeSet<String>)>,
    gaps: &mut Vec<Value>,
    cited_edges: &mut BTreeSet<String>,
) {
    let jid = &s.journey;
    // composition-census: a journey with more than the threshold of legs MUST group.
    if s.legs.len() > spine::SCENE_GROUPING_THRESHOLD {
        let covered: BTreeSet<&str> = s
            .scenes
            .iter()
            .flat_map(|sc| sc.legs.iter().map(|l| l.as_str()))
            .collect();
        for leg in &s.legs {
            if leg.scene.is_none() || !covered.contains(leg.id.as_str()) {
                gap(
                    gaps,
                    "composition-census",
                    &format!("{jid}/{}", leg.id),
                    "must-group",
                    &format!("{jid}/{}", leg.id),
                );
            }
        }
    }
    // forks-budget: the proposed default, labelled in every report line.
    for leg in &s.legs {
        if leg.forks.len() > spine::FORK_BUDGET_PROPOSED {
            gap(
                gaps,
                "forks-budget",
                &format!("{jid}/{}", leg.id),
                "over-budget-proposed-default",
                &format!(
                    "{jid}/{} has {} forks (max {})",
                    leg.id,
                    leg.forks.len(),
                    spine::FORK_BUDGET_PROPOSED
                ),
            );
        }
    }
    // edge-cites, claims half: every edges[] claim resolves against the census.
    if let Some((solid, dashed)) = edges {
        for (leg_id, fork_id, edges_claimed) in walk_edges(s) {
            let who = match &fork_id {
                Some(f) => format!("{jid}/{leg_id}#{f}"),
                None => format!("{jid}/{leg_id}"),
            };
            for e in edges_claimed {
                if solid.contains(e.as_str()) || dashed.contains(e.as_str()) {
                    cited_edges.insert(e.clone());
                } else {
                    gap(
                        gaps,
                        "edge-cites",
                        &who,
                        "edge-unresolvable",
                        &format!("{who} claims {e:?} — neither a solid edge nor an allow-listed dashed edge"),
                    );
                }
            }
        }
    }
    asd_lint(s, gaps);
}

fn gap(list: &mut Vec<Value>, gate: &str, leg: &str, kind: &str, target: &str) {
    list.push(json!({ "gate": gate, "leg": leg, "kind": kind, "target": target }));
}

/// (leg id, optional fork id, edges) for legs and forks that claim edges.
fn walk_edges(s: &Spine) -> Vec<(String, Option<String>, Vec<String>)> {
    let mut out = Vec::new();
    for leg in &s.legs {
        if !leg.edges.is_empty() {
            out.push((leg.id.clone(), None, leg.edges.clone()));
        }
        for f in &leg.forks {
            if !f.edges.is_empty() {
                out.push((leg.id.clone(), Some(f.id.clone()), f.edges.clone()));
            }
        }
    }
    out
}

fn prose_count(s: &Spine) -> usize {
    let mut n = 0;
    for leg in &s.legs {
        n += prose_strings(leg.when.ui.as_deref(), &leg.given, &leg.then).len();
        for f in &leg.forks {
            n += prose_strings(f.when.ui.as_deref(), &f.given, &f.then).len();
        }
    }
    n
}

/// asd-lint over one spine. Rules (all deterministic):
/// - when.ui first word ∈ ASD_VERBS (lowercased, leading punctuation stripped);
/// - every sentence ≤ ASD_MAX_SENTENCE_WORDS words (ui/given/then);
/// - given/then sentences end in terminal punctuation (when.ui is a verb
///   phrase — exempt, documented);
/// - ASD_BANNED words appear nowhere (token boundary, case-insensitive).
fn asd_lint(s: &Spine, gaps: &mut Vec<Value>) {
    for leg in &s.legs {
        lint_prose(
            &format!("{}/{}", s.journey, leg.id),
            leg.when.ui.as_deref(),
            &leg.given,
            &leg.then,
            gaps,
        );
        for f in &leg.forks {
            lint_prose(
                &format!("{}/{}#{}", s.journey, leg.id, f.id),
                f.when.ui.as_deref(),
                &f.given,
                &f.then,
                gaps,
            );
        }
    }
}

fn prose_strings(ui: Option<&str>, given: &str, then: &[String]) -> Vec<(&'static str, usize, String)> {
    let mut out = Vec::new();
    if let Some(u) = ui {
        out.push(("ui", 1, u.to_string()));
    }
    out.push(("given", 1, given.to_string()));
    for (i, t) in then.iter().enumerate() {
        out.push(("then", i + 1, t.clone()));
    }
    out
}

fn lint_prose(
    who: &str,
    ui: Option<&str>,
    given: &str,
    then: &[String],
    gaps: &mut Vec<Value>,
) {
    if let Some(u) = ui {
        let first = first_word(u);
        if !ASD_VERBS.contains(&first.as_str()) {
            gap(
                gaps,
                "asd-lint",
                who,
                "ui-first-word-not-imperative",
                &format!("{who}.ui.first-word={first}"),
            );
        }
    }
    for (field, idx, text) in prose_strings(ui, given, then) {
        for (k, sentence) in sentences(&text).iter().enumerate() {
            let target = format!("{who}.{field}{idx}.s{}", k + 1);
            if sentence.split_whitespace().count() > ASD_MAX_SENTENCE_WORDS {
                gap(gaps, "asd-lint", who, "sentence-too-long", &target);
            }
            if field != "ui"
                && !sentence.ends_with('.')
                && !sentence.ends_with('!')
                && !sentence.ends_with('?')
            {
                gap(
                    gaps,
                    "asd-lint",
                    who,
                    "sentence-no-terminal-punctuation",
                    &target,
                );
            }
            for w in ASD_BANNED {
                if contains_ci(sentence, w) {
                    gap(
                        gaps,
                        "asd-lint",
                        who,
                        "banned-word",
                        &format!("{target}.word={w}"),
                    );
                }
            }
        }
    }
}

/// First word: leading punctuation/backticks stripped, alphabetic head taken,
/// lowercased. "`Add project` → …" → "add"; "rest, then …" → "rest".
fn first_word(text: &str) -> String {
    let trimmed = text.trim_start_matches(|c: char| !c.is_alphabetic());
    let head: String = trimmed
        .chars()
        .take_while(|c| c.is_alphabetic() || *c == '-')
        .collect();
    head.to_lowercase()
}

/// Sentence split at .!? when the next character starts a new sentence:
/// whitespace, end, an upper-case letter or a backtick. Mid-sentence periods
/// ("audit #12 LANDED)", "v1.2.3") must NOT split — the next character is
/// lower-case there. Deterministic, no exceptions list.
fn sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        cur.push(chars[i]);
        let boundary = chars[i] == '.' || chars[i] == '!' || chars[i] == '?';
        let next_starts = i + 1 == chars.len()
            || chars[i + 1].is_whitespace()
            || chars[i + 1].is_uppercase()
            || chars[i + 1] == '`';
        if boundary && next_starts {
            out.push(cur.trim().to_string());
            cur.clear();
        }
        i += 1;
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

fn contains_ci(text: &str, word: &str) -> bool {
    let lower = text.to_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find(word) {
        let at = from + i;
        let before_ok = at == 0 || !lower.as_bytes()[at - 1].is_ascii_alphanumeric();
        let after_ok = !lower
            .as_bytes()
            .get(at + word.len())
            .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'-');
        if before_ok && after_ok {
            return true;
        }
        from = at + 1;
    }
    false
}

/// True when a doc line containing the id also carries the pile/parked
/// marker (case-insensitive "pile" or "parked") the ledger parking table uses.
fn marker_marked(doc: &str, id: &str) -> bool {
    doc.lines()
        .any(|line| contains_id(line, id) && (contains_ci(line, "pile") || contains_ci(line, "parked")))
}

/// All UI-*/PC-* ids in the ledger doc, token-boundary extracted.
fn ledger_ids(doc: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let bytes = doc.as_bytes();
    for prefix in ["UI-", "PC-"] {
        let mut from = 0;
        while let Some(i) = doc[from..].find(prefix) {
            let at = from + i;
            let before_ok = at == 0
                || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'-');
            let mut end = at + prefix.len();
            while bytes.get(end).is_some_and(|b| b.is_ascii_digit()) {
                end += 1;
            }
            let after_ok = !bytes.get(end).is_some_and(|b| b.is_ascii_alphanumeric());
            if before_ok && after_ok && end > at + prefix.len() {
                out.insert(doc[at..end].to_string());
            }
            from = at + 1;
        }
    }
    out
}

/// True when id occurs with token boundaries (UI-1 must not match UI-107).
fn contains_id(text: &str, id: &str) -> bool {
    let mut from = 0usize;
    while let Some(i) = text[from..].find(id) {
        let at = from + i;
        let before_ok = at == 0 || !text.as_bytes()[at - 1].is_ascii_alphanumeric();
        let after = at + id.len();
        let after_ok = !text
            .as_bytes()
            .get(after)
            .is_some_and(|b| b.is_ascii_alphanumeric());
        if before_ok && after_ok {
            return true;
        }
        from = at + 1;
    }
    false
}

enum CheckState {
    Exists,
    Missing,
    /// The cite's spec file exists but does not name the check.
    SpecAbsent,
    /// The spines cite proof checks and the profile declares no proof-glob.
    Undeclared,
}

/// A proof cite resolves when the declared source names `check <tail>`.
fn check_exists(
    root: &Path,
    profile: &Profile,
    cite: &str,
    cache: &mut BTreeMap<String, Option<String>>,
) -> CheckState {
    let Some((head, tail)) = cite.split_once('#') else {
        return CheckState::Missing;
    };
    let Some(glob) = &profile.proof_glob else {
        return CheckState::Undeclared;
    };
    if !cache.contains_key(head) {
        let path = root.join(glob.replace("{head}", head));
        cache.insert(head.to_string(), std::fs::read_to_string(path).ok());
    }
    let Some(text) = &cache[head] else {
        return CheckState::SpecAbsent;
    };
    let needle = format!("check {tail}");
    let mut from = 0usize;
    while let Some(i) = text[from..].find(&needle) {
        let at = from + i;
        let before_ok = at == 0 || !text.as_bytes()[at - 1].is_ascii_alphanumeric();
        let after = at + needle.len();
        let after_ok = !text
            .as_bytes()
            .get(after)
            .is_some_and(|b| b.is_ascii_alphanumeric());
        if before_ok && after_ok {
            return CheckState::Exists;
        }
        from = at + 1;
    }
    CheckState::Missing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_word_strips_punctuation_and_lowercases() {
        assert_eq!(first_word("`Add project` then wait"), "add");
        assert_eq!(first_word("rest, then observe"), "rest");
        assert_eq!(first_word("Re-shoot the matrix"), "re-shoot");
    }

    #[test]
    fn sentences_keep_mid_sentence_periods() {
        let s = sentences("Run audit #12 LANDED on v1.2.3. Then stop.");
        assert_eq!(s.len(), 2, "{s:?}");
        assert!(s[0].contains("v1.2.3"), "{s:?}");
    }

    #[test]
    fn token_boundary_contains() {
        assert!(contains_id("see UI-107 here", "UI-107"));
        assert!(!contains_id("see UI-107 here", "UI-1"));
        assert!(!contains_id("utilized", "utilize"));
        assert!(contains_ci("Utilize nothing", "utilize"));
    }

    #[test]
    fn ledger_ids_end_at_digits() {
        let ids = ledger_ids("UI-1 row, PC-22 row, UI-108 row, UI- row");
        assert!(ids.contains("UI-1"));
        assert!(ids.contains("PC-22"));
        assert!(ids.contains("UI-108"));
        assert!(!ids.contains("UI-"), "no trailing digits is not an id");
    }

    #[test]
    fn pile_marker_counts_as_homed() {
        let doc = "| UI-9 | pile: waits on the board lane |\n| UI-10 | shipped |\n";
        assert!(marker_marked(doc, "UI-9"));
        assert!(!marker_marked(doc, "UI-10"));
    }
}
