//! Agent onboarding: the hyp skill, embedded in the binary, installed as a
//! project skill for Claude Code and Codex.
//!
//! An installed file is the skill plus one marker line in its YAML front
//! matter: `# hyp-managed: version=<hyp version> sha256=<hash>`. The hash
//! covers the file without that line, so a local edit is detected without
//! knowing the content of earlier versions. A file without the marker was
//! not installed by hyp. Line endings are normalized before comparing, so a
//! CRLF conversion by a sync tool is not mistaken for an edit.
use crate::model::hash;
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// The skill as shipped with this binary; `hyp agents print` writes it verbatim.
pub const SKILL: &str = include_str!("../agents/hyp/SKILL.md");
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const MARKER: &str = "# hyp-managed:";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    Claude,
    Codex,
}
impl Agent {
    pub const ALL: [Agent; 2] = [Agent::Claude, Agent::Codex];
    /// Project-relative location of the skill. Claude Code reads project
    /// skills from `.claude/skills/<name>/SKILL.md`; Codex reads repository
    /// skills from `.agents/skills/<name>/SKILL.md` in the directory it is
    /// started in (and, only in a Git repository, its parents up to the
    /// root). Both use the same SKILL.md format. Sources: HYPO-0016 notes.
    pub fn skill_path(self) -> &'static str {
        match self {
            Agent::Claude => ".claude/skills/hyp/SKILL.md",
            Agent::Codex => ".agents/skills/hyp/SKILL.md",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    /// Write the current skill, creating it where missing.
    Install,
    /// Refresh only skills that are already installed.
    Update,
    /// Delete installed skills.
    Remove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Installed,
    Updated,
    /// A locally modified or foreign file overwritten because of `--force`.
    Replaced,
    Unchanged,
    Removed,
    NotInstalled,
}

#[derive(Debug, Serialize)]
pub struct Step {
    pub agent: Agent,
    pub path: String,
    pub action: Action,
    #[serde(skip)]
    target: PathBuf,
}

/// What is at a skill location now.
#[derive(Debug, PartialEq)]
enum Existing {
    Missing,
    /// Exactly what this binary would install.
    Current,
    /// Installed by hyp and unmodified, but from another version.
    Managed {
        version: String,
    },
    /// Installed by hyp, then edited (the hash no longer matches).
    Modified,
    /// No hyp marker, a symlink, or not UTF-8: not installed by hyp.
    Foreign,
}

/// The installed form of `skill`: the marker line goes last in the front
/// matter, which must open the file.
pub fn managed_file(version: &str, skill: &str) -> Result<String> {
    let end = front_matter_end(skill).context("the skill must start with YAML front matter")?;
    Ok(format!(
        "{}{MARKER} version={version} sha256={}\n{}",
        &skill[..end],
        hash(skill),
        &skill[end..]
    ))
}
/// Byte offset of the closing `---` line of the front matter.
fn front_matter_end(text: &str) -> Option<usize> {
    let rest = text.strip_prefix("---\n")?;
    rest.find("\n---\n").map(|i| "---\n".len() + i + 1)
}
/// The version the marker names and whether the rest of the file still
/// hashes to it; `None` without a marker line in the front matter.
fn parse_managed(text: &str) -> Option<(String, bool)> {
    let front_matter = &text[..front_matter_end(text)?];
    let start = front_matter.find(&format!("\n{MARKER}"))? + 1;
    let end = text[start..]
        .find('\n')
        .map_or(text.len(), |i| start + i + 1);
    let line = text[start..end].trim_end();
    let field = |key: &str| {
        line.split_whitespace()
            .find_map(|w| w.strip_prefix(key))
            .map(str::to_string)
    };
    let version = field("version=").unwrap_or_default();
    let intact =
        field("sha256=").is_some_and(|h| h == hash(format!("{}{}", &text[..start], &text[end..])));
    Some((version, intact))
}
fn inspect(path: &Path) -> Result<Existing> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Existing::Missing),
        Err(e) => return Err(e).with_context(|| format!("cannot inspect {}", path.display())),
    };
    if !meta.is_file() {
        return Ok(Existing::Foreign);
    }
    let bytes = fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    let Ok(text) = String::from_utf8(bytes) else {
        return Ok(Existing::Foreign);
    };
    let text = text.replace("\r\n", "\n");
    if text == managed_file(VERSION, SKILL)? {
        return Ok(Existing::Current);
    }
    Ok(match parse_managed(&text) {
        None => Existing::Foreign,
        Some((_, false)) => Existing::Modified,
        Some((version, true)) => Existing::Managed { version },
    })
}
/// `a` is a later release than `b`; unparsable versions never are.
fn newer(a: &str, b: &str) -> bool {
    let parse = |v: &str| -> Option<Vec<u64>> { v.split('.').map(|p| p.parse().ok()).collect() };
    matches!((parse(a), parse(b)), (Some(a), Some(b)) if a > b)
}

/// The first existing directory on `path` (relative to `root`) that is a
/// symlink or not a directory. Writing or deleting through it would reach
/// outside the project, e.g. into a dotfile manager's tree.
fn indirect_component(root: &Path, path: &str) -> Option<String> {
    let mut dir = root.to_path_buf();
    let parent = Path::new(path).parent()?;
    for component in parent.components() {
        dir.push(component);
        match fs::symlink_metadata(&dir) {
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => return Some(dir.strip_prefix(root).ok()?.display().to_string()),
            Err(_) => return None,
        }
    }
    None
}

