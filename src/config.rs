//! config — the project profile and its discovery.
//!
//! The profile is one TOML file, `journeyspec.toml`. It declares every path
//! the verdicts read and write; the payload carries no project paths of its
//! own. Discovery order (never a CWD guess):
//!
//! 1. `--config PATH` — an explicit file wins over everything.
//! 2. `./journeyspec.toml` — the current directory.
//! 3. A walk up the ancestor directories to the filesystem root (the repo
//!    root is where the profile of a repo lives).
//!
//! Nothing found is an OPERATIONAL (70) refusal, not a silent default root.
//! Every declared path resolves against the profile file's directory: that
//! directory IS the root, so any subdirectory sees the same surface.
//!
//! Shape (all keys optional unless noted; unknown keys are refused):
//!
//! ```toml
//! journeys-dir = "journeys"          # directory of spine YAML files
//! schema = "journeys/spine.schema.json" # normative grammar shape (default: journeys-dir/spine.schema.json)
//! census = "census/doors.tsv"        # committed door census artifact (required once doors are cited)
//! census-may-be-zero = false         # only a spec that declares zero passes an empty census
//! census-command = "..."             # proof command `generate --census` may run; nothing else regenerates the census
//! baseline = "journeys/baseline.tsv" # ratchet store (default: journeys-dir/baseline.tsv)
//! ledger = "docs/ledger.md"          # ledger-row source document
//! proof-glob = "tests/{head}.spec.mjs" # proof-check sources; {head} expands to the cite head
//! edge-census = "docs/schema.md"     # committed schema-edge census artifact
//! edge-allow-list = "scripts/edges.sh" # dashed (hidden) edge allow-list source
//! docs-dir = "specs/journeys/docs"   # generated-artifact home directory
//! doc-file = "journeys.md"           # generated doc file name inside docs-dir
//! doc-title = "18 — Journeys"        # generated doc H1 text
//! doc-note = "..."                   # optional extra banner line (the artifact's own history note)
//! preamble = "journeys/preamble.md"  # hand prose that rides into the generated doc verbatim
//! ```

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The profile file name discovery looks for.
pub const PROFILE_FILE: &str = "journeyspec.toml";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Profile {
    /// Directory holding the spine YAML files (`.yaml`; other extensions
    /// are ignored).
    #[serde(default = "default_journeys_dir")]
    pub journeys_dir: PathBuf,
    /// The normative grammar file. The tool enforces the grammar
    /// structurally; this path is the cite the surfaces render.
    #[serde(default)]
    pub schema: Option<PathBuf>,
    /// The committed door census artifact (TSV; see `census.rs`). Required
    /// once any spine cites a door or coverage is judged.
    #[serde(default)]
    pub census: Option<PathBuf>,
    /// Only a spec that declares zero may pass an empty census (the
    /// zero-census floor; see the Census law in the family doctrine).
    #[serde(default)]
    pub census_may_be_zero: bool,
    /// The proof command `generate --census` may run to regenerate the
    /// census artifact. No other path regenerates it, and no verdict ever
    /// execs it — gates read the committed file.
    #[serde(default)]
    pub census_command: Option<String>,
    /// The ratchet baseline store (see `baseline.rs`).
    #[serde(default)]
    pub baseline: Option<PathBuf>,
    /// The ledger source document every ledger cite resolves against.
    #[serde(default)]
    pub ledger: Option<PathBuf>,
    /// Proof-check sources. `{head}` expands to a cite's head; a proof
    /// cite resolves when the file names `check <tail>`.
    #[serde(default)]
    pub proof_glob: Option<String>,
    /// The committed schema-edge census artifact (markdown).
    #[serde(default)]
    pub edge_census: Option<PathBuf>,
    /// The dashed-edge allow-list source consulted for edge CLAIMS only.
    #[serde(default)]
    pub edge_allow_list: Option<PathBuf>,
    /// The generated-artifact home directory.
    #[serde(default = "default_docs_dir")]
    pub docs_dir: PathBuf,
    /// The generated doc file name inside `docs-dir`.
    #[serde(default = "default_doc_file")]
    pub doc_file: String,
    /// The generated doc H1 text (rendered, never hardcoded in the tool).
    #[serde(default)]
    pub doc_title: Option<String>,
    /// One optional banner line: the artifact's own history note.
    #[serde(default)]
    pub doc_note: Option<String>,
    /// Hand prose no spine field carries; it rides into the doc verbatim.
    #[serde(default)]
    pub preamble: Option<PathBuf>,
}

fn default_journeys_dir() -> PathBuf {
    PathBuf::from("journeys")
}

fn default_docs_dir() -> PathBuf {
    PathBuf::from("specs/journeys/docs")
}

fn default_doc_file() -> String {
    "journeys.md".to_string()
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            journeys_dir: default_journeys_dir(),
            schema: None,
            census: None,
            census_may_be_zero: false,
            census_command: None,
            baseline: None,
            ledger: None,
            proof_glob: None,
            edge_census: None,
            edge_allow_list: None,
            docs_dir: default_docs_dir(),
            doc_file: default_doc_file(),
            doc_title: None,
            doc_note: None,
            preamble: None,
        }
    }
}

