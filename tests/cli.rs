//! CLI goldens: fixture repo + fixture profile, explicit exit-code asserts.
//! Hermetic: every fixture lives under a tmpdir; the binary is spawned with
//! its cwd set inside the fixture so profile discovery is exercised for real.

use std::path::Path;
use std::process::{Command, Output};

fn journeyspec(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_journeyspec"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("journeyspec runs")
}

fn code(out: &Output) -> i32 {
    out.status.code().expect("no signal")
}

/// A minimal complete surface: profile, one spine citing one census door,
/// one proof spec naming the cited check. Zero gaps.
fn fixture(root: &Path) {
    std::fs::write(
        root.join("journeyspec.toml"),
        "journeys-dir = \"journeys\"\ncensus = \"census.tsv\"\ndocs-dir = \"docs\"\ndoc-file = \"j.md\"\nproof-glob = \"specs/{head}.spec.mjs\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("journeys")).unwrap();
    std::fs::create_dir_all(root.join("specs")).unwrap();
    std::fs::write(root.join("census.tsv"), "GET\t/db\tok\n").unwrap();
    std::fs::write(
        root.join("journeys/uj-a.yaml"),
        "journey: UJ-A\ntitle: Fixture\nprovenance: fixture\nworlds:\n  clean-slate: { seed: 0, description: Fixture world. }\nlegs:\n  - id: \"1\"\n    given: The binary is built.\n    when:\n      ui: Open the page\n      doors: [\"rt:1\"]\n    then:\n      - The page renders.\n    ledger: []\n    proof: { web: \"uj-a#1\", tauri: null, android: null }\n    edges: []\n    forks: []\n    world: clean-slate\n    status: GREEN\n",
    )
    .unwrap();
    std::fs::write(root.join("specs/uj-a.spec.mjs"), "check 1 name\n").unwrap();
}

#[test]
fn verify_green_and_status_shapes_exit_zero() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let out = journeyspec(dir.path(), &["verify"]);
    assert_eq!(code(&out), 0, "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let out = journeyspec(dir.path(), &["status", "--json"]);
    assert_eq!(code(&out), 0);
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("status --json is one JSON object");
    assert_eq!(v["exit"], 0);
    assert_eq!(v["gaps"].as_array().unwrap().len(), 0);
    assert_eq!(v["gates"].as_array().unwrap().len(), 9);
    let out = journeyspec(dir.path(), &["status", "--md"]);
    assert_eq!(code(&out), 0);
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("| gate | pass |"),
        "status --md renders a table"
    );
}

#[test]
fn live_gap_without_baseline_entry_exits_two() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    // The door goes uncited: a live gap with no baseline entry.
    std::fs::write(
        dir.path().join("journeys/uj-a.yaml"),
        "journey: UJ-A\ntitle: Fixture\nprovenance: fixture\nworlds:\n  clean-slate: { seed: 0, description: Fixture world. }\nlegs:\n  - id: \"1\"\n    given: The binary is built.\n    when:\n      ui: Open the page\n      doors: []\n    then:\n      - The page renders.\n    ledger: []\n    proof: { web: \"uj-a#1\", tauri: null, android: null }\n    edges: []\n    forks: []\n    world: clean-slate\n    status: GREEN\n",
    )
    .unwrap();
    let out = journeyspec(dir.path(), &["verify"]);
    assert_eq!(code(&out), 2);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("door-census") && err.contains("GET /db"), "{err}");
}

