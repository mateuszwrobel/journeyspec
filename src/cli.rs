//! cli — verb dispatch and output shapes.
//!
//! Every verb resolves the same way: discover the profile (never a CWD
//! guess), load it, load the spines it declares, then act. A missing
//! precondition exits OPERATIONAL (70) with the path named — a verdict
//! that could not run is never reported as a verdict. Grammar breaks are
//! CONSTRAINT (2); bad argv is USAGE (64).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

use crate::census::Census;
use crate::config::{self, Profile};
use crate::exit;
use crate::help;
use crate::render;
use crate::scaffold;
use crate::spine::{self, Spine};
use crate::verdict;

/// The exit-class name each code prints with.
fn class_name(code: i32) -> &'static str {
    match code {
        exit::GREEN => "green",
        exit::STALE => "stale-drift",
        exit::CONSTRAINT => "constraint-violation",
        exit::USAGE => "usage",
        _ => "operational",
    }
}

fn err(msg: impl std::fmt::Display, code: i32) -> i32 {
    eprintln!("journeyspec: {msg}");
    code
}

/// One parsed invocation.
struct Args {
    verb: String,
    positionals: Vec<String>,
    config: Option<PathBuf>,
    json: bool,
    md: bool,
    census: bool,
    open: bool,
}

fn parse(argv: &[String]) -> Result<Args, String> {
    let mut a = Args {
        verb: String::new(),
        positionals: Vec::new(),
        config: None,
        json: false,
        md: false,
        census: false,
        open: false,
    };
    let mut i = 0;
    let mut verb_seen = false;
    while i < argv.len() {
        let arg = argv[i].as_str();
        match arg {
            "--config" => {
                i += 1;
                let Some(p) = argv.get(i) else {
                    return Err("--config needs a PATH argument".into());
                };
                a.config = Some(PathBuf::from(p));
            }
            "--json" => {
                if a.md {
                    return Err("--json and --md cannot combine".into());
                }
                a.json = true;
            }
            "--md" => {
                if a.json {
                    return Err("--json and --md cannot combine".into());
                }
                a.md = true;
            }
            "--open" => {
                if a.json || a.md {
                    return Err("--open is a display shape; it cannot combine with --json or --md".into());
                }
                a.open = true;
            }
            "--census" => a.census = true,
            "-h" | "--help" if !verb_seen => {
                a.verb = "help".into();
                verb_seen = true;
            }
            "-V" | "--version" if !verb_seen => {
                a.verb = "version".into();
                verb_seen = true;
            }
            _ if arg.starts_with('-') && arg.len() > 1 => {
                return Err(format!("unknown flag {arg}"));
            }
            _ if !verb_seen => {
                a.verb = arg.to_string();
                verb_seen = true;
            }
            _ => a.positionals.push(arg.to_string()),
        }
        i += 1;
    }
    if !verb_seen {
        return Err("no verb given".into());
    }
    Ok(a)
}