impl Profile {
    /// Parse a profile file. A malformed file is an OPERATIONAL refusal:
    /// the config is the precondition the verbs ask about.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("{}: cannot read profile: {e}", path.display()))?;
        toml::from_str(&text).map_err(|e| format!("{}: profile invalid: {e}", path.display()))
    }

    /// The root this profile declares: its own file's directory.
    pub fn root_of(profile_path: &Path) -> PathBuf {
        profile_path.parent().map_or_else(|| PathBuf::from("."), Path::to_path_buf)
    }

    /// The schema cite to render (the declared path, or the default home).
    pub fn schema_display(&self) -> String {
        self.schema
            .clone()
            .unwrap_or_else(|| self.journeys_dir.join("spine.schema.json"))
            .display()
            .to_string()
    }

    /// The baseline store path (the declared path, or journeys-dir default).
    pub fn baseline_path(&self) -> PathBuf {
        self.baseline
            .clone()
            .unwrap_or_else(|| self.journeys_dir.join("baseline.tsv"))
    }

    /// The generated-doc target: docs-dir joined with doc-file.
    pub fn doc_target(&self) -> PathBuf {
        self.docs_dir.join(&self.doc_file)
    }
}

/// A discovery or profile-load failure: always OPERATIONAL (70).
#[derive(Debug)]
pub struct DiscoverError(pub String);

/// Resolve the profile path: `--config` wins, then `./journeyspec.toml`,
/// then the ancestor walk. Never a CWD guess; nothing found is loud.
pub fn discover(explicit: Option<&Path>, cwd: &Path) -> Result<PathBuf, DiscoverError> {
    if let Some(p) = explicit {
        if p.is_file() {
            return Ok(p.to_path_buf());
        }
        return Err(DiscoverError(format!(
            "--config {} is not a readable file",
            p.display()
        )));
    }
    let here = cwd.join(PROFILE_FILE);
    if here.is_file() {
        return Ok(here);
    }
    let mut cursor = cwd.to_path_buf();
    while let Some(parent) = cursor.parent() {
        let cand = parent.join(PROFILE_FILE);
        if cand.is_file() {
            return Ok(cand);
        }
        cursor = parent.to_path_buf();
    }
    Err(DiscoverError(format!(
        "no {PROFILE_FILE} found from {} upward — run at the repo root or pass --config PATH",
        cwd.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_profile() {
        let text = r#"
journeys-dir = "j"
schema = "j/s.json"
census = "c/doors.tsv"
census-may-be-zero = true
census-command = "bash regen.sh"
baseline = "j/b.tsv"
ledger = "d/ledger.md"
proof-glob = "t/{head}.spec.mjs"
edge-census = "d/schema.md"
edge-allow-list = "s/edges.sh"
docs-dir = "gen"
doc-file = "x.md"
doc-title = "T"
doc-note = "N"
preamble = "j/p.md"
"#;
        let p: Profile = toml::from_str(text).unwrap();
        assert_eq!(p.journeys_dir, PathBuf::from("j"));
        assert!(p.census_may_be_zero);
        assert_eq!(p.doc_file, "x.md");
        assert_eq!(p.proof_glob.as_deref(), Some("t/{head}.spec.mjs"));
    }

    #[test]
    fn fields_default() {
        let p: Profile = toml::from_str("").unwrap();
        assert_eq!(p, Profile::default());
        assert_eq!(p.journeys_dir, PathBuf::from("journeys"));
        assert_eq!(p.baseline_path(), PathBuf::from("journeys/baseline.tsv"));
        assert_eq!(p.schema_display(), "journeys/spine.schema.json");
        assert_eq!(
            p.doc_target(),
            PathBuf::from("specs/journeys/docs").join("journeys.md")
        );
    }

    #[test]
    fn unknown_profile_key_is_refused() {
        assert!(toml::from_str::<Profile>("bogus = 1").is_err());
    }

    #[test]
    fn discovery_prefers_explicit_then_cwd_then_walk() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let deep = root.join("a/b");
        std::fs::create_dir_all(&deep).unwrap();
        // Nothing yet: discovery fails loud.
        assert!(discover(None, &deep).is_err());
        // An explicit path that is not a file fails loud.
        assert!(discover(Some(&root.join("nope.toml")), &deep).is_err());
        // The ancestor walk finds the root profile from a subdirectory.
        std::fs::write(root.join(PROFILE_FILE), "").unwrap();
        assert_eq!(discover(None, &deep).unwrap(), root.join(PROFILE_FILE));
        // ./journeyspec.toml wins over the walk.
        let cwd_profile = deep.join(PROFILE_FILE);
        std::fs::write(&cwd_profile, "doc-file = \"here.md\"").unwrap();
        assert_eq!(discover(None, &deep).unwrap(), cwd_profile);
        // --config wins over both.
        assert_eq!(
            discover(Some(&cwd_profile), root).unwrap(),
            cwd_profile
        );
    }

    #[test]
    fn root_is_the_profile_directory() {
        assert_eq!(
            Profile::root_of(Path::new("/r/x/journeyspec.toml")),
            PathBuf::from("/r/x")
        );
    }
}