#[test]
fn ratchet_round_trip_baselines_then_shrinks_to_one() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    // Baseline the uncited door: green again.
    std::fs::write(
        dir.path().join("journeys/uj-a.yaml"),
        "journey: UJ-A\ntitle: Fixture\nprovenance: fixture\nworlds:\n  clean-slate: { seed: 0, description: Fixture world. }\nlegs:\n  - id: \"1\"\n    given: The binary is built.\n    when:\n      ui: Open the page\n      doors: []\n    then:\n      - The page renders.\n    ledger: []\n    proof: { web: \"uj-a#1\", tauri: null, android: null }\n    edges: []\n    forks: []\n    world: clean-slate\n    status: GREEN\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("journeys/baseline.tsv"),
        "door-census\tGET /db\tfixture audit\t2026-10-09\n",
    )
    .unwrap();
    assert_eq!(code(&journeyspec(dir.path(), &["verify"])), 0);
    // Restore the cite: the entry matches no live gap — stale drift (1).
    fixture(dir.path());
    let out = journeyspec(dir.path(), &["verify"]);
    assert_eq!(code(&out), 1);
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("shrink baseline"),
        "the stale half of the ratchet law says shrink"
    );
}

#[test]
fn check_and_generate_round_trip_with_refusals() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    // No doc yet: stale (1).
    assert_eq!(code(&journeyspec(dir.path(), &["check"])), 1);
    // An authored file at the target: refusal (70) for both verbs.
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/j.md"), "# hand written\n").unwrap();
    assert_eq!(code(&journeyspec(dir.path(), &["check"])), 70);
    assert_eq!(code(&journeyspec(dir.path(), &["generate"])), 70);
    // generate --census without a declared census-command: refusal (70).
    assert_eq!(code(&journeyspec(dir.path(), &["generate", "--census"])), 70);
    // Remove the authored file; generate writes and check goes fresh.
    std::fs::remove_file(dir.path().join("docs/j.md")).unwrap();
    assert_eq!(code(&journeyspec(dir.path(), &["generate"])), 0);
    assert_eq!(code(&journeyspec(dir.path(), &["check"])), 0);
    // Rendering is deterministic: a second generate stays fresh.
    assert_eq!(code(&journeyspec(dir.path(), &["generate"])), 0);
    assert_eq!(code(&journeyspec(dir.path(), &["check"])), 0);
}

#[test]
fn grammar_break_is_two_and_missing_artifact_is_seventy() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    std::fs::write(dir.path().join("journeys/uj-b.yaml"), "journey: BAD\n").unwrap();
    assert_eq!(code(&journeyspec(dir.path(), &["verify"])), 2);
    std::fs::remove_file(dir.path().join("journeys/uj-b.yaml")).unwrap();
    // A declared artifact that cannot be read: the verdict did not run (70).
    std::fs::remove_file(dir.path().join("census.tsv")).unwrap();
    assert_eq!(code(&journeyspec(dir.path(), &["verify"])), 70);
}

#[test]
fn zero_census_floor_is_never_ratchetable() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    std::fs::write(dir.path().join("census.tsv"), "").unwrap();
    // Even a baseline that would forgive every door cannot lift the floor.
    std::fs::write(
        dir.path().join("journeys/baseline.tsv"),
        "door-census\tGET /db\tfixture audit\t2026-10-09\n",
    )
    .unwrap();
    let out = journeyspec(dir.path(), &["verify"]);
    assert_eq!(code(&out), 2);
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("zero-census floor"),
        "the floor failure is named on stdout"
    );
}