/// Which argv shapes each verb accepts. Checked before any environment
/// question: a usage error is usage (64) even in an empty directory.
fn shape_ok(a: &Args) -> Result<(), String> {
    match a.verb.as_str() {
        "verify" => {
            if a.json || a.md || a.open {
                Err("verify has no output flags (use status --json or status --open)".into())
            } else if a.census {
                Err("--census belongs to generate".into())
            } else {
                Ok(())
            }
        }
        "check" => {
            if !a.positionals.is_empty() {
                Err("check takes no positional arguments".into())
            } else if a.json || a.md || a.census || a.open {
                Err("check takes no flags".into())
            } else {
                Ok(())
            }
        }
        "status" => {
            if a.census {
                Err("--census belongs to generate".into())
            } else {
                Ok(())
            }
        }
        "generate" => {
            if !a.positionals.is_empty() {
                Err("generate takes no positional arguments".into())
            } else if a.json || a.md {
                Err("generate takes no output flags".into())
            } else if a.open {
                Err("--open belongs to status".into())
            } else {
                Ok(())
            }
        }
        "scaffold" => {
            if a.json || a.md || a.census || a.open {
                Err("scaffold takes no output flags".into())
            } else if a.positionals.len() != 1 {
                Err("scaffold needs one letter (UJ-A..UJ-Z)".into())
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

/// Run one invocation from `cwd`; returns the process exit code.
pub fn run(argv: &[String], cwd: &Path) -> i32 {
    let args = match parse(argv) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("journeyspec: usage: {e}");
            eprintln!("{}", help::SUMMARY);
            return exit::USAGE;
        }
    };
    if args.positionals.iter().any(|p| p == "--help" || p == "-h") {
        println!("{}", help::SUMMARY);
        return exit::GREEN;
    }
    match args.verb.as_str() {
        "help" => {
            match args.positionals.as_slice() {
                [] => {
                    println!("{}", help::SUMMARY);
                    exit::GREEN
                }
                [topic] => match help::topic(topic) {
                    Some(text) => {
                        println!("{}", text.trim());
                        exit::GREEN
                    }
                    None => err(
                        format!("unknown help topic {topic:?}; topics: {}", help::TOPICS.join(", ")),
                        exit::USAGE,
                    ),
                },
                more => err(
                    format!("help takes one topic, got {}", more.len()),
                    exit::USAGE,
                ),
            }
        }
        "version" => {
            println!("journeyspec {}", env!("CARGO_PKG_VERSION"));
            exit::GREEN
        }
        "verify" | "check" | "status" | "generate" | "scaffold" => {
            // The argv shape is a usage question; it answers before any
            // environment question (profile discovery, artifacts).
            if let Err(e) = shape_ok(&args) {
                return err(e, exit::USAGE);
            }
            let (root, profile) = match open_profile(args.config.as_deref(), cwd) {
                Ok(rp) => rp,
                Err((code, msg)) => return err(msg, code),
            };
            match args.verb.as_str() {
                "verify" => verb_verify(&root, &profile, &args),
                "check" => verb_check(&root, &profile),
                "status" => verb_status(&root, &profile, &args),
                "generate" => verb_generate(&root, &profile, &args),
                _ => verb_scaffold(&root, &profile, &args),
            }
        }
        other => err(format!("unknown verb {other:?}"), exit::USAGE),
    }
}

/// Discover + load the profile; the (root, profile) pair every verb needs.
fn open_profile(
    explicit: Option<&Path>,
    cwd: &Path,
) -> Result<(PathBuf, Profile), (i32, String)> {
    let path = config::discover(explicit, cwd)
        .map_err(|e| (exit::OPERATIONAL, e.0))?;
    let profile = Profile::load(&path).map_err(|e| (exit::OPERATIONAL, e))?;
    Ok((Profile::root_of(&path), profile))
}

/// Load the spines the verb judges: named files, or the declared dir.
/// Grammar errors are CONSTRAINT (2); unreadable files are OPERATIONAL (70).
fn load_spines(
    root: &Path,
    profile: &Profile,
    named: &[String],
) -> Result<Vec<(PathBuf, Spine)>, (i32, String)> {
    let paths: Vec<PathBuf> = if named.is_empty() {
        let dir = root.join(&profile.journeys_dir);
        let entries = std::fs::read_dir(&dir)
            .map_err(|e| (exit::OPERATIONAL, format!("{}: cannot read spine dir: {e}", dir.display())))?;
        let mut ps: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "yaml"))
            .collect();
        ps.sort();
        ps
    } else {
        named.iter().map(|p| root.join(p)).collect()
    };
    if paths.is_empty() {
        return Err((
            exit::OPERATIONAL,
            format!(
                "{}: no spine YAML files — nothing to judge",
                root.join(&profile.journeys_dir).display()
            ),
        ));
    }
    let mut out = Vec::new();
    for p in paths {
        match spine::load(&p) {
            Ok(s) => out.push((p, s)),
            Err(errors) => {
                let detail = errors.join("\n");
                let code = if detail.contains("cannot read spine") {
                    exit::OPERATIONAL
                } else {
                    exit::CONSTRAINT
                };
                return Err((code, detail));
            }
        }
    }
    Ok(out)
}

fn run_verdict(
    root: &Path,
    profile: &Profile,
    spines: &[(PathBuf, Spine)],
) -> Result<verdict::Outcome, (i32, String)> {
    verdict::run(root, profile, spines).map_err(|abort| {
        (
            abort.exit_code,
            abort.messages.join("\n"),
        )
    })
}

fn print_gate_table(o: &verdict::Outcome) {
    for g in &o.gates {
        if g.name == "cite-resolution" || g.name == "verdict-freshness" {
            println!(
                "{:<20} pass {:>4}  baselined {:>3}  fail {}",
                g.name, g.pass, g.baselined, g.fail
            );
        } else {
            println!(
                "{:<20} pass {:>4}/{:<4} baselined {:>3}  fail {}",
                g.name, g.pass, g.of, g.baselined, g.fail
            );
        }
    }
    for sk in &o.skipped {
        println!("{sk:<20} skipped (profile declares no source)");
    }
    if o.zero_floor {
        println!("zero-census floor: FAILED — an empty census passes only a profile that declares zero");
    }
}

