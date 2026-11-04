# journeyspec capability matrix

Every command, the exit code it produces, and the test that proves it. This
page is a map of the contract, not a restatement of behavior: the binary
owns each claim (`journeyspec help commands`, `journeyspec help
exit-codes`), and this table points at the golden test that pins it.

## Commands

| Command | What it does |
|---------|--------------|
| `verify [SPINE...]` | Grammar, cite resolution, census coverage and the stale-open gate; prints the gate table, the gaps, the resolved entries. |
| `check` | Compares the generated doc with a fresh render, byte for byte. Writes nothing. |
| `status [--json\|--md\|--open]` | The verdict readout in the machine or document shape; same verdict and codes as `verify`. `--open` adds the open-leg census: every leg and fork not GREEN, with its note, its `waits_on` tokens and a `check-exists` marker — the census rows are display only; the stale-open gate inside `verify` judges the same facts. |
| `generate [--census]` | Renders the generated doc over the declared target, or runs the declared census-command. |
| `scaffold LETTER` | Drafts grammar-valid spine `UJ-<LETTER>` and reports the census doors nothing cites yet. |
| `help [TOPIC]` | The built-in manual: `commands`, `exit-codes`, `configuration`, `diagnostics`. |
| `version` | Prints the version. |

## Command × exit code × tested-in

| Command | Exit | Meaning | Proven by |
|---------|-----:|---------|-----------|
| `verify` (clean) | 0 | Every cite resolves; every gap is baselined. | `verify_green_and_status_shapes_exit_zero` |
| `verify` (live gap) | 2 | A gap with no baseline entry breaks a declared constraint. | `live_gap_without_baseline_entry_exits_two` |
| `verify` (ratchet shrink) | 1 | A baseline entry matches no live gap — shrink the baseline. | `ratchet_round_trip_baselines_then_shrinks_to_one` |
| `verify` (zero floor) | 2 | An empty census passes only a profile that declares zero; never ratchetable. | `zero_census_floor_is_never_ratchetable` |
| `verify` (grammar) | 2 | A spine violates the grammar; the message names file and field. | `grammar_break_is_two_and_missing_artifact_is_seventy` |
| `verify` (missing artifact) | 70 | A declared artifact cannot be read — the verdict did not run. | `grammar_break_is_two_and_missing_artifact_is_seventy` |
| `verify` (stale-open: no wait declared) | 2 | An open (non-GREEN) unit declares no `waits_on` token. | `stale_open_missing_waits_on_is_red` |
| `verify` (stale-open: unblock resolved) | 2 | A `mile` the unit waits on now resolves in the declared proof source — walk it or re-declare. | `stale_open_unblock_resolved_is_red` |
| `verify` (stale-open: check-exists) | 2 | A PARTIAL unit's own check exists while it waits on no unresolved mile; a `dependency` alone cannot defer. | `stale_open_check_exists_needs_a_mile_wait_to_defer` |
| `verify` (honest open unit) | 0 | An open unit waiting on a landed-not-yet mile or a decision declaration is clean. | `stale_open_honest_partial_is_green` |
| `verify` (stale-open ratchet) | 0 → 1 | A kind-keyed grandfather row is BASELINED; fixing the unit shrinks the row (shrink law). | `stale_open_grandfathered_row_baselines_then_shrinks` |
| `verify` (mile without proof-glob) | 70 | A `mile` token has no declared resolution plane — the verdict did not run. | `stale_open_mile_without_declared_proof_glob_is_seventy` |
| `check` (stale) | 1 | The committed doc differs from a fresh render (or does not exist). | `check_and_generate_round_trip_with_refusals` |
| `check` (fresh) | 0 | The committed doc matches a fresh render byte for byte. | `check_and_generate_round_trip_with_refusals` |
| `check`/`generate` (authored target) | 70 | The target exists without the GENERATED marker — refusal, never a verdict. | `check_and_generate_round_trip_with_refusals` |
| `generate` | 0 | The doc is written; rendering is deterministic (a second run stays fresh). | `check_and_generate_round_trip_with_refusals` |
| `generate --census` (undeclared) | 70 | Regeneration needs a declared census-command; nothing else runs it. | `check_and_generate_round_trip_with_refusals` |
| `scaffold` (draft) | 0 | A grammar-valid spine is written; the uncited census doors are listed. | `scaffold_drafts_refuses_and_reports_uncited_doors` |
| `scaffold` (exists) | 70 | An existing spine is refused, never clobbered. | `scaffold_drafts_refuses_and_reports_uncited_doors` |
| `scaffold` (bad letter) | 64 | The journey id must be one letter `A..Z`. | `scaffold_drafts_refuses_and_reports_uncited_doors` |
| `status` shapes | 0 | `--json` is one machine object (with the `open` rows); `--md` is a document table; verdict unchanged. | `verify_green_and_status_shapes_exit_zero` |
| `status --open` | verdict | Every not-GREEN leg and fork is listed with note, `waits_on` tokens and the `check-exists` marker; the census never adds an exit code — the exit is the verdict's. | `status_open_lists_non_green_legs_and_marks_existing_checks` |
| `help <topic>` | 0 | A known topic prints its page; the index prints with no topic. | `usage_surface_exits_sixty_four` |
| `help <unknown>` | 64 | The named topic is not in the manual — a usage error, never a verdict code. | `usage_surface_exits_sixty_four` |
| (wrong command line) | 64 | Unknown verb, unknown flag, clashing flags, missing value. | `usage_surface_exits_sixty_four` |
| (no profile found) | 70 | No profile at `--config`, `./`, or up the walk — fail closed, no default root. | `discovery_walks_up_and_config_overrides` |

## Exit-code contract

Exit codes are defined once in `journeyspec::exit` (`GREEN`, `STALE`,
`CONSTRAINT`, `USAGE`, `OPERATIONAL`) and returned by `cli::run`. See
`journeyspec help exit-codes` for the same contract from the binary — the
help topic and the constants are diffed both ways by a guard test
(`help::tests::exit_codes_topic_matches_the_constants`), so the manual
cannot lie about the surface.

The split that matters: a broken constraint is `2`, an artifact that fell
behind its source is `1`, and a missing precondition is `70`. A harness
must never read "the tool could not run" as "the project is green".