#[test]
fn status_open_lists_non_green_legs_and_marks_existing_checks() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    // One GREEN leg keeps its named check; one PARTIAL leg waits on a
    // mile that has not landed and names a check that already exists.
    std::fs::write(
        dir.path().join("journeys/uj-a.yaml"),
        "journey: UJ-A\ntitle: Fixture\nprovenance: fixture\nworlds:\n  clean-slate: { seed: 0, description: Fixture world. }\nlegs:\n  - id: \"1\"\n    given: The binary is built.\n    when:\n      ui: Open the page\n      doors: [\"rt:1\"]\n    then:\n      - The page renders.\n    ledger: []\n    proof: { web: \"uj-a#1\", tauri: null, android: null }\n    edges: []\n    forks: []\n    world: clean-slate\n    status: GREEN\n  - id: \"2\"\n    given: A second mile is planned.\n    when:\n      ui: Open the second page\n      doors: []\n    then:\n      - The second page renders.\n    ledger: []\n    proof: { web: \"uj-a#2\", tauri: null, android: null }\n    edges: []\n    forks: []\n    world: clean-slate\n    status: PARTIAL\n    note: waits for the second mile to land green.\n    waits_on: [\"mile uj-a#3\"]\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("specs/uj-a.spec.mjs"), "check 1 name\ncheck 2 name\n").unwrap();
    let out = journeyspec(dir.path(), &["status", "--open"]);
    assert_eq!(code(&out), 0, "a display never gates: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("UJ-A/2"), "the open leg is listed: {text}");
    assert!(text.contains("PARTIAL"), "the status prints: {text}");
    assert!(text.contains("check-exists"), "the named check resolving now is marked: {text}");
    assert!(text.contains("mile uj-a#3"), "the waits_on tokens print: {text}");
    assert!(text.contains("waits for the second mile"), "the note prints: {text}");
    assert!(
        !text.contains("UJ-A/1 "),
        "the GREEN leg stays out of the census: {text}"
    );
    let out = journeyspec(dir.path(), &["status", "--json"]);
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("status --json is one JSON object");
    assert_eq!(v["open"].as_array().unwrap().len(), 1);
    assert_eq!(v["open"][0]["leg"], "UJ-A/2");
    assert_eq!(v["open"][0]["check-exists"], true);
    assert_eq!(v["open"][0]["waits_on"][0], "mile uj-a#3");
}

/// Overwrite the spine with a GREEN leg 1 plus one open leg 2 whose status,
/// proof.web cite and waits_on tail are under test. `spec` is the proof
/// source content. The second mile check is named only when `spec` says so.
fn open_leg_fixture(root: &Path, status: &str, proof: &str, waits: &str, spec: &str) {
    let spine = format!(
        "journey: UJ-A\ntitle: Fixture\nprovenance: fixture\nlegs:\n  - id: \"1\"\n    given: The binary is built.\n    when:\n      ui: Open the page\n      doors: [\"rt:1\"]\n    then:\n      - The page renders.\n    ledger: []\n    proof: {{ web: \"uj-a#1\", tauri: null, android: null }}\n    edges: []\n    forks: []\n    status: GREEN\n  - id: \"2\"\n    given: A second mile is planned.\n    when:\n      ui: Open the second page\n      doors: []\n    then:\n      - The second page renders.\n    ledger: []\n    proof: {{ web: {proof}, tauri: null, android: null }}\n    edges: []\n    forks: []\n    status: {status}\n{waits}"
    );
    std::fs::write(root.join("journeys/uj-a.yaml"), spine).unwrap();
    std::fs::write(root.join("specs/uj-a.spec.mjs"), spec).unwrap();
}

#[test]
fn stale_open_missing_waits_on_is_red() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    open_leg_fixture(dir.path(), "PARTIAL", "null", "", "check 1 name\n");
    let out = journeyspec(dir.path(), &["verify"]);
    assert_eq!(code(&out), 2);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("stale-open") && err.contains("missing-waits-on"), "{err}");
    assert!(err.contains("UJ-A/2.missing-waits-on"), "the target carries the kind: {err}");
    // The census returns the verdict's exit — the gate rides the verdict.
    assert_eq!(code(&journeyspec(dir.path(), &["status", "--open"])), 2);
}

#[test]
fn stale_open_unblock_resolved_is_red() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    // BLOCKED leg waits on mile uj-a#2 — and check 2 now exists in the spec.
    open_leg_fixture(
        dir.path(),
        "BLOCKED",
        "null",
        "    waits_on: [\"mile uj-a#2\"]\n",
        "check 1 name\ncheck 2 name\n",
    );
    let out = journeyspec(dir.path(), &["verify"]);
    assert_eq!(code(&out), 2);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("unblock-resolved") && err.contains("UJ-A/2.unblock-resolved"), "{err}");
}