fn print_gaps(o: &verdict::Outcome) {
    if !o.gaps.is_empty() {
        eprintln!("GAPS ({}):", o.gaps.len());
        for g in &o.gaps {
            eprintln!(
                "  GAP {} {} {} {:?}",
                g["gate"].as_str().unwrap_or_default(),
                g["leg"].as_str().unwrap_or_default(),
                g["kind"].as_str().unwrap_or_default(),
                g["target"]
            );
        }
    }
    if !o.resolved.is_empty() {
        eprintln!("RESOLVED ({}) — baseline entries matching no live gap, shrink baseline:", o.resolved.len());
        for g in &o.resolved {
            eprintln!(
                "  RESOLVED {} {} — {}",
                g["gate"].as_str().unwrap_or_default(),
                g["target"].as_str().unwrap_or_default(),
                g["detail"].as_str().unwrap_or_default()
            );
        }
    }
}

fn outcome_json(o: &verdict::Outcome, spines: usize) -> Value {
    json!({
        "tool": "journeyspec",
        "exit": o.exit_code,
        "spines": spines,
        "zero_floor": o.zero_floor,
        "prose_checked": o.prose_checked,
        "fork_budget": o.fork_budget,
        "skipped": o.skipped,
        "gates": o.gates.iter().map(|g| json!({
            "name": g.name, "pass": g.pass, "baselined": g.baselined,
            "fail": g.fail, "of": g.of,
        })).collect::<Vec<_>>(),
        "gaps": o.gaps,
        "resolved": o.resolved,
        "info": o.info,
    })
}

fn verb_verify(root: &Path, profile: &Profile, args: &Args) -> i32 {
    let spines = match load_spines(root, profile, &args.positionals) {
        Ok(s) => s,
        Err((code, msg)) => return err(msg, code),
    };
    let outcome = match run_verdict(root, profile, &spines) {
        Ok(o) => o,
        Err((code, msg)) => return err(msg, code),
    };
    println!(
        "journeyspec verify — {} spine(s), census {}",
        spines.len(),
        match &profile.census {
            Some(p) => p.display().to_string(),
            None => "(undeclared)".into(),
        }
    );
    print_gate_table(&outcome);
    print_gaps(&outcome);
    println!(
        "verify: exit {} ({})",
        outcome.exit_code, class_name(outcome.exit_code)
    );
    outcome.exit_code
}

/// The open-leg census: one entry per leg or fork that is not GREEN,
/// with its status, proof cite and note. A display for review — it
/// never gates and never changes an exit code. `check-exists` mirrors
/// the verdict's `candidate-staleness` info: the leg is not GREEN while
/// the check its own `proof.web` names resolves in the spec.
fn open_census(spines: &[(PathBuf, Spine)], outcome: &verdict::Outcome) -> Vec<Value> {
    let stale: Vec<&str> = outcome
        .info
        .iter()
        .filter(|i| i["kind"] == "candidate-staleness")
        .filter_map(|i| i["leg"].as_str())
        .collect();
    let mut rows = Vec::new();
    for (_, s) in spines {
        for leg in &s.legs {
            let who = format!("{}/{}", s.journey, leg.id);
            if leg.status != "GREEN" {
                rows.push(open_row(
                    &who,
                    &leg.status,
                    leg.proof.web.as_deref(),
                    leg.note.as_deref(),
                    stale.contains(&who.as_str()),
                    &leg.waits_on,
                ));
            }
            for f in &leg.forks {
                let who = format!("{}/{}#{}", s.journey, leg.id, f.id);
                if f.status != "GREEN" {
                    rows.push(open_row(
                        &who,
                        &f.status,
                        f.proof.web.as_deref(),
                        f.note.as_deref(),
                        stale.contains(&who.as_str()),
                        &f.waits_on,
                    ));
                }
            }
        }
    }
    rows
}

fn open_row(
    who: &str,
    status: &str,
    proof: Option<&str>,
    note: Option<&str>,
    exists: bool,
    waits: &[String],
) -> Value {
    json!({
        "leg": who,
        "status": status,
        "proof": proof,
        "check-exists": exists,
        "waits_on": waits,
        "note": note.unwrap_or(""),
    })
}

