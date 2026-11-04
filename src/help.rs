//! help — the manual surface. Four topics; an unknown topic is USAGE (64).
//!
//! The `exit-codes` topic and `exit.rs` are diffed both ways by a guard
//! test below: the manual cannot lie about the exit surface, and the exit
//! surface cannot grow without the manual.

use crate::exit;

/// Every topic name, in `help` listing order.
pub const TOPICS: [&str; 4] = ["commands", "exit-codes", "configuration", "diagnostics"];

/// The one-line summary `help` and the usage error print.
pub const SUMMARY: &str = "journeyspec — journey spine verdict CLI\n\
 verbs: verify | check | status | generate | scaffold | help | version\n\
 flags: --config PATH | --json | --md | --open (with status) | --census (with generate)\n\
 help <topic> for: commands, exit-codes, configuration, diagnostics";

/// The text of one topic; None when the topic does not exist.
pub fn topic(name: &str) -> Option<String> {
    let text: String = match name {
        "commands" => "\
journeyspec verbs

verify [SPINE...]  Grammar, cite resolution and census coverage over the
    spines the profile declares (or over the named SPINE files). Reports a
    gate table, every gap with its stable key, and every ratchet entry that
    resolved. Exit: 0, 1 (ratchet shrink), 2 (gap or grammar break), 70.

check              Compare the generated doc with a fresh render, byte for
    byte. Writes nothing. Exit: 0 fresh, 1 stale, 2 unrenderable spine,
    70 (no profile, unreadable source, existing target without the
    GENERATED marker).

status [--json|--md|--open]  Print the verdict readout: counts per gate,
    per journey, and the ratchet state. Same verdict as `verify`, same
    exit codes; --json and --md choose the machine and document shapes.
    --open adds the open-leg census: every leg and fork that is not
    GREEN, with its note and its waits_on tokens, marked `check-exists`
    when the check its own proof.web names already resolves. A display
    for review — the census rows never gate on their own and never add
    an exit code; the stale-open gate inside `verify` judges the same
    facts (see diagnostics). --json carries the same rows under the
    key `open`.

generate [--census]  Render the generated doc over the declared target.
    Refuses an existing target without the GENERATED marker (70). With
    --census: run the profile's census-command and write nothing else;
    without the declaration the flag is refused (70). Exit: 0, 2, 70.

scaffold LETTER    Draft a grammar-valid one-leg spine UJ-<LETTER> into the
    declared spine directory and report the census door ids no spine cites
    yet. Refuses an existing target (70). Exit: 0, 64, 70.

help [TOPIC]       Print this manual. Unknown topic: usage (64).
version            Print the version. Exit: 0.

Flags: --config PATH overrides profile discovery; --json, --md and --open
choose output shapes where documented. A spine file that fails the grammar stops
verify with its line and field named".to_string(),

        "exit-codes" => format!(
            "\
journeyspec exit codes — the family taxonomy, one class per code

{}  green                 the verdict ran and found nothing
{}  stale-drift           a generated artifact differs from what its
                          source declares, or a baseline entry matches no
                          live gap (shrink the baseline)
{}  constraint-violation  a declared constraint is broken: grammar break,
                          unresolved cite, uncited census door, live gap
                          with no baseline entry, zero-census floor
{}  usage                 bad arguments, unknown verb, unknown flag,
                          unknown help topic
{}  operational           a precondition is missing: no profile, an
                          unreadable declared artifact, an existing target
                          without the GENERATED marker, nothing to judge.
                          Fail closed — never reported as a verdict.",
            exit::GREEN,
            exit::STALE,
            exit::CONSTRAINT,
            exit::USAGE,
            exit::OPERATIONAL,
        ),

        "configuration" => "\
journeyspec.toml — the project profile

One TOML file declares every path the verdicts read and write. The payload
carries no project paths; the profile is the only place a path exists.
Unknown keys are refused (64-class grammar error on the profile: 70).

Discovery order (never a current-directory guess):
  1. --config PATH
  2. ./journeyspec.toml
  3. the ancestor walk up to the filesystem root
Nothing found is operational (70), never a silent default root. Every
declared path resolves against the profile file's directory — that
directory is the root.

Keys (all optional; defaults in parentheses):
  journeys-dir        directory of spine YAML files (journeys)
  schema              grammar file the banner cites (journeys-dir/spine.schema.json)
  census              committed door census TSV (none; required once doors are cited)
  census-may-be-zero  only a spec that declares zero passes an empty census (false)
  census-command      proof command generate --census may run; no verdict execs it (none)
  baseline            ratchet store TSV (journeys-dir/baseline.tsv)
  ledger              ledger source document (none; required once ledger rows are cited)
  proof-glob          proof-check sources; {head} expands to the cite head (none)
  edge-census         committed schema-edge census markdown (none)
  edge-allow-list     dashed-edge allow-list source (none)
  docs-dir            generated-artifact directory (specs/journeys/docs)
  doc-file            generated doc file name (journeys.md)
  doc-title           generated doc H1 text (Journeys)
  doc-note            one extra banner line (none)
  preamble            hand prose that rides into the doc verbatim (none)".to_string(),

        "diagnostics" => "\
journeyspec gates and their gaps

cite-resolution      a door cite does not name a census door, a ledger cite
    names no ledger id, or a proof cite names no check in the declared
    proof sources. Door cites are rt:NN addresses; they resolve to the row's
    stable door id (first two columns joined by a space).
verdict-freshness    a GREEN leg names no existing check.
door-census          a committed census door is cited by no leg. Coverage
    is judged against the committed artifact only; the gate never runs a
    census command.
ledger-homing        a ledger id is cited by no leg and carries no
    pile/parked marker.
composition-census   a journey over the grouping threshold has an ungrouped
    leg.
forks-budget         a leg carries more forks than the proposed budget.
edge-cites           an edge claim resolves against no committed edge, or
    a solid edge is cited by no leg.
asd-lint             deterministic ASD prose lint over when.ui, given and
    then (first-word imperative, sentence length, terminal punctuation,
    banned words).
stale-open           an open (non-GREEN) leg or fork declares no waits_on
     token (missing-waits-on); a mile it waits on now resolves in the
     declared proof source (unblock-resolved — the wait is over: walk the
     unit or re-declare it); or a PARTIAL unit whose own proof.web check
     exists while it waits on no unresolved mile (check-exists). A mile
     token reuses the proof-cite grammar and resolves against the
     declared proof-glob — resolution means the named check exists; the
     gate never claims it passed. A dependency token (CAND-<LETTER>
     <digits>) is a declaration only: it can never resolve and never
     defers check-exists. A GREEN unit declaring waits_on is a grammar
     break. Targets carry the kind: UNIT.missing-waits-on,
     UNIT.unblock-resolved, UNIT.check-exists.
zero-census floor    an empty census fails unless the profile declares
    zero. The floor is never ratchetable.

ratchet law          a live gap matching a baseline entry is BASELINED
    (info). A live gap matching nothing is NEW (constraint, exit 2). A
    baseline entry matching no live gap is RESOLVED (stale, exit 1 —
    shrink the baseline). Baseline targets are stable keys, never
    positional addresses.".to_string(),

        _ => return None,
    };
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_topic_is_none() {
        assert!(topic("bogus").is_none());
        for t in TOPICS {
            assert!(topic(t).is_some(), "{t} must have text");
        }
    }

    /// Both-ways diff: every exit constant appears in the exit-codes
    /// topic, and every code the topic lists is a real constant.
    #[test]
    fn exit_codes_topic_matches_the_constants() {
        let text = topic("exit-codes").unwrap();
        for code in [
            exit::GREEN,
            exit::STALE,
            exit::CONSTRAINT,
            exit::USAGE,
            exit::OPERATIONAL,
        ] {
            assert!(
                text.contains(&code.to_string()),
                "constant {code} missing from the exit-codes topic"
            );
        }
        for word in text.split_whitespace() {
            if let Ok(n) = word.parse::<i32>() {
                assert!(
                    [
                        exit::GREEN,
                        exit::STALE,
                        exit::CONSTRAINT,
                        exit::USAGE,
                        exit::OPERATIONAL,
                    ]
                    .contains(&n),
                    "topic lists {n} which is no constant"
                );
            }
        }
    }

    #[test]
    fn configuration_topic_lists_every_profile_key() {
        // The keys the config parser accepts must appear in the manual.
        let text = topic("configuration").unwrap();
        for key in [
            "journeys-dir",
            "schema",
            "census",
            "census-may-be-zero",
            "census-command",
            "baseline",
            "ledger",
            "proof-glob",
            "edge-census",
            "edge-allow-list",
            "docs-dir",
            "doc-file",
            "doc-title",
            "doc-note",
            "preamble",
        ] {
            assert!(text.contains(key), "key {key} missing from the manual");
        }
    }
}