#[test]
fn stale_open_check_exists_needs_a_mile_wait_to_defer() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    // PARTIAL, its own check exists, and it waits on a decision only:
    // a dependency cannot defer the kind (declaration only, never resolves).
    open_leg_fixture(
        dir.path(),
        "PARTIAL",
        "\"uj-a#2\"",
        "    waits_on: [\"dependency CAND-X9\"]\n",
        "check 1 name\ncheck 2 name\n",
    );
    let out = journeyspec(dir.path(), &["verify"]);
    assert_eq!(code(&out), 2);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("check-exists") && err.contains("UJ-A/2.check-exists"), "{err}");
}

#[test]
fn stale_open_honest_partial_is_green() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    // PARTIAL with its green half anchored and the red half waiting on a
    // mile that has not landed — honest, stays green until the mile lands.
    open_leg_fixture(
        dir.path(),
        "PARTIAL",
        "\"uj-a#2\"",
        "    waits_on: [\"mile uj-a#3\"]\n",
        "check 1 name\ncheck 2 name\n",
    );
    let out = journeyspec(dir.path(), &["verify"]);
    assert_eq!(code(&out), 0, "stderr: {}", String::from_utf8_lossy(&out.stderr));
    // The pending mile is visible in the census for the auditor.
    let census = journeyspec(dir.path(), &["status", "--open"]);
    let text = String::from_utf8_lossy(&census.stdout);
    assert!(text.contains("mile uj-a#3"), "waits_on prints: {text}");
    // A no-check PARTIAL waiting on a decision is the canonical shape.
    open_leg_fixture(
        dir.path(),
        "PARTIAL",
        "null",
        "    waits_on: [\"dependency CAND-X9\"]\n",
        "check 1 name\n",
    );
    assert_eq!(code(&journeyspec(dir.path(), &["verify"])), 0);
}

#[test]
fn stale_open_grandfathered_row_baselines_then_shrinks() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    open_leg_fixture(dir.path(), "PARTIAL", "null", "", "check 1 name\n");
    assert_eq!(code(&journeyspec(dir.path(), &["verify"])), 2);
    // The kind-keyed target: a row for one kind never masks another kind.
    std::fs::write(
        dir.path().join("journeys/baseline.tsv"),
        "stale-open\tUJ-A/2.missing-waits-on\tgrandfather tranche\t2026-10-10\n",
    )
    .unwrap();
    assert_eq!(code(&journeyspec(dir.path(), &["verify"])), 0);
    // Fix the leg (declare the wait): the row matches nothing — shrink (1).
    open_leg_fixture(
        dir.path(),
        "PARTIAL",
        "null",
        "    waits_on: [\"mile uj-a#3\"]\n",
        "check 1 name\n",
    );
    let out = journeyspec(dir.path(), &["verify"]);
    assert_eq!(code(&out), 1);
    assert!(String::from_utf8_lossy(&out.stderr).contains("shrink baseline"), "{out:?}");
}

