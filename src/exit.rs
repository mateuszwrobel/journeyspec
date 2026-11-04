//! exit — the five verdict classes of the family exit taxonomy.
//!
//! Every code here is a CLASS, not an error number: a harness must be able
//! to tell "a constraint is broken" from "an artifact is stale" from "a
//! declared file is missing". The split is pinned per verb in the tool's
//! capability matrix and enumerated in the `exit-codes` help topic — the
//! help topic and these constants are diffed both ways by a guard test, so
//! the manual cannot lie about the surface.

/// The verdict ran and found nothing.
pub const GREEN: i32 = 0;
/// A generated artifact differs from what its source declares.
pub const STALE: i32 = 1;
/// A declared constraint is broken.
pub const CONSTRAINT: i32 = 2;
/// Bad arguments, unknown verb, unknown flag, `help` typo. Universal.
pub const USAGE: i32 = 64;
/// A precondition is missing: no profile, unreadable declared artifact,
/// no spines to judge. Fail closed — never answered as drift, never as
/// constraint.
pub const OPERATIONAL: i32 = 70;
