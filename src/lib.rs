//! lib — journeyspec: the journey spine verdict CLI.
//!
//! One profile (`journeyspec.toml`) declares the whole surface: the spine
//! directory, the spine grammar, the committed door census, the ratchet
//! baseline, the proof-check sources, and the generated-doc target. Every
//! path resolves against the profile file's directory — never the process
//! CWD, never a guessed root (the family evidence-root law).
//!
//! Verdict verbs: `verify` (grammar + cite resolution + census coverage),
//! `check` (generated-doc freshness), `status` (verdict readout),
//! `generate` (render the generated doc), `scaffold` (draft a spine), plus
//! `help` and `version`. The exit surface is the family taxonomy: 0 green,
//! 1 stale-drift, 2 constraint-violation, 64 usage, 70 operational.

pub mod baseline;
pub mod census;
pub mod cli;
pub mod config;
pub mod exit;
pub mod help;
pub mod render;
pub mod scaffold;
pub mod spine;
pub mod verdict;