#[test]
fn stale_open_mile_without_declared_proof_glob_is_seventy() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    // Strip the proof-glob from the profile; the mile cite cannot resolve.
    std::fs::write(
        dir.path().join("journeyspec.toml"),
        "journeys-dir = \"journeys\"\ncensus = \"census.tsv\"\ndocs-dir = \"docs\"\ndoc-file = \"j.md\"\n",
    )
    .unwrap();
    // Leg 1 cites NO proof check: only the mile token reaches the proof
    // plane, so the abort is provably the mile rule, not the proof-cite rule.
    std::fs::write(
        dir.path().join("journeys/uj-a.yaml"),
        "journey: UJ-A\ntitle: Fixture\nprovenance: fixture\nlegs:\n  - id: \"1\"\n    given: The binary is built.\n    when:\n      ui: Open the page\n      doors: [\"rt:1\"]\n    then:\n      - The page renders.\n    ledger: []\n    proof: { web: null, tauri: null, android: null }\n    edges: []\n    forks: []\n    status: RED\n  - id: \"2\"\n    given: A second mile is planned.\n    when:\n      ui: Open the second page\n      doors: []\n    then:\n      - The second page renders.\n    ledger: []\n    proof: { web: null, tauri: null, android: null }\n    edges: []\n    forks: []\n    status: BLOCKED\n    waits_on: [\"mile uj-a#2\"]\n",
    )
    .unwrap();
    let out = journeyspec(dir.path(), &["verify"]);
    assert_eq!(code(&out), 70, "an unresolvable resolution plane is 70, not a verdict: {}", String::from_utf8_lossy(&out.stderr));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("mile waits_on"),
        "the abort names the mile rule: {out:?}"
    );
}

#[test]
fn usage_surface_exits_sixty_four() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    for argv in [
        vec!["bogus-verb"],
        vec!["help", "bogus-topic"],
        vec!["verify", "--bogus"],
        vec!["scaffold"],
        vec!["status", "--json", "--md"],
        vec!["status", "--json", "--open"],
        vec!["verify", "--open"],
        vec![],
    ] {
        assert_eq!(code(&journeyspec(dir.path(), &argv)), 64, "argv {argv:?}");
    }
    for argv in [vec!["help"], vec!["version"], vec!["help", "commands"]] {
        assert_eq!(code(&journeyspec(dir.path(), &argv)), 0, "argv {argv:?}");
    }
}

#[test]
fn discovery_walks_up_and_config_overrides() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let deep = dir.path().join("a/b");
    std::fs::create_dir_all(&deep).unwrap();
    // Ancestor walk: the same verdict from a subdirectory.
    assert_eq!(code(&journeyspec(&deep, &["verify"])), 0);
    // An explicit --config answers from anywhere; nothing found fails loud.
    assert_eq!(
        code(&journeyspec(
            &deep,
            &["--config", dir.path().join("journeyspec.toml").to_str().unwrap(), "verify"]
        )),
        0
    );
    let lone = tempfile::tempdir().unwrap();
    assert_eq!(code(&journeyspec(lone.path(), &["verify"])), 70);
}

#[test]
fn scaffold_drafts_refuses_and_reports_uncited_doors() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    // Drop the door cite so the census door is uncited in every spine.
    std::fs::write(
        dir.path().join("journeys/uj-a.yaml"),
        "journey: UJ-A\ntitle: Fixture\nprovenance: fixture\nworlds:\n  clean-slate: { seed: 0, description: Fixture world. }\nlegs:\n  - id: \"1\"\n    given: The binary is built.\n    when:\n      ui: Open the page\n      doors: []\n    then:\n      - The page renders.\n    ledger: []\n    proof: { web: \"uj-a#1\", tauri: null, android: null }\n    edges: []\n    forks: []\n    world: clean-slate\n    status: GREEN\n",
    )
    .unwrap();
    let out = journeyspec(dir.path(), &["scaffold", "k"]);
    assert_eq!(code(&out), 0, "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(dir.path().join("journeys/uj-k.yaml").is_file());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("GET /db"), "the draft reports the uncited door: {stdout}");
    // The drafted spine cites nothing, so the door is now a live gap (2)…
    assert_eq!(code(&journeyspec(dir.path(), &["verify"])), 2);
    // …and an existing target is refused (70), never clobbered.
    let before = std::fs::read_to_string(dir.path().join("journeys/uj-k.yaml")).unwrap();
    assert_eq!(code(&journeyspec(dir.path(), &["scaffold", "k"])), 70);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("journeys/uj-k.yaml")).unwrap(),
        before
    );
    assert_eq!(code(&journeyspec(dir.path(), &["scaffold", "7"])), 64);
}