/// Decide what `op` does for each agent under `root`, reading only. Fails,
/// naming every blocked file, if any file would be overwritten or deleted
/// that hyp did not install unmodified, or would be reached through a
/// symlink (unless `force`), so a failed plan writes nothing. `remove` never
/// deletes a file hyp did not install.
pub fn plan(root: &Path, agents: &[Agent], op: Operation, force: bool) -> Result<Vec<Step>> {
    let mut steps = Vec::new();
    // (reason, whether --force overrides it)
    let mut blocked: Vec<(String, bool)> = Vec::new();
    for &agent in agents {
        let path = agent.skill_path();
        let existing = inspect(&root.join(path))?;
        let action = match (op, existing) {
            (Operation::Install, Existing::Missing) => Action::Installed,
            (Operation::Update | Operation::Remove, Existing::Missing) => Action::NotInstalled,
            (Operation::Install | Operation::Update, Existing::Current) => Action::Unchanged,
            (Operation::Install | Operation::Update, Existing::Managed { version }) => {
                if newer(&version, VERSION) {
                    blocked.push((
                        format!(
                            "{path} was installed by hyp {version}, newer than this hyp {VERSION} \
                         (--force replaces it)"
                        ),
                        true,
                    ));
                }
                Action::Updated
            }
            (Operation::Remove, Existing::Current | Existing::Managed { .. }) => Action::Removed,
            (Operation::Install | Operation::Update, Existing::Modified) => {
                blocked.push((
                    format!("{path} was modified after hyp installed it (--force overwrites it)"),
                    true,
                ));
                Action::Replaced
            }
            (Operation::Install | Operation::Update, Existing::Foreign) => {
                blocked.push((
                    format!("{path} was not installed by hyp (--force overwrites it)"),
                    true,
                ));
                Action::Replaced
            }
            (Operation::Remove, Existing::Modified) => {
                blocked.push((
                    format!("{path} was modified after hyp installed it (--force removes it)"),
                    true,
                ));
                Action::Removed
            }
            (Operation::Remove, Existing::Foreign) => {
                // Not even with --force: remove deletes only what hyp installed.
                blocked.push((
                    format!(
                        "{path} was not installed by hyp; remove it yourself if you want it gone"
                    ),
                    false,
                ));
                Action::Removed
            }
        };
        let changes = !matches!(action, Action::Unchanged | Action::NotInstalled);
        if let Some(dir) = indirect_component(root, path).filter(|_| changes) {
            let verb = if op == Operation::Remove {
                "removes"
            } else {
                "writes"
            };
            blocked.push((
                format!(
                    "{dir} is a symlink or not a directory, so {path} is outside the project \
                     (--force {verb} through it)"
                ),
                true,
            ));
        }
        steps.push(Step {
            agent,
            path: path.to_string(),
            action,
            target: root.join(path),
        });
    }
    let blocked: Vec<String> = blocked
        .into_iter()
        .filter(|(_, overridable)| !(force && *overridable))
        .map(|(reason, _)| reason)
        .collect();
    if !blocked.is_empty() {
        bail!("nothing changed: {}", blocked.join("; "));
    }
    Ok(steps)
}
/// Carry out a plan. Writes are atomic per file.
pub fn apply(steps: &[Step]) -> Result<()> {
    for step in steps {
        let target = &step.target;
        match step.action {
            Action::Installed | Action::Updated | Action::Replaced => {
                let dir = target.parent().context("skill path has no parent")?;
                fs::create_dir_all(dir)
                    .with_context(|| format!("cannot create {}", dir.display()))?;
                crate::store::atomic_readable(target, managed_file(VERSION, SKILL)?.as_bytes())
                    .with_context(|| format!("cannot write {}", target.display()))?;
            }
            Action::Removed => {
                fs::remove_file(target)
                    .with_context(|| format!("cannot remove {}", target.display()))?;
                remove_empty_dirs(target, &step.path)?;
            }
            Action::Unchanged | Action::NotInstalled => {}
        }
    }
    Ok(())
}
/// Remove the directories on the skill's path (`.claude/skills/hyp`,
/// `.claude/skills`, `.claude`) while they are empty real directories. hyp
/// does not record which of them it created, so an empty one that existed
/// before the install goes too. Stops at a symlink, which `--force` may have
/// let `remove` reach through.
fn remove_empty_dirs(target: &Path, path: &str) -> Result<()> {
    let depth = Path::new(path).components().count() - 1;
    for dir in target.ancestors().skip(1).take(depth) {
        if !fs::symlink_metadata(dir).is_ok_and(|m| m.is_dir()) {
            break;
        }
        match fs::remove_dir(dir) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::DirectoryNotEmpty => break,
            Err(e) => {
                return Err(e).with_context(|| format!("cannot remove {}", dir.display()));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn marker_round_trips_and_detects_edits() {
        let file = managed_file("1.2.3", SKILL).unwrap();
        assert_eq!(parse_managed(&file), Some(("1.2.3".into(), true)));
        let edited = file.replace("# hyp: work", "# hyp: my work");
        assert_eq!(parse_managed(&edited), Some(("1.2.3".into(), false)));
        assert_eq!(parse_managed(SKILL), None);
        let in_body = format!("{SKILL}{MARKER} version=1.2.3 sha256={}\n", hash(SKILL));
        assert_eq!(
            parse_managed(&in_body),
            None,
            "only the front matter counts"
        );
    }
    #[test]
    fn versions_compare_numerically() {
        assert!(newer("0.10.0", "0.9.1"));
        assert!(!newer("0.1.0", "0.1.0"));
        assert!(!newer("garbage", "0.1.0"));
    }
}