fn verb_status(root: &Path, profile: &Profile, args: &Args) -> i32 {
    let spines = match load_spines(root, profile, &args.positionals) {
        Ok(s) => s,
        Err((code, msg)) => return err(msg, code),
    };
    let outcome = match run_verdict(root, profile, &spines) {
        Ok(o) => o,
        Err((code, msg)) => return err(msg, code),
    };
    if args.json {
        let mut v = outcome_json(&outcome, spines.len());
        v["open"] = Value::Array(open_census(&spines, &outcome));
        println!("{v}");
        return outcome.exit_code;
    }
    if args.open {
        let rows = open_census(&spines, &outcome);
        println!("open legs: {} — display only, notes state conditions, the walk decides", rows.len());
        for r in &rows {
            let proof = match r["proof"].as_str() {
                Some(c) if r["check-exists"] == true => format!("proof {c} · check-exists"),
                Some(c) => format!("proof {c}"),
                None => "proof —".into(),
            };
            let waits: Vec<&str> = r["waits_on"]
                .as_array()
                .map(|a| a.iter().filter_map(|t| t.as_str()).collect())
                .unwrap_or_default();
            let waits_col = if waits.is_empty() {
                "waits —".to_string()
            } else {
                format!("waits[{}]", waits.join("; "))
            };
            println!(
                "{:<14} {:<7} {:<32} {:<40} {}",
                r["leg"].as_str().unwrap_or(""),
                r["status"].as_str().unwrap_or(""),
                proof,
                waits_col,
                r["note"].as_str().unwrap_or("")
            );
        }
        println!();
        println!(
            "status: exit {} ({})",
            outcome.exit_code,
            class_name(outcome.exit_code)
        );
        return outcome.exit_code;
    }
    if args.md {
        println!("| gate | pass | baselined | fail | of |");
        println!("|---|---:|---:|---:|---:|");
        for g in &outcome.gates {
            println!(
                "| {} | {} | {} | {} | {} |",
                g.name, g.pass, g.baselined, g.fail, g.of
            );
        }
        println!();
        println!(
            "spines: {} · gaps: {} · resolved: {} · info: {} · exit {} ({})",
            spines.len(),
            outcome.gaps.len(),
            outcome.resolved.len(),
            outcome.info.len(),
            outcome.exit_code,
            class_name(outcome.exit_code),
        );
        for g in &outcome.gaps {
            println!("- GAP `{}` {}", g["gate"], g["target"]);
        }
        return outcome.exit_code;
    }
    // No flag: the counts readout.
    for (_, s) in &spines {
        let green = s.legs.iter().filter(|l| l.status == "GREEN").count();
        let forks: usize = s.legs.iter().map(|l| l.forks.len()).sum();
        println!(
            "{:<8} {:>2} legs · {} GREEN · {} forks · {}",
            s.journey,
            s.legs.len(),
            green,
            forks,
            s.title
        );
    }
    println!();
    print_gate_table(&outcome);
    print_gaps(&outcome);
    println!(
        "status: exit {} ({})",
        outcome.exit_code, class_name(outcome.exit_code)
    );
    outcome.exit_code
}

fn verb_check(root: &Path, profile: &Profile) -> i32 {
    let spines = match load_spines(root, profile, &[]) {
        Ok(s) => s,
        Err((code, msg)) => return err(msg, code),
    };
    if let Err(e) = render::renderable(&spines) {
        return err(e, exit::CONSTRAINT);
    }
    let target = root.join(profile.doc_target());
    if !target.exists() {
        println!("{}: stale — the declared doc does not exist yet", target.display());
        return exit::STALE;
    }
    if !render::target_is_generated(&target) {
        return err(
            format!(
                "{}: existing target carries no {} marker — refusing to call it stale or fresh; generate takes over only authored-free targets",
                target.display(),
                render::GENERATED_MARKER,
            ),
            exit::OPERATIONAL,
        );
    }
    if render::is_fresh(root, profile, &spines) {
        println!("{}: fresh", target.display());
        exit::GREEN
    } else {
        println!("{}: stale — re-run generate", target.display());
        exit::STALE
    }
}

