//! spine — read a journey spine YAML and validate it against the declared
//! grammar (the profile's `schema` file is the normative shape; the grammar
//! is enforced structurally here, hermetic, zero schema crates). A violation
//! is a CONSTRAINT (exit 2): the spine breaks what the grammar declares.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

/// One journey spine file. Unknown keys are grammar violations: a typo'd
/// field (e.g. `waits-one:`) must never parse as absent (fail-closed).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spine {
    pub journey: String,
    pub title: String,
    #[serde(default)]
    pub provenance: Option<String>,
    #[serde(default)]
    pub worlds: BTreeMap<String, World>,
    #[serde(default)]
    pub scenes: Vec<Scene>,
    pub legs: Vec<Leg>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct World {
    pub seed: i64,
    pub description: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    pub id: String,
    pub title: String,
    pub legs: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Leg {
    pub id: String,
    #[serde(default)]
    pub scene: Option<String>,
    pub given: String,
    pub when: When,
    pub then: Vec<String>,
    #[serde(default)]
    pub ledger: Vec<String>,
    pub proof: Proof,
    #[serde(default)]
    pub edges: Vec<String>,
    #[serde(default)]
    pub forks: Vec<Fork>,
    #[serde(default)]
    pub world: Option<String>,
    pub status: String,
    #[serde(default)]
    pub note: Option<String>,
    /// What unblocks this leg while it is not GREEN. Typed tokens:
    /// `mile <proof-cite>` (machine-resolved via the declared proof
    /// source) or `dependency CAND-<LETTER><digits>` (declaration only).
    #[serde(default)]
    pub waits_on: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct When {
    #[serde(default)]
    pub ui: Option<String>,
    #[serde(default)]
    pub doors: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proof {
    #[serde(default)]
    pub web: Option<String>,
    #[serde(default)]
    pub tauri: Option<String>,
    #[serde(default)]
    pub android: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fork {
    pub id: String,
    #[serde(default)]
    pub world: Option<String>,
    pub given: String,
    pub when: When,
    pub then: Vec<String>,
    #[serde(default)]
    pub ledger: Vec<String>,
    pub proof: Proof,
    #[serde(default)]
    pub edges: Vec<String>,
    pub status: String,
    #[serde(default)]
    pub note: Option<String>,
    /// What unblocks this fork while it is not GREEN (see `Leg::waits_on`).
    #[serde(default)]
    pub waits_on: Vec<String>,
}

pub const STATUSES: [&str; 6] = ["GREEN", "RED", "PARTIAL", "BLOCKED", "PARKED", "SKIP"];
/// Proposed fork budget per leg.
pub const FORK_BUDGET_PROPOSED: usize = 4;
/// A journey with more than this many legs MUST group its legs into scenes.
pub const SCENE_GROUPING_THRESHOLD: usize = 12;

/// Load and parse one spine file. Parse failure is a grammar violation.
pub fn load(path: &Path) -> Result<Spine, Vec<String>> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| vec![format!("{}: cannot read spine: {e}", path.display())])?;
    let spine: Spine = serde_yaml::from_str(&text)
        .map_err(|e| vec![format!("{}: spine violates the grammar: {e}", path.display())])?;
    let errors = validate(&spine);
    if errors.is_empty() {
        Ok(spine)
    } else {
        Err(errors)
    }
}

/// Structural validation mirroring the declared grammar. One message per violation.
pub fn validate(s: &Spine) -> Vec<String> {
    let mut e = Vec::new();
    if !is_upper_journey(&s.journey) {
        e.push(format!("journey {r:?}: grammar wants ^UJ-[A-Z]$", r = s.journey));
    }
    if s.title.trim().len() < 3 {
        e.push("title: minLength 3 violated".into());
    }
    if s.legs.is_empty() {
        e.push("legs: at least one leg is required".into());
    }
    let mut seen: Vec<&str> = Vec::new();
    for leg in &s.legs {
        if !digits_only(&leg.id) {
            e.push(format!("leg {r:?}: id must be digits only", r = leg.id));
        }
        if !seen.contains(&leg.id.as_str()) {
            seen.push(&leg.id);
        } else {
            e.push(format!("leg {r:?}: duplicate leg id", r = leg.id));
        }
        check_fields(
            s,
            &mut e,
            Fields {
                kind: "leg",
                id: &leg.id,
                given: &leg.given,
                then: &leg.then,
                ledger: &leg.ledger,
                doors: leg.doors(),
                proof: &leg.proof,
                world: &leg.world,
                status: &leg.status,
                waits: &leg.waits_on,
            },
        );
        // The fork budget and the >12-leg grouping law are GATES (verify),
        // not grammar: they carry baseline/ratchet semantics. See verdict.rs.
        for f in &leg.forks {
            if !fork_id_ok(&f.id, &leg.id) {
                e.push(format!(
                    "fork {r:?}: id must be the parent leg id plus one letter",
                    r = f.id
                ));
            }
            check_fields(
                s,
                &mut e,
                Fields {
                    kind: "fork",
                    id: &f.id,
                    given: &f.given,
                    then: &f.then,
                    ledger: &f.ledger,
                    doors: f.doors(),
                    proof: &f.proof,
                    world: &f.world,
                    status: &f.status,
                    waits: &f.waits_on,
                },
            );
        }
    }
    // Scene grouping: every scene leg must exist; scene ids must be unique.
    let mut covered: Vec<&str> = Vec::new();
    let mut scene_ids: Vec<&str> = Vec::new();
    for sc in &s.scenes {
        if !scene_id_ok(&sc.id) {
            e.push(format!(
                "scene {r:?}: id must match ^[a-z0-9][a-z0-9-]*$",
                r = sc.id
            ));
        }
        if scene_ids.contains(&sc.id.as_str()) {
            e.push(format!("scene {r:?}: duplicate scene id", r = sc.id));
        }
        scene_ids.push(&sc.id);
        for lid in &sc.legs {
            if !s.legs.iter().any(|l| &l.id == lid) {
                e.push(format!(
                    "scene {sc}: leg {lid} does not exist",
                    sc = sc.id,
                    lid = lid
                ));
            }
            if covered.contains(&lid.as_str()) {
                e.push(format!(
                    "scene {sc}: leg {lid} is grouped twice",
                    sc = sc.id,
                    lid = lid
                ));
            }
            covered.push(lid);
        }
    }
    for leg in &s.legs {
        // The ">12 legs MUST group" law is a verify gate (composition-census)
        // with baseline/ratchet semantics, not grammar; only scene-name
        // integrity stays here.
        if let Some(scene) = &leg.scene
            && !scene_ids.contains(&scene.as_str())
        {
            e.push(format!(
                "leg {id}: scene {scene:?} does not exist",
                id = leg.id,
                scene = scene
            ));
        }
    }
    e
}

/// One leg or fork viewed by check_fields (argument-count law: fields, not 12 params).
pub struct Fields<'a> {
    pub kind: &'static str,
    pub id: &'a str,
    pub given: &'a str,
    pub then: &'a [String],
    pub ledger: &'a [String],
    pub doors: &'a [String],
    pub proof: &'a Proof,
    pub world: &'a Option<String>,
    pub status: &'a str,
    pub waits: &'a [String],
}

fn check_fields(s: &Spine, e: &mut Vec<String>, f: Fields<'_>) {
    let tag = format!("{} {}", f.kind, f.id);
    if f.given.trim().len() < 3 {
        e.push(format!("{tag}: given needs minLength 3"));
    }
    if f.then.is_empty() || f.then.iter().any(|t| t.trim().len() < 3) {
        e.push(format!(
            "{tag}: then needs at least one outcome of minLength 3"
        ));
    }
    for d in f.doors {
        if !door_cite_ok(d) {
            e.push(format!("{tag}: door {d:?} must match ^rt:[0-9]+$"));
        }
    }
    for l in f.ledger {
        if !ledger_id_ok(l) {
            e.push(format!(
                "{tag}: ledger cite {l:?} must match ^(UI|PC)-[0-9]+$"
            ));
        }
    }
    for (plat, cite) in [
        ("web", &f.proof.web),
        ("tauri", &f.proof.tauri),
        ("android", &f.proof.android),
    ] {
        if let Some(c) = cite
            && !proof_id_ok(c)
        {
            e.push(format!(
                "{tag}: proof.{plat} {c:?} must match ^[a-z0-9-]+#[0-9a-z]+$"
            ));
        }
    }
    if !STATUSES.contains(&f.status) {
        e.push(format!("{tag}: status {st:?} is outside {STATUSES:?}", st = f.status));
    }
    if !f.waits.is_empty() && f.status == "GREEN" {
        e.push(format!(
            "{tag}: a GREEN unit declares waits_on — a walked unit waits on nothing"
        ));
    }
    for w in f.waits {
        if let Some(msg) = waits_token_err(w) {
            e.push(format!("{tag}: waits_on {w:?} {msg}"));
        }
    }
    if let Some(w) = f.world
        && !s.worlds.contains_key(w)
    {
        e.push(format!(
            "{tag}: world {w:?} is not declared in the worlds map"
        ));
    }
}

impl Leg {
    pub fn doors(&self) -> &[String] {
        &self.when.doors
    }
}

impl Fork {
    pub fn doors(&self) -> &[String] {
        &self.when.doors
    }
}

/// One walked unit: (leg id, optional fork id, status, proof, door cites, ledger cites).
pub type WalkItem = (String, Option<String>, String, Proof, Vec<String>, Vec<String>);

pub fn walk(s: &Spine) -> Vec<WalkItem> {
    let mut out = Vec::new();
    for leg in &s.legs {
        out.push((
            leg.id.clone(),
            None,
            leg.status.clone(),
            proof_of(&leg.proof),
            leg.doors().to_vec(),
            leg.ledger.clone(),
        ));
        for f in &leg.forks {
            out.push((
                leg.id.clone(),
                Some(f.id.clone()),
                f.status.clone(),
                proof_of(&f.proof),
                f.doors().to_vec(),
                f.ledger.clone(),
            ));
        }
    }
    out
}

fn proof_of(p: &Proof) -> Proof {
    Proof {
        web: p.web.clone(),
        tauri: p.tauri.clone(),
        android: p.android.clone(),
    }
}

fn is_upper_journey(s: &str) -> bool {
    s.len() == 4 && s.starts_with("UJ-") && s.as_bytes()[3].is_ascii_uppercase()
}
fn digits_only(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}
fn fork_id_ok(id: &str, parent: &str) -> bool {
    id.len() == parent.len() + 1
        && id.starts_with(parent)
        && id[parent.len()..].chars().all(|c| c.is_ascii_lowercase())
}
fn scene_id_ok(s: &str) -> bool {
    let mut c = s.chars();
    c.next()
        .is_some_and(|f| f.is_ascii_lowercase() || f.is_ascii_digit())
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}
fn door_cite_ok(s: &str) -> bool {
    s.strip_prefix("rt:").is_some_and(digits_only)
}
fn ledger_id_ok(s: &str) -> bool {
    if let Some(rest) = s.strip_prefix("UI-").or(s.strip_prefix("PC-")) {
        digits_only(rest)
    } else {
        false
    }
}
fn proof_id_ok(s: &str) -> bool {
    let Some((head, tail)) = s.split_once('#') else {
        return false;
    };
    !head.is_empty()
        && head
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !tail.is_empty()
        && tail.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

/// Grammar of one `waits_on` token. Ok shape: `mile <proof-cite>` (the body
/// reuses the proof-cite grammar — one function, zero drift) or
/// `dependency CAND-<LETTER><digits>` (a decision id; declaration only).
/// Returns the violation message, or None when the token is well-formed.
fn waits_token_err(s: &str) -> Option<String> {
    if let Some(body) = s.strip_prefix("mile ") {
        if proof_id_ok(body) {
            None
        } else {
            Some("mile wants a proof cite ^[a-z0-9-]+#[0-9a-z]+$".into())
        }
    } else if let Some(body) = s.strip_prefix("dependency ") {
        if cand_id_ok(body) {
            None
        } else {
            Some("dependency wants ^CAND-[A-Z][0-9]+$".into())
        }
    } else {
        Some("wants the shape mile <proof-cite> or dependency CAND-<LETTER><digits>".into())
    }
}

fn cand_id_ok(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("CAND-") else {
        return false;
    };
    let mut chars = rest.chars();
    let first = chars.next();
    let Some(second) = chars.next() else {
        return false;
    };
    first.is_some_and(|c| c.is_ascii_uppercase())
        && second.is_ascii_digit()
        && chars.all(|c| c.is_ascii_digit())
}

/// One unit seen by the stale-open gate: its stable who-address, status,
/// proof.web cite, and waits_on tokens. Separate from WalkItem on purpose:
/// the gate needs waits_on and none of the door/ledger cites.
pub struct StaleUnit {
    pub who: String,
    pub status: String,
    pub proof_web: Option<String>,
    pub waits_on: Vec<String>,
}

pub fn stale_walk(s: &Spine) -> Vec<StaleUnit> {
    let mut out = Vec::new();
    for leg in &s.legs {
        out.push(StaleUnit {
            who: format!("{}/{}", s.journey, leg.id),
            status: leg.status.clone(),
            proof_web: leg.proof.web.clone(),
            waits_on: leg.waits_on.clone(),
        });
        for f in &leg.forks {
            out.push(StaleUnit {
                who: format!("{}/{}#{}", s.journey, leg.id, f.id),
                status: f.status.clone(),
                proof_web: f.proof.web.clone(),
                waits_on: f.waits_on.clone(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spine_yaml(body: &str) -> Spine {
        serde_yaml::from_str(body).expect("fixture spine parses")
    }

    const OK: &str = r#"
journey: UJ-A
title: A journey that is fine
legs:
  - id: "1"
    given: The tool is built.
    when:
      ui: Open the page
      doors: ["rt:1"]
    then:
      - The page shows the board.
    ledger: []
    proof: { web: null, tauri: null, android: null }
    edges: []
    forks: []
    status: RED
"#;

    #[test]
    fn valid_spine_has_no_violations() {
        assert!(validate(&spine_yaml(OK)).is_empty());
    }

    #[test]
    fn grammar_violations_are_named() {
        let s = spine_yaml(
            r#"
journey: uj-a
title: x
legs:
  - id: "1"
    given: ab
    when: { ui: null, doors: ["rt:x", "9"] }
    then: ["ok enough"]
    ledger: ["UI"]
    proof: { web: "BAD#", tauri: null, android: null }
    edges: []
    forks: [{ id: "1A", given: ok ok, when: { ui: null, doors: [] }, then: ["fine fine"], ledger: [], proof: { web: null, tauri: null, android: null }, edges: [], status: NOPE }]
    status: GREEN
"#,
        );
        let e = validate(&s);
        assert!(e.iter().any(|x| x.contains("^UJ-[A-Z]$")), "{e:?}");
        assert!(e.iter().any(|x| x.contains("title: minLength 3")), "{e:?}");
        assert!(e.iter().any(|x| x.contains("rt:[0-9]+$")), "{e:?}");
        assert!(e.iter().any(|x| x.contains("ledger cite")), "{e:?}");
        assert!(e.iter().any(|x| x.contains("proof.web")), "{e:?}");
        assert!(e.iter().any(|x| x.contains("parent leg id plus one letter")), "{e:?}");
        assert!(e.iter().any(|x| x.contains("status \"NOPE\"")), "{e:?}");
    }

    #[test]
    fn scene_integrity_is_checked() {
        let s = spine_yaml(
            r#"
journey: UJ-A
title: Scene integrity
scenes:
  - id: SCENE
    title: Bad id
    legs: ["7", "1"]
legs:
  - id: "1"
    scene: missing
    given: The tool is built.
    when: { ui: null, doors: [] }
    then: ["ok ok ok"]
    ledger: []
    proof: { web: null, tauri: null, android: null }
    edges: []
    forks: []
    status: RED
"#,
        );
        let e = validate(&s);
        assert!(e.iter().any(|x| x.contains("scene \"SCENE\": id must match")), "{e:?}");
        assert!(e.iter().any(|x| x.contains("scene SCENE: leg 7 does not exist")), "{e:?}");
        assert!(e.iter().any(|x| x.contains("scene \"missing\" does not exist")), "{e:?}");
    }

    #[test]
    fn waits_on_tokens_are_typed() {
        let base = r#"
journey: UJ-A
title: Waits on grammar
legs:
  - id: "1"
    given: The tool is built.
    when: { ui: null, doors: [] }
    then: ["The board shows the run."]
    ledger: []
    proof: { web: null, tauri: null, android: null }
    edges: []
    forks: []
    status: PARTIAL
    "#;
        let ok = spine_yaml(&format!(
            "{base}\n    waits_on: [\"mile uj-a#7\", \"dependency CAND-X9\"]\n"
        ));
        assert!(validate(&ok).is_empty(), "{:?}", validate(&ok));
        let bad = spine_yaml(&format!(
            "{base}\n    waits_on: [\"mile nope\", \"dependency DEC-7\", \"blocked on CAND-X9\"]\n"
        ));
        let e = validate(&bad);
        assert!(e.iter().any(|x| x.contains("mile wants a proof cite")), "{e:?}");
        assert!(e.iter().any(|x| x.contains("dependency wants")), "{e:?}");
        assert!(e.iter().any(|x| x.contains("wants the shape")), "{e:?}");
    }

    #[test]
    fn green_unit_must_not_declare_waits_on() {
        let s = spine_yaml(
            r#"
journey: UJ-A
title: Green waits on nothing
legs:
  - id: "1"
    given: The tool is built.
    when: { ui: null, doors: [] }
    then: ["The board shows the run."]
    ledger: []
    proof: { web: uj-a#1, tauri: null, android: null }
    edges: []
    forks: []
    status: GREEN
    waits_on: ["mile uj-a#9"]
"#,
        );
        let e = validate(&s);
        assert!(e.iter().any(|x| x.contains("GREEN unit declares waits_on")), "{e:?}");
    }

    #[test]
    fn unknown_spine_fields_are_rejected() {
        // A typo'd waits_on must never parse as absent (fail-closed).
        let err = serde_yaml::from_str::<Spine>(
            r#"
journey: UJ-A
title: Typo gate input
legs:
  - id: "1"
    given: The tool is built.
    when: { ui: null, doors: [] }
    then: ["The board shows the run."]
    ledger: []
    proof: { web: null, tauri: null, android: null }
    edges: []
    forks: []
    status: PARTIAL
    waits-one: ["mile uj-a#7"]
"#,
        );
        assert!(err.is_err(), "unknown field must be a grammar violation");
    }
}
