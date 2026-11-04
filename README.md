# journeyspec

Journey spine verdict CLI. It reads declarative journey spines (YAML) plus
the committed artifacts they cite — a door census, a ledger document, a
schema-edge census, a ratchet baseline — and answers one question in exit
codes: *does what the spines declare still match what the repository
commits?*

Zero LLM in the verdict path: every gate is a deterministic check with a
named gap and a stable key.

## Build

```sh
cargo build --release            # binary: target/release/journeyspec
```

Rust stable (edition 2024). No runtime outside the binary's own
dependencies.

## Quick start

Put a `journeyspec.toml` at your repo root declaring the surface:

```toml
journeys-dir = "journeys"
census = "fixtures/doors.tsv"
ledger = "docs/ledger.md"
proof-glob = "tests/{head}.spec.mjs"
```

Then, from anywhere inside the repo:

```sh
journeyspec verify            # grammar + cites + coverage → exit code
journeyspec check             # generated doc matches a fresh render?
journeyspec generate          # render the generated doc
journeyspec scaffold R        # draft spine UJ-R and list uncited doors
journeyspec help commands     # the full manual
```

Discovery: `--config PATH`, then `./journeyspec.toml`, then the ancestor
walk. Every declared path resolves against the profile file's directory —
never the process CWD, never a guessed root.

## The exit taxonomy

| Code | Class | Meaning |
|-----:|-------|---------|
| 0 | green | the verdict ran and found nothing |
| 1 | stale-drift | a generated artifact or baseline entry differs from its source |
| 2 | constraint-violation | a declared constraint is broken |
| 64 | usage | bad arguments, unknown verb, unknown flag or topic |
| 70 | operational | a precondition is missing — the verdict did not run |

`70` is never reported as drift or constraint: a verdict that could not
run is not a verdict. `journeyspec help exit-codes` carries the full
contract.

## The two laws the verdicts enforce

**Census law.** Coverage is judged against a COMMITTED artifact. The gate
never runs a generator: an empty census fails unless the profile declares
zero, and a declared-but-unreadable artifact is `70`, never a silent zero.

**Ratchet law.** A live gap matching a baseline entry is BASELINED (info).
A live gap matching nothing is NEW (exit 2). A baseline entry matching no
live gap is RESOLVED (exit 1 — shrink the baseline). Entries are keyed by
stable keys — door ids, ledger ids, edge keys, prose addresses — never by
row positions, so a regeneration cannot silently repoint one.

## Documentation

- `docs/capability-matrix.md` — every command × exit code × the golden test
  that pins it.
- `journeyspec help commands|exit-codes|configuration|diagnostics` — the
  manual owned by the binary itself.