fn verb_generate(root: &Path, profile: &Profile, args: &Args) -> i32 {
    if args.census {
        return match &profile.census_command {
            None => err(
                "profile declares no census-command — regenerating the census needs a declared proof command",
                exit::OPERATIONAL,
            ),
            Some(cmd) => {
                let status = Command::new("sh")
                    .arg("-c")
                    .arg(cmd)
                    .current_dir(root)
                    .status();
                match status {
                    Err(e) => err(format!("census-command failed to start: {e}"), exit::OPERATIONAL),
                    Ok(s) if s.success() => {
                        println!("census-command ran; the census artifact is regenerated");
                        exit::GREEN
                    }
                    Ok(s) => err(
                        format!("census-command exited {}", s.code().unwrap_or(-1)),
                        exit::OPERATIONAL,
                    ),
                }
            }
        };
    }
    let spines = match load_spines(root, profile, &[]) {
        Ok(s) => s,
        Err((code, msg)) => return err(msg, code),
    };
    if let Err(e) = render::renderable(&spines) {
        return err(e, exit::CONSTRAINT);
    }
    let target = root.join(profile.doc_target());
    if !render::target_is_generated(&target) {
        return err(
            format!(
                "{}: existing target carries no {} marker — refusing to overwrite authored content",
                target.display(),
                render::GENERATED_MARKER,
            ),
            exit::OPERATIONAL,
        );
    }
    let text = render::render_doc(root, profile, &spines);
    if let Some(dir) = target.parent()
        && let Err(e) = std::fs::create_dir_all(dir)
    {
        return err(format!("cannot create doc dir: {e}"), exit::OPERATIONAL);
    }
    match std::fs::write(&target, text) {
        Ok(()) => {
            println!("wrote {}", target.display());
            exit::GREEN
        }
        Err(e) => err(format!("{}: cannot write doc: {e}", target.display()), exit::OPERATIONAL),
    }
}

/// Door ids the committed census declares that no spine cites.
fn uncited_doors(root: &Path, profile: &Profile, spines: &[(PathBuf, Spine)]) -> Option<Vec<String>> {
    let path = root.join(profile.census.as_ref()?);
    let census = Census::load(&path).ok()?;
    let mut cited: Vec<String> = Vec::new();
    for (_, s) in spines {
        for (_, _, _, _, doors, _) in spine::walk(s) {
            for d in doors {
                if let Some(door) = census.resolve_cite(&d) {
                    cited.push(door.id.clone());
                }
            }
        }
    }
    Some(
        census
            .ids()
            .into_iter()
            .filter(|id| !cited.iter().any(|c| c == id))
            .map(String::from)
            .collect(),
    )
}

fn verb_scaffold(root: &Path, profile: &Profile, args: &Args) -> i32 {
    let [letter] = args.positionals.as_slice() else {
        return err(
            "scaffold needs one letter (UJ-A..UJ-Z)",
            exit::USAGE,
        );
    };
    let path = match scaffold::draft(root, profile, letter) {
        Ok(p) => p,
        Err((code, msg)) => return err(msg, code),
    };
    println!("wrote {}", path.display());
    // The uncited-door report reads whatever is loadable; a broken spine
    // set never blocks the draft the caller just asked for.
    let spines = load_spines(root, profile, &[]).unwrap_or_default();
    match uncited_doors(root, profile, &spines) {
        None => println!("census unreadable or undeclared — no uncited-door report"),
        Some(ids) => {
            println!("census doors no spine cites yet: {}", ids.len());
            for id in ids {
                println!("  {id}");
            }
        }
    }
    exit::GREEN
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn parse_flags_and_verbs() {
        let a = parse(&argv("verify a.yaml --config c.toml")).unwrap();
        assert_eq!(a.verb, "verify");
        assert_eq!(a.positionals, ["a.yaml"]);
        assert_eq!(a.config, Some(PathBuf::from("c.toml")));
        assert!(parse(&argv("status --json --md")).is_err());
        assert!(parse(&argv("verify --bogus")).is_err());
        assert!(parse(&argv("--config")).is_err());
        assert!(parse(&argv("")).is_err());
        assert_eq!(parse(&argv("-V")).unwrap().verb, "version");
    }

    #[test]
    fn usage_errors_exit_64() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(run(&argv("bogus-verb"), dir.path()), exit::USAGE);
        assert_eq!(run(&argv("help bogus-topic"), dir.path()), exit::USAGE);
        assert_eq!(run(&argv(""), dir.path()), exit::USAGE);
        assert_eq!(run(&argv("scaffold"), dir.path()), exit::USAGE);
        assert_eq!(run(&argv("scaffold a b"), dir.path()), exit::USAGE);
        assert_eq!(run(&argv("version"), dir.path()), exit::GREEN);
    }

    fn fixture(root: &Path) -> Profile {
        // A minimal complete surface: one spine, one census door, zero gaps.
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
        Profile::load(&root.join("journeyspec.toml")).unwrap()
    }

    #[test]
    fn verify_status_and_ratchet_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fixture(root);
        assert_eq!(run(&argv("verify"), root), exit::GREEN);
        assert_eq!(run(&argv("status --json"), root), exit::GREEN);
        assert_eq!(run(&argv("status --md"), root), exit::GREEN);
        // Drop the cite: live gap, no baseline entry — constraint (2).
        std::fs::write(
            root.join("journeys/uj-a.yaml"),
            "journey: UJ-A\ntitle: Fixture\nprovenance: fixture\nworlds:\n  clean-slate: { seed: 0, description: Fixture world. }\nlegs:\n  - id: \"1\"\n    given: The binary is built.\n    when:\n      ui: Open the page\n      doors: []\n    then:\n      - The page renders.\n    ledger: []\n    proof: { web: \"uj-a#1\", tauri: null, android: null }\n    edges: []\n    forks: []\n    world: clean-slate\n    status: GREEN\n",
        )
        .unwrap();
        assert_eq!(run(&argv("verify"), root), exit::CONSTRAINT);
        // Baseline it: green again.
        std::fs::write(
            root.join("journeys/baseline.tsv"),
            "door-census\tGET /db\tfixture audit\t2026-10-09\n",
        )
        .unwrap();
        assert_eq!(run(&argv("verify"), root), exit::GREEN);
        // Restore the cite: the entry now matches nothing — stale drift (1).
        fixture(root);
        assert_eq!(run(&argv("verify"), root), exit::STALE);
    }

    #[test]
    fn check_refuses_authored_target_and_reports_staleness() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let profile = fixture(root);
        // No doc yet: stale (1).
        assert_eq!(run(&argv("check"), root), exit::STALE);
        // An authored file at the target: refusal (70), never a verdict.
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::write(root.join("docs/j.md"), "# hand written\n").unwrap();
        assert_eq!(run(&argv("check"), root), exit::OPERATIONAL);
        assert_eq!(run(&argv("generate"), root), exit::OPERATIONAL);
        // generate --census without a declaration: refusal (70).
        assert_eq!(run(&argv("generate --census"), root), exit::OPERATIONAL);
        // Remove the authored file; generate writes, check goes fresh.
        std::fs::remove_file(root.join("docs/j.md")).unwrap();
        assert_eq!(run(&argv("generate"), root), exit::GREEN);
        assert_eq!(run(&argv("check"), root), exit::GREEN);
        // The written doc is deterministic: generate is idempotent.
        assert_eq!(run(&argv("generate"), root), exit::GREEN);
        assert!(render::is_fresh(root, &profile, &load_spines(root, &profile, &[]).unwrap()));
    }

    #[test]
    fn grammar_break_is_constraint_and_missing_artifact_is_operational() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fixture(root);
        std::fs::write(root.join("journeys/uj-b.yaml"), "journey: BAD\n").unwrap();
        assert_eq!(run(&argv("verify"), root), exit::CONSTRAINT);
        std::fs::remove_file(root.join("journeys/uj-b.yaml")).unwrap();
        std::fs::remove_file(root.join("census.tsv")).unwrap();
        assert_eq!(run(&argv("verify"), root), exit::OPERATIONAL);
    }

    #[test]
    fn scaffold_drafts_and_reports_uncited_doors() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fixture(root);
        std::fs::write(
            root.join("journeys/uj-a.yaml"),
            "journey: UJ-A\ntitle: Fixture\nprovenance: fixture\nworlds:\n  clean-slate: { seed: 0, description: Fixture world. }\nlegs:\n  - id: \"1\"\n    given: The binary is built.\n    when:\n      ui: Open the page\n      doors: []\n    then:\n      - The page renders.\n    ledger: []\n    proof: { web: \"uj-a#1\", tauri: null, android: null }\n    edges: []\n    forks: []\n    world: clean-slate\n    status: RED\n",
        )
        .unwrap();
        assert_eq!(run(&argv("scaffold k"), root), exit::GREEN);
        assert!(root.join("journeys/uj-k.yaml").is_file());
        // The drafted spine has no cites, so the door stays uncited: 2.
        assert_eq!(run(&argv("verify"), root), exit::CONSTRAINT);
        assert_eq!(run(&argv("scaffold k"), root), exit::OPERATIONAL);
        assert_eq!(run(&argv("scaffold 7"), root), exit::USAGE);
    }
}
