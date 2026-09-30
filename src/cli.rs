use crate::{
    agents::{self, Agent, Operation},
    model::*,
    store::{Change, Committed, Conflict, Store, Written},
    web,
};
use anyhow::{Context, Result, ensure};
use clap::{
    CommandFactory, Parser, Subcommand, ValueEnum,
    builder::{PossibleValue, PossibleValuesParser, TypedValueParser},
    error::ErrorKind,
};
use std::{
    io::{IsTerminal, Read},
    path::PathBuf,
};

#[derive(Parser)]
#[command(
    version,
    about = "Hypothesis tracking for coding and research agents: plain files in any directory, Git-friendly"
)]
pub struct Cli {
    /// The project directory; hyp searches upward from here (default: .).
    #[arg(
        long,
        global = true,
        value_name = "DIR",
        default_value = ".",
        hide_default_value = true
    )]
    pub project: PathBuf,
    /// Machine-readable output; errors as {"error": "..."} on stderr.
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Command,
}
impl Cli {
    /// `Cli::parse`, plus the argument checks clap cannot express; a failed
    /// one ends the process like a clap error (usage on stderr, exit 2).
    pub fn parse_checked() -> Self {
        let cli = Self::parse();
        if let Err(err) = cli.check() {
            err.exit();
        }
        cli
    }
    /// Filter combinations that could only list nothing: a `--status` or
    /// `--needs-review` the listed kind cannot have.
    pub fn check(&self) -> Result<(), clap::Error> {
        let Command::List {
            kind,
            all,
            status,
            needs_review,
            ..
        } = &self.command
        else {
            return Ok(());
        };
        let listed = listed_kind(*kind, *all);
        let conflict = match (listed, status) {
            (Some(k), Some(st)) if k != st.kind() => Some(format!(
                "--status applies to {} records, not {k}; add --kind {} or --all",
                st.kind(),
                st.kind()
            )),
            (Some(k), _) if *needs_review && k != Kind::Hypothesis => Some(format!(
                "--needs-review applies to hypotheses, not {k}; drop --kind or use --all"
            )),
            _ => None,
        };
        match conflict {
            None => Ok(()),
            Some(message) => {
                let mut command = Self::command();
                command.build();
                let list = command
                    .find_subcommand_mut("list")
                    .expect("the list subcommand exists");
                Err(list.error(ErrorKind::ArgumentConflict, message))
            }
        }
    }
}
/// The kind `hyp list` lists: `--kind`, else hypotheses; None (every kind)
/// with `--all`.
fn listed_kind(kind: Option<Kind>, all: bool) -> Option<Kind> {
    (!all).then_some(kind.unwrap_or(Kind::Hypothesis))
}
/// Help for a TITLE argument that `titled` reads.
const TITLE: &str = "One line; '-' reads stdin: its first line is the title, \
                     the rest is appended to the body";
/// Help for a hypothesis ID argument.
const HYPOTHESIS: &str = "The hypothesis: an H- ID or unique prefix of one";
#[derive(Subcommand)]
pub enum Command {
    /// Create a project here, optionally with an example notebook.
    Init {
        /// Populate it with a firmware debugging example (synthetic data).
        #[arg(long)]
        demo: bool,
        /// Also install the hyp skill for these coding agents (see `hyp agents`):
        /// claude, codex, both comma-separated, or none (the default).
        #[arg(long, value_name = "LIST", value_parser = parse_init_agents)]
        agents: Option<AgentList>,
    },
    /// Print or install the hyp skill for coding agents.
    ///
    /// Installs it as a project skill for Claude Code
    /// (.claude/skills/hyp/SKILL.md) and Codex (.agents/skills/hyp/SKILL.md).
    Agents {
        #[command(subcommand)]
        command: AgentsCommand,
    },
    /// Capture a hypothesis: a claim that could turn out wrong.
    Add {
        /// The claim. One line; '-' reads stdin: its first line is the
        /// title, the rest is appended to the body.
        title: String,
        /// Details and context ('-' reads stdin).
        #[arg(long, default_value = "", hide_default_value = true)]
        body: String,
        /// Where the claim is meant to hold: system, version, conditions.
        #[arg(long, default_value = "", hide_default_value = true)]
        scope: String,
        /// Comma-separated tags (also --tag).
        #[arg(long, alias = "tag", value_delimiter = ',')]
        tags: Vec<String>,
    },
    /// Add a prediction: what the hypothesis says you will observe.
    Predict {
        #[arg(help = HYPOTHESIS)]
        hypothesis: String,
        /// The expected observation. One line; '-' reads stdin: its first
        /// line is the title, the rest is appended to the body.
        title: String,
        /// When the prediction applies.
        #[arg(long, default_value = "", hide_default_value = true)]
        conditions: String,
    },
    /// Add a falsification criterion: what would refute the hypothesis.
    FalsifyIf {
        #[arg(help = HYPOTHESIS)]
        hypothesis: String,
        /// The refuting observation. One line; '-' reads stdin: its first
        /// line is the title, the rest is appended to the body.
        title: String,
    },
    /// Note a gap: an open question about a hypothesis.
    Gap {
        #[arg(help = HYPOTHESIS)]
        hypothesis: String,
        #[arg(help = TITLE)]
        title: String,
    },
    /// Record evidence and link it to a claim, or attach files to it.
    Evidence {
        #[command(subcommand)]
        command: EvidenceCommand,
    },
    /// Plan an experiment that tests a hypothesis.
    Experiment {
        #[command(subcommand)]
        command: ExperimentCommand,
    },
    /// Record a run of an experiment and its outcome.
    Run {
        /// The experiment: an X- ID or unique prefix of one.
        experiment: String,
        #[arg(help = TITLE)]
        title: String,
        /// How the run went.
        #[arg(long, value_enum, default_value = "observed")]
        outcome: Outcome,
        /// Comma-separated E- IDs of the evidence it produced.
        #[arg(long, value_delimiter = ',')]
        evidence: Vec<String>,
        /// What was done and seen ('-' reads stdin).
        #[arg(long, default_value = "", hide_default_value = true)]
        body: String,
    },
    /// Link evidence to a claim, or relate two hypotheses.
    Link {
        /// The E- evidence, or the H- hypothesis the relation starts at.
        from: String,
        /// The H-, P- or F- record it bears on, or the other H- hypothesis.
        to: String,
        /// supports, contradicts, qualifies: evidence to a claim;
        /// depends-on, competes-with, supersedes: between hypotheses.
        #[arg(long, value_enum)]
        relation: Relation,
        /// Why the link holds ('-' reads stdin).
        #[arg(long)]
        reason: String,
    },
    /// Record a judgment of a hypothesis, based on the state you reviewed.
    Assess {
        #[arg(help = HYPOTHESIS)]
        hypothesis: String,
        /// The review token of the hypothesis state you reviewed: the "review
        /// token:" line of `hyp show H-…` (.state.review_token with --json);
        /// its first 12 or more hex digits suffice. If the basis (claim,
        /// criteria, predictions, links, linked evidence, runs) or the current
        /// assessments changed since, nothing is written and the command
        /// exits 3: review the change and assess again.
        #[arg(long, value_name = "TOKEN")]
        reviewed: String,
        /// The judgment.
        #[arg(long, value_enum)]
        status: Judgment,
        /// Subjective confidence in the judgment, from 0.0 to 1.0 (not a
        /// percentage).
        #[arg(long, value_parser = parse_confidence)]
        confidence: Option<f64>,
        /// Evidence already linked to the hypothesis or its criteria or
        /// predictions (`hyp link` first). Required unless --status untested.
        #[arg(long, value_delimiter = ',')]
        evidence: Vec<String>,
        /// The F- criterion the evidence meets (required for falsified).
        #[arg(long)]
        criterion: Option<String>,
        /// The rationale ('-' reads stdin).
        #[arg(long)]
        reason: String,
    },
    /// Change a record's title, body, tags, lifecycle or status.
    Set {
        /// The record: an ID or unique prefix.
        id: String,
        /// The new title. One line; '-' reads stdin: its first line is the
        /// title, the rest is appended to the body.
        #[arg(long)]
        title: Option<String>,
        /// The new body, replacing the old one ('-' reads stdin).
        #[arg(long)]
        body: Option<String>,
        /// A hypothesis's lifecycle; investigating needs an active criterion
        /// or --untestable-reason.
        #[arg(long, value_enum)]
        lifecycle: Option<Lifecycle>,
        /// Why a hypothesis cannot be tested (instead of a criterion).
        #[arg(long)]
        untestable_reason: Option<String>,
        /// An experiment's status.
        #[arg(long, value_enum)]
        experiment_status: Option<ExperimentStatus>,
        /// Comma-separated tags, replacing the old ones (also --tag).
        #[arg(long, alias = "tag", value_delimiter = ',')]
        tags: Option<Vec<String>>,
        /// Whether a gap is resolved.
        #[arg(long)]
        resolved: Option<bool>,
    },
    /// Show a record; --json gives the complete form.
    ///
    /// Plain output is a summary for people (for a hypothesis with its review
    /// token); --json is the complete form.
    Show {
        /// The record: an ID or unique prefix.
        id: String,
    },
    /// List hypotheses (by default), or records of another kind.
    List {
        /// Only records of this kind (default: hypothesis).
        #[arg(long, value_enum)]
        kind: Option<Kind>,
        /// List every kind of record.
        #[arg(long, conflicts_with = "kind")]
        all: bool,
        /// A judgment or lifecycle (hypotheses) or an experiment status
        /// (experiments: add --kind experiment).
        #[arg(long, value_parser = PossibleValuesParser::new(status_values()).map(|s| Status::parse(&s)))]
        status: Option<Status>,
        /// Only records with this tag (also --tags).
        #[arg(long, alias = "tags")]
        tag: Option<String>,
        /// Only hypotheses whose basis changed since their assessment.
        #[arg(long)]
        needs_review: bool,
        /// Include archived records.
        #[arg(long)]
        archived: bool,
    },
    /// List records containing a text, in any field.
    Search {
        /// The text, matched case-insensitively.
        query: String,
    },
    /// Edit a record's file in $VISUAL or $EDITOR.
    Edit {
        /// The record: an ID or unique prefix.
        id: String,
    },
    /// Archive a record: hide it from lists and the graph.
    Archive {
        /// The record: an ID or unique prefix.
        id: String,
    },
    /// Restore an archived record.
    Restore {
        /// The record: an ID or unique prefix.
        id: String,
    },
    /// Permanently delete an archived, unreferenced record.
    Delete {
        /// The record: an ID or unique prefix.
        id: String,
    },
    /// Apply a batch of changes, a JSON array on stdin.
    ///
    /// All or nothing. Each change states what it depends on: its
    /// `expected_revision`, or for creating an assessment, experiment or run
    /// an `expected` object.
    #[command(after_long_help = APPLY_HELP)]
    Apply {
        /// Apply only if the project is still at this revision (the
        /// "revision" a --json write printed).
        #[arg(long, value_name = "REVISION")]
        expected_revision: Option<String>,
    },
    /// Validate every record; report problems and repairs.
    Check {
        /// Fail on warnings too, not only on errors.
        #[arg(long)]
        strict: bool,
    },
    /// Serve the local web UI on 127.0.0.1.
    Web {
        /// The port to listen on.
        #[arg(long, default_value_t = 7432)]
        port: u16,
    },
    /// Print a Mermaid flowchart of the records and their links.
    Graph {
        /// Only this record and the records it refers to or that refer to it.
        #[arg(long, value_name = "ID")]
        focus: Option<String>,
    },
    /// Export the whole project as Markdown, JSON or HTML.
    Export {
        /// The output format.
        #[arg(long,default_value="markdown",value_parser=["markdown","json","html"])]
        format: String,
        /// Write to this file instead of stdout.
        #[arg(long)]
        output: Option<PathBuf>,
    },
}
/// The long help of `hyp apply`: enough to write a batch without the README.
const APPLY_HELP: &str = r#"Changes:
  {"op": "create",  "record": RECORD}
  {"op": "update",  "record": RECORD, "expected_revision": REV}
  {"op": "archive", "id": ID, "archived": true, "expected_revision": REV}
  {"op": "delete",  "id": ID, "expected_revision": REV}

RECORD is a record as `hyp --json show ID` prints it (.entry.record): "kind",
"title", optional "body" and "tags", and the fields of its kind. An update
gives the whole record as read, changed. A create may set "id" to a new full
<letter>-<UUID> ID, so later changes in the batch can refer to it. REV is
.entry.revision of `hyp --json show ID`, or a revision a --json write printed.
"archived": false restores. Use full IDs, not prefixes.

Create a hypothesis, archive a prediction:
  [{"op": "create", "record": {"kind": "hypothesis", "lifecycle": "draft",
     "title": "The cache causes the timeouts"}},
   {"op": "archive", "id": "P-…", "archived": true, "expected_revision": "…"}]

Creating an assessment, experiment or run states in "expected" what it was
based on; if that changed since, nothing is written and hyp exits 3. An
assessment states its hypothesis's .state.review_token (`hyp --json show H-…`)
and cites only evidence linked to the hypothesis or its criteria or
predictions; every judgment except untested cites some, and falsified also a
"criterion": "F-…":
  [{"op": "create",
    "record": {"kind": "assessment", "title": "Weakened", "body": "Why …",
               "hypothesis": "H-…", "judgment": "weakened",
               "confidence": 0.3, "evidence": ["E-…"]},
    "expected": {"hypotheses": {"H-…": {"review_token": "…"}}}}]

An experiment ("hypothesis", "status", "targets": [{"id": "H-…"}, {"id":
"P-…"}], its hypothesis among them) states the revision of every target, a run
("experiment", "outcome", "evidence") those of its experiment and evidence:
  "expected": {"revisions": {"H-…": "…", "P-…": "…"}}"#;
/// A `hyp list --status` value: judgments and lifecycles apply to
/// hypotheses, experiment statuses to experiments.
#[derive(Clone, Copy)]
pub enum Status {
    Judgment(Judgment),
    Lifecycle(Lifecycle),
    Experiment(ExperimentStatus),
}
fn possible<T: ValueEnum + 'static>() -> impl Iterator<Item = PossibleValue> {
    T::value_variants().iter().filter_map(T::to_possible_value)
}
fn status_values() -> Vec<PossibleValue> {
    possible::<Judgment>()
        .chain(possible::<Lifecycle>())
        .chain(possible::<ExperimentStatus>())
        .collect()
}
impl Status {
    /// Only called on a value `status_values` accepted.
    fn parse(value: &str) -> Self {
        Judgment::from_str(value, false)
            .map(Self::Judgment)
            .or_else(|_| Lifecycle::from_str(value, false).map(Self::Lifecycle))
            .or_else(|_| ExperimentStatus::from_str(value, false).map(Self::Experiment))
            .expect("a value from status_values")
    }
    fn kind(self) -> Kind {
        match self {
            Self::Judgment(_) | Self::Lifecycle(_) => Kind::Hypothesis,
            Self::Experiment(_) => Kind::Experiment,
        }
    }
    fn matches(self, e: &Entry, state: Option<&HypothesisState>) -> bool {
        match (self, &e.record.data) {
            (Self::Judgment(j), _) => state.is_some_and(|x| x.judgment == j),
            (Self::Lifecycle(l), Data::Hypothesis { lifecycle, .. }) => *lifecycle == l,
            (Self::Experiment(x), Data::Experiment { status, .. }) => *status == x,
            _ => false,
        }
    }
}
/// Agents in `Agent::ALL` order, without duplicates.
#[derive(Clone)]
pub struct AgentList(Vec<Agent>);
impl AgentList {
    fn of(list: &[Agent]) -> Self {
        Self(
            Agent::ALL
                .into_iter()
                .filter(|a| list.contains(a))
                .collect(),
        )
    }
}
/// `none` alone, or a comma-separated list of agents. A clap value parser,
/// so a bad list is an argument error (exit 2).
fn parse_init_agents(value: &str) -> Result<AgentList, String> {
    if value == "none" {
        return Ok(AgentList(vec![]));
    }
    let list = value
        .split(',')
        .map(|name| match name {
            "none" => Err("none cannot be combined with other agents".to_string()),
            _ => Agent::from_str(name, false)
                .map_err(|_| format!("unknown agent {name:?}; use claude, codex, or none")),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AgentList::of(&list))
}
#[derive(Subcommand)]
pub enum AgentsCommand {
    /// Print the skill to stdout.
    Print,
    /// Install or refresh the skill (default: all agents).
    Install(AgentsArgs),
    /// Refresh installed skills to this hyp version; installs nothing new.
    Update(AgentsArgs),
    /// Delete skills that hyp installed.
    Remove(AgentsArgs),
}
#[derive(clap::Args)]
pub struct AgentsArgs {
    /// Comma-separated coding agents.
    #[arg(
        long,
        value_enum,
        value_delimiter = ',',
        value_name = "LIST",
        default_value = "claude,codex"
    )]
    agents: Vec<Agent>,
    /// Also overwrite or delete a skill file that was modified after hyp
    /// installed it (or overwrite one hyp did not install).
    #[arg(long)]
    force: bool,
}
#[derive(Subcommand)]
pub enum EvidenceCommand {
    /// Record evidence and link it to the claim it bears on.
    Add {
        /// The H-, P- or F- record it bears on: an ID or unique prefix.
        hypothesis: String,
        /// The observation. One line; '-' reads stdin: its first line is
        /// the title, the rest is appended to the body.
        title: String,
        /// Where it comes from: a file, URL, command or log.
        #[arg(long)]
        source: String,
        /// Where in the source: a line, timestamp or page.
        #[arg(long, default_value = "", hide_default_value = true)]
        locator: String,
        /// It contradicts the claim (default: supports).
        #[arg(long)]
        against: bool,
        /// It qualifies the claim: neither supports nor contradicts it.
        #[arg(long, conflicts_with = "against")]
        qualifies: bool,
        /// Why it bears on the claim (default: the title; '-' reads stdin).
        #[arg(long, default_value = "", hide_default_value = true)]
        reason: String,
        /// The observation in detail ('-' reads stdin).
        #[arg(long, default_value = "", hide_default_value = true)]
        body: String,
    },
    /// Copy a file into the project and attach it to evidence.
    Attach {
        /// The evidence: an E- ID or unique prefix.
        id: String,
        /// The file (at most 32 MiB).
        path: PathBuf,
    },
}
#[derive(Subcommand)]
pub enum ExperimentCommand {
    /// Plan an experiment; its targets are frozen as they are now.
    Add {
        #[arg(help = HYPOTHESIS)]
        hypothesis: String,
        /// What the experiment does. One line; '-' reads stdin: its first
        /// line is the title, the rest is appended to the body.
        title: String,
        /// Comma-separated F-/P- IDs it tests (the hypothesis always is a target).
        #[arg(long, value_delimiter = ',')]
        targets: Vec<String>,
        /// The procedure ('-' reads stdin).
        #[arg(long, default_value = "", hide_default_value = true)]
        body: String,
    },
}
/// A `--confidence` value: any number here, so that one out of range gets
/// `confidence_in_range`'s hint.
fn parse_confidence(value: &str) -> Result<f64, String> {
    value
        .parse()
        .map_err(|_| "expected a number from 0.0 to 1.0".to_string())
}
fn confidence_in_range(confidence: Option<f64>) -> Result<()> {
    let Some(c) = confidence else { return Ok(()) };
    // 2 to 100 reads as a percentage; 1.5 more likely as a slip.
    let hint = if (2.0..=100.0).contains(&c) {
        format!(" (did you mean {}?)", c / 100.0)
    } else {
        String::new()
    };
    ensure!(
        c.is_finite() && (0.0..=1.0).contains(&c),
        "--confidence must be from 0.0 to 1.0, got {c}{hint}"
    );
    Ok(())
}
/// The text arguments of `command` given as '-', by name. Each reads all of
/// stdin, so at most one may.
fn stdin_arguments(command: &Command) -> Vec<&'static str> {
    const TITLE: &str = "TITLE";
    let texts: Vec<(&str, Option<&String>)> = match command {
        Command::Add { title, body, .. }
        | Command::Run { title, body, .. }
        | Command::Experiment {
            command: ExperimentCommand::Add { title, body, .. },
        } => vec![(TITLE, Some(title)), ("--body", Some(body))],
        Command::Predict { title, .. }
        | Command::FalsifyIf { title, .. }
        | Command::Gap { title, .. } => vec![(TITLE, Some(title))],
        Command::Evidence {
            command:
                EvidenceCommand::Add {
                    title,
                    reason,
                    body,
                    ..
                },
        } => vec![
            (TITLE, Some(title)),
            ("--reason", Some(reason)),
            ("--body", Some(body)),
        ],
        Command::Link { reason, .. } | Command::Assess { reason, .. } => {
            vec![("--reason", Some(reason))]
        }
        Command::Set { title, body, .. } => {
            vec![("--title", title.as_ref()), ("--body", body.as_ref())]
        }
        _ => vec![],
    };
    texts
        .into_iter()
        .filter(|(_, value)| value.is_some_and(|v| v == "-"))
        .map(|(name, _)| name)
        .collect()
}
/// A text argument (`arg` names it): the text, or with '-' all of stdin. A
/// person typing it (stdin and stderr are terminals) is told how to end it.
fn input(arg: &str, s: String) -> Result<String> {
    if s == "-" {
        if std::io::stdin().is_terminal() && std::io::stderr().is_terminal() {
            eprintln!("reading {arg} from stdin; end with Ctrl-D");
        }
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text)?;
        Ok(text.trim_end().to_string())
    } else {
        Ok(s)
    }
}
fn print_json(value: &impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
/// What a write command prints: one full ID per line, or with --json
/// `{"written": [{"id", "kind", "revision"}], "revision"}`, the objects
/// its changes named with their revisions (null once deleted) and the
/// project revision, all after the write.
fn print_written(written: &[Written], after: &Snapshot, json: bool) -> Result<()> {
    if json {
        return print_json(&serde_json::json!({"written": written, "revision": after.revision}));
    }
    for w in written {
        println!("{}", w.id);
    }
    Ok(())
}
/// What a change did, for the human summary of a write.
#[derive(Clone, Copy, PartialEq)]
enum Action {
    Created,
    Updated,
    Archived,
    Restored,
    Deleted,
}
impl Action {
    fn of(change: &Change) -> Self {
        match change {
            Change::Create { .. } => Self::Created,
            Change::Update { .. } => Self::Updated,
            Change::Archive { archived: true, .. } => Self::Archived,
            Change::Archive {
                archived: false, ..
            } => Self::Restored,
            Change::Delete { .. } => Self::Deleted,
        }
    }
    fn verb(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Updated => "updated",
            Self::Archived => "archived",
            Self::Restored => "restored",
            Self::Deleted => "deleted",
        }
    }
}
/// An ID shortened to its kind letter and 8 hex digits, a prefix hyp accepts.
fn short(id: &str) -> &str {
    id.get(..10).unwrap_or(id)
}
/// One line for people saying what a write did, `actions[i]` being what its
/// i-th change was: e.g. "created evidence E-… (+ link L-…: E-… supports
/// H-…)", or "no changes" when nothing was written.
fn summary(actions: &[Action], c: &Committed) -> String {
    let changed: Vec<(Action, &Written)> = actions
        .iter()
        .copied()
        .zip(&c.written)
        .filter(|(_, w)| w.changed)
        .collect();
    if changed.is_empty() {
        return match (actions, c.written.as_slice()) {
            ([Action::Restored], [w]) => format!("no changes: {} is not archived", short(&w.id)),
            ([Action::Archived], [w]) => {
                format!("no changes: {} is already archived", short(&w.id))
            }
            _ => "no changes".into(),
        };
    }
    let what = |w: &Written| match c.snapshot.get(&w.id).map(|e| &e.record.data) {
        Some(Data::Link { from, to, relation }) => format!(
            "link {}: {} {relation} {}",
            short(&w.id),
            short(from),
            short(to)
        ),
        _ => format!("{} {}", w.kind, short(&w.id)),
    };
    let mut groups: Vec<(Action, Vec<String>)> = Vec::new();
    for (action, w) in changed {
        match groups.iter_mut().find(|(a, _)| *a == action) {
            Some((_, items)) => items.push(what(w)),
            None => groups.push((action, vec![what(w)])),
        }
    }
    let unchanged = c.written.len() - groups.iter().map(|(_, i)| i.len()).sum::<usize>();
    let mut parts: Vec<String> = groups
        .iter()
        .map(|(action, items)| match items.as_slice() {
            [first] => format!("{} {first}", action.verb()),
            [first, rest @ ..] => format!("{} {first} (+ {})", action.verb(), rest.join(", ")),
            [] => unreachable!("a group has an item"),
        })
        .collect();
    if unchanged > 0 {
        parts.push(format!("{unchanged} unchanged"));
    }
    parts.join("; ")
}
/// Prints what a write wrote (`print_written`, the contract) and, without
/// --json, its `summary` on stderr: always when nothing changed, otherwise
/// only for a person (stderr is a terminal).
fn report(actions: &[Action], c: &Committed, json: bool) -> Result<()> {
    print_written(&c.written, &c.snapshot, json)?;
    let nothing = c.written.iter().all(|w| !w.changed);
    if !json && (nothing || std::io::stderr().is_terminal()) {
        eprintln!("{}", summary(actions, c));
    }
    Ok(())
}
/// A TITLE argument: the text, or with '-' the first line of stdin and the
/// lines after it, which go to the body. A given text is kept as it is: a
/// newline in it fails validation.
fn title_input(arg: &str, title: String) -> Result<(String, String)> {
    if title != "-" {
        return Ok((title, String::new()));
    }
    let text = input(arg, title)?;
    let text = text.trim_start();
    let (first, rest) = text.split_once('\n').unwrap_or((text, ""));
    Ok((
        first.trim_end().to_string(),
        rest.trim_start_matches(['\n', '\r']).to_string(),
    ))
}
/// `body`, then `more` after a blank line.
fn joined(body: String, more: String) -> String {
    match (body.is_empty(), more.is_empty()) {
        (_, true) => body,
        (true, false) => more,
        (false, false) => format!("{body}\n\n{more}"),
    }
}
/// A new record from a TITLE argument and a --body argument (or "").
fn titled(title: String, body: String, data: Data) -> Result<Record> {
    let (title, more) = title_input("TITLE", title)?;
    let mut r = Record::new(title, data);
    r.body = joined(input("--body", body)?, more);
    Ok(r)
}
/// The full ID of the record that argument `arg` (`<NAME>` or `--flag`)
/// names, which must be of one of `kinds`.
fn find_kind(s: &Snapshot, arg: &str, id: &str, kinds: &[Kind]) -> Result<String> {
    let e = s.find(id).with_context(|| format!("argument {arg}"))?;
    let kind = e.record.data.kind_value();
    if !kinds.contains(&kind) {
        let names: Vec<&str> = kinds.iter().map(|k| k.as_str()).collect();
        let expected = match names.split_last() {
            Some((last, [])) => last.to_string(),
            Some((last, init)) => format!("{} or {last}", init.join(", ")),
            None => unreachable!("no kinds given"),
        };
        // "evidence" is a mass noun: "expected evidence", not "an evidence".
        let a = match kinds[0] {
            Kind::Evidence => String::new(),
            k => format!("{} ", article(k.as_str())),
        };
        anyhow::bail!(
            "argument {arg}: expected {a}{expected}, got {kind} {}",
            e.record.id
        );
    }
    Ok(e.record.id.clone())
}
fn find_all(s: &Snapshot, arg: &str, ids: &[String], kinds: &[Kind]) -> Result<Vec<String>> {
    ids.iter().map(|id| find_kind(s, arg, id, kinds)).collect()
}
/// A create with its preconditions stated from `s`, the snapshot the command read.
fn create(s: &Snapshot, r: Record) -> Change {
    Change::create_seen(r, s)
}
fn hypothesis(s: &Snapshot, id: &str) -> Result<String> {
    find_kind(s, "<HYPOTHESIS>", id, &[Kind::Hypothesis])
}
/// What evidence bears on, and what an experiment tests.
const CLAIM: [Kind; 3] = [Kind::Hypothesis, Kind::Prediction, Kind::Criterion];
fn print_steps(steps: &[agents::Step], json: bool) -> Result<()> {
    if json {
        return print_json(&steps);
    }
    for step in steps {
        let action = serde_json::to_value(step.action)?;
        println!("{} {}", action.as_str().unwrap_or_default(), step.path);
    }
    Ok(())
}
pub async fn run(cli: Cli) -> Result<()> {
    if let names @ [_, _, ..] = stdin_arguments(&cli.command).as_slice() {
        anyhow::bail!(
            "{} are each '-', but stdin can be read only once: give all but one as text",
            names.join(" and ")
        );
    }
    if let Command::Init { demo, agents } = &cli.command {
        let agents = agents.clone().map(|list| list.0).unwrap_or_default();
        // Planned before the project exists, so a blocked skill file fails
        // `init` before it creates anything.
        let steps = agents::plan(&cli.project, &agents, Operation::Install, false).context(
            "cannot install the agent skills (run `hyp init` without --agents, then \
                 `hyp agents install --force` if hyp should replace or write through them)",
        )?;
        let store = Store::init(&cli.project)?;
        if *demo {
            seed_demo(&store)?;
        }
        agents::apply(&steps)?;
        if cli.json {
            // A new project: everything in it was written by this command.
            let s = store.snapshot()?;
            let written: Vec<Written> = s.objects.iter().map(Written::of).collect();
            print_written(&written, &s, true)?;
        } else {
            println!("Initialized {}", store.root.display());
            print_steps(&steps, false)?;
            println!(
                "Next: hyp add \"<claim>\", then hyp falsify-if H-… \"<what would refute it>\"; \
                 hyp agents install teaches coding agents to use hyp"
            );
        }
        return Ok(());
    }
    if let Command::Agents { command } = &cli.command {
        let (op, args) = match command {
            AgentsCommand::Print => {
                print!("{}", agents::SKILL);
                return Ok(());
            }
            AgentsCommand::Install(args) => (Operation::Install, args),
            AgentsCommand::Update(args) => (Operation::Update, args),
            AgentsCommand::Remove(args) => (Operation::Remove, args),
        };
        let root = Store::open(&cli.project)?.root;
        let list = AgentList::of(&args.agents);
        let steps = agents::plan(&root, &list.0, op, args.force)?;
        agents::apply(&steps)?;
        return print_steps(&steps, cli.json);
    }
    let store = Store::open(&cli.project)?;
    if let Command::Web { port } = cli.command {
        return web::serve(store, port).await;
    }
    let s = store.snapshot()?;
    let mut changes = Vec::new();
    match cli.command {
        Command::Add {
            title,
            body,
            scope,
            tags,
        } => {
            let mut r = titled(
                title,
                body,
                Data::Hypothesis {
                    scope,
                    assumptions: String::new(),
                    lifecycle: Lifecycle::Draft,
                    untestable_reason: String::new(),
                },
            )?;
            r.tags = tags;
            changes.push(create(&s, r));
        }
        Command::Predict {
            hypothesis: h,
            title,
            conditions,
        } => changes.push(create(
            &s,
            titled(
                title,
                String::new(),
                Data::Prediction {
                    hypothesis: hypothesis(&s, &h)?,
                    conditions,
                },
            )?,
        )),
        Command::FalsifyIf {
            hypothesis: h,
            title,
        } => changes.push(create(
            &s,
            titled(
                title,
                String::new(),
                Data::Criterion {
                    hypothesis: hypothesis(&s, &h)?,
                },
            )?,
        )),
        Command::Gap {
            hypothesis: h,
            title,
        } => changes.push(create(
            &s,
            titled(
                title,
                String::new(),
                Data::Gap {
                    hypothesis: hypothesis(&s, &h)?,
                    resolved: false,
                },
            )?,
        )),
        Command::Evidence { command } => match command {
            EvidenceCommand::Add {
                hypothesis: h,
                title,
                source,
                locator,
                against,
                qualifies,
                reason,
                body,
            } => {
                let target = find_kind(&s, "<HYPOTHESIS>", &h, &CLAIM)?;
                let r = titled(
                    title,
                    body,
                    Data::Evidence {
                        source,
                        locator,
                        observed_at: chrono::Utc::now().to_rfc3339(),
                        attachments: vec![],
                    },
                )?;
                let mut l = Record::new(
                    format!("Evidence for {}", &target[..10]),
                    Data::Link {
                        from: r.id.clone(),
                        to: target,
                        relation: if against {
                            Relation::Contradicts
                        } else if qualifies {
                            Relation::Qualifies
                        } else {
                            Relation::Supports
                        },
                    },
                );
                l.body = if reason.is_empty() {
                    r.title.clone()
                } else {
                    input("--reason", reason)?
                };
                changes.extend([create(&s, r), create(&s, l)]);
            }
            EvidenceCommand::Attach { id, path } => {
                return report(&[Action::Updated], &store.attach(&id, &path)?, cli.json);
            }
        },
        Command::Experiment {
            command:
                ExperimentCommand::Add {
                    hypothesis: h,
                    title,
                    targets,
                    body,
                },
        } => {
            let hypothesis = hypothesis(&s, &h)?;
            let targets = find_all(&s, "--targets", &targets, &CLAIM)?
                .iter()
                .map(|id| s.find(id).map(Entry::frozen))
                .collect::<Result<_>>()?;
            let r = titled(
                title,
                body,
                Data::Experiment {
                    hypothesis,
                    targets,
                    status: ExperimentStatus::Planned,
                },
            )?;
            changes.push(create(&s, r));
        }
        Command::Run {
            experiment,
            title,
            outcome,
            evidence,
            body,
        } => {
            let e = s.find(&find_kind(
                &s,
                "<EXPERIMENT>",
                &experiment,
                &[Kind::Experiment],
            )?)?;
            let r = titled(
                title,
                body,
                Data::Run {
                    experiment: e.record.id.clone(),
                    plan: e.frozen(),
                    outcome,
                    evidence: find_all(&s, "--evidence", &evidence, &[Kind::Evidence])?,
                },
            )?;
            changes.push(create(&s, r));
        }
        Command::Link {
            from,
            to,
            relation,
            reason,
        } => {
            let mut r = Record::new(
                format!("{from} {relation} {to}"),
                Data::Link {
                    from: find_kind(&s, "<FROM>", &from, &[Kind::Evidence, Kind::Hypothesis])?,
                    to: find_kind(&s, "<TO>", &to, &CLAIM)?,
                    relation,
                },
            );
            r.body = input("--reason", reason)?;
            changes.push(create(&s, r));
        }
        Command::Assess {
            hypothesis: h,
            reviewed,
            status,
            confidence,
            evidence,
            criterion,
            reason,
        } => {
            let h = hypothesis(&s, &h)?;
            // A prefix of 12 hex digits (48 bits) is ample to detect a change.
            let reviewed = reviewed.to_ascii_lowercase();
            ensure!(
                (12..=64).contains(&reviewed.len())
                    && reviewed.bytes().all(|b| b.is_ascii_hexdigit()),
                "--reviewed must be the review token that `hyp show {h}` prints (its \
                 \"review token:\" line), or its first 12 or more hex digits"
            );
            confidence_in_range(confidence)?;
            let state = s
                .hypotheses
                .get(&h)
                .with_context(|| format!("no derived state for hypothesis {h}"))?;
            // The token covers the agent's review up to this command's read;
            // the preconditions stated below cover this read up to the write.
            if !state
                .review_token
                .to_ascii_lowercase()
                .starts_with(&reviewed)
            {
                return Err(Conflict(format!(
                    "hypothesis {h} changed since you reviewed it (its basis or its current \
                     assessments; the review token differs), so nothing was written. \
                     `hyp show {h}` lists its basis now (criteria, predictions, linked \
                     evidence, runs, links to other hypotheses) and its current \
                     assessments: compare them with what you reviewed, then assess again \
                     with the review token it prints"
                ))
                .into());
            }
            let mut r = Record::new(
                format!("Assessment: {status}"),
                Data::Assessment {
                    hypothesis: h.clone(),
                    judgment: status,
                    confidence,
                    evidence: find_all(&s, "--evidence", &evidence, &[Kind::Evidence])?,
                    criterion: criterion
                        .map(|c| find_kind(&s, "--criterion", &c, &[Kind::Criterion]))
                        .transpose()?,
                    based_on: String::new(),
                    supersedes: vec![],
                },
            );
            r.body = input("--reason", reason)?;
            changes.push(create(&s, r));
        }
        Command::Set {
            id,
            title,
            body,
            lifecycle,
            untestable_reason,
            experiment_status,
            tags,
            resolved,
        } => {
            let e = s.find(&id)?;
            let mut r = e.record.clone();
            if let Some(b) = body {
                r.body = input("--body", b)?;
            }
            if let Some(t) = title {
                let (title, more) = title_input("--title", t)?;
                r.title = title;
                r.body = joined(std::mem::take(&mut r.body), more);
            }
            if let Some(t) = tags {
                r.tags = t;
            }
            if let Some(state) = lifecycle {
                if let Data::Hypothesis { lifecycle, .. } = &mut r.data {
                    *lifecycle = state;
                } else {
                    anyhow::bail!("lifecycle only applies to hypotheses");
                }
            }
            if let Some(reason) = untestable_reason {
                if let Data::Hypothesis {
                    untestable_reason, ..
                } = &mut r.data
                {
                    *untestable_reason = reason;
                } else {
                    anyhow::bail!("untestable_reason only applies to hypotheses");
                }
            }
            if let Some(state) = experiment_status {
                if let Data::Experiment { status, .. } = &mut r.data {
                    *status = state;
                } else {
                    anyhow::bail!("experiment-status only applies to experiments");
                }
            }
            if let Some(value) = resolved {
                if let Data::Gap { resolved, .. } = &mut r.data {
                    *resolved = value;
                } else {
                    anyhow::bail!("resolved only applies to gaps");
                }
            }
            changes.push(Change::Update {
                record: r,
                expected_revision: e.revision.clone(),
            });
        }
        Command::Show { id } => {
            let e = s.find(&id)?;
            if cli.json {
                let related: Vec<&Entry> = s
                    .objects
                    .iter()
                    .filter(|x| x.record.data.references().contains(&e.record.id.as_str()))
                    .collect();
                // For a hypothesis, what its fingerprint hashes (`basis`), plus
                // in full the runs and evidence in it, which `related` does not
                // reach (a link or run names only IDs).
                let (basis, runs, evidence) = match e.record.data {
                    Data::Hypothesis { .. } => {
                        let basis = s.basis(&e.record.id);
                        let of_kind = |kind: &str| -> Vec<&Entry> {
                            s.objects
                                .iter()
                                .filter(|x| {
                                    x.record.data.kind() == kind && basis.contains_key(&x.record.id)
                                })
                                .collect()
                        };
                        let (runs, evidence) = (of_kind("run"), of_kind("evidence"));
                        (Some(basis), Some(runs), Some(evidence))
                    }
                    _ => (None, None, None),
                };
                print_json(&serde_json::json!({
                    "entry": e,
                    "state": s.hypotheses.get(&e.record.id),
                    "related": related,
                    "runs": runs,
                    "evidence": evidence,
                    "basis": basis,
                }))?;
            } else {
                println!("{}", crate::show::plain(&s, e));
            }
            return Ok(());
        }
        Command::List {
            kind,
            all,
            status,
            tag,
            needs_review,
            archived,
        } => {
            // `Cli::check` rejected filters the listed kind cannot match.
            let kind = listed_kind(kind, all);
            let rows: Vec<_> = s
                .objects
                .iter()
                .filter(|e| {
                    let r = &e.record;
                    let state = s.hypotheses.get(&r.id);
                    (archived || !r.archived)
                        && kind.is_none_or(|k| r.data.kind_value() == k)
                        && tag.as_ref().is_none_or(|t| r.tags.contains(t))
                        && (!needs_review || state.is_some_and(|x| x.needs_review))
                        && status.is_none_or(|x| x.matches(e, state))
                })
                .collect();
            list(&s, &rows, cli.json)?;
            return Ok(());
        }
        Command::Search { query } => {
            let q = query.to_lowercase();
            let rows = s
                .objects
                .iter()
                .filter(|e| {
                    serde_json::to_string(&e.record)
                        .unwrap_or_default()
                        .to_lowercase()
                        .contains(&q)
                })
                .collect::<Vec<_>>();
            list(&s, &rows, cli.json)?;
            return Ok(());
        }
        Command::Edit { id } => {
            let committed = crate::edit::edit(&store, s.find(&id)?)?;
            return report(&[Action::Updated], &committed, cli.json);
        }
        Command::Archive { id } => {
            let e = s.find(&id)?;
            changes.push(Change::Archive {
                id: e.record.id.clone(),
                archived: true,
                expected_revision: e.revision.clone(),
            });
        }
        Command::Restore { id } => {
            let e = s.find(&id)?;
            changes.push(Change::Archive {
                id: e.record.id.clone(),
                archived: false,
                expected_revision: e.revision.clone(),
            });
        }
        Command::Delete { id } => {
            let e = s.find(&id)?;
            changes.push(Change::Delete {
                id: e.record.id.clone(),
                expected_revision: e.revision.clone(),
            });
        }
        Command::Apply { expected_revision } => {
            let mut raw = String::new();
            std::io::stdin().read_to_string(&mut raw)?;
            let changes: Vec<Change> = serde_json::from_str(&raw)?;
            let actions: Vec<Action> = changes.iter().map(Action::of).collect();
            let committed = store.commit_written(changes, expected_revision.as_deref())?;
            return report(&actions, &committed, cli.json);
        }
        Command::Check { strict } => {
            if cli.json {
                print_json(&s.diagnostics)?;
            } else {
                for d in &s.diagnostics {
                    println!("{} {}: {}", d.severity, d.path, d.message);
                    if let Some(repair) = &d.repair {
                        if let Some(note) = &repair.note {
                            println!("  note: {note}");
                        }
                        for command in &repair.commands {
                            println!("  repair: {}", command.join(" "));
                        }
                    }
                }
                match s.objects.len() {
                    1 => println!("Checked 1 object"),
                    n => println!("Checked {n} objects"),
                }
            }
            ensure!(
                !s.diagnostics
                    .iter()
                    .any(|d| strict || d.severity == "error"),
                "validation failed"
            );
            return Ok(());
        }
        Command::Graph { focus } => {
            let focus = focus
                .map(|id| s.find(&id).map(|e| e.record.id.clone()))
                .transpose()?;
            println!("{}", graph(&s, focus.as_deref()));
            return Ok(());
        }
        Command::Export { format, output } => {
            let text = match format.as_str() {
                "json" => serde_json::to_string_pretty(&s)?,
                "html" => web::export_html(&s)?,
                _ => markdown(&s),
            };
            if let Some(path) = output {
                std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
            } else {
                println!("{text}");
            }
            return Ok(());
        }
        Command::Init { .. } | Command::Agents { .. } | Command::Web { .. } => unreachable!(),
    }
    // Each change states what it depends on; the whole-project revision would
    // turn unrelated concurrent writes into conflicts.
    let actions: Vec<Action> = changes.iter().map(Action::of).collect();
    report(&actions, &store.commit_written(changes, None)?, cli.json)
}
/// With `json`, each row is the entry plus, for a hypothesis, its derived
/// `state`. A plain hypothesis row shows its judgment, lifecycle and, when
/// it has one, a `needs-review` marker.
fn list(s: &Snapshot, rows: &[&Entry], json: bool) -> Result<()> {
    if json {
        let rows: Vec<_> = rows
            .iter()
            .map(|e| {
                let mut row = serde_json::json!(e);
                if let Some(state) = s.hypotheses.get(&e.record.id) {
                    row["state"] = serde_json::json!(state);
                }
                row
            })
            .collect();
        return print_json(&rows);
    }
    for e in rows {
        let r = &e.record;
        let status = match (&r.data, s.hypotheses.get(&r.id)) {
            (Data::Hypothesis { lifecycle, .. }, Some(state)) => format!(
                "{:12} {:13} {:12} ",
                state.judgment.as_str(),
                lifecycle.as_str(),
                if state.needs_review {
                    "needs-review"
                } else {
                    ""
                }
            ),
            _ => String::new(),
        };
        println!(
            "{}  {:11} {status}{}{}",
            r.id,
            r.data.kind(),
            r.title,
            if r.archived { " [archived]" } else { "" }
        );
    }
    Ok(())
}
pub fn graph(s: &Snapshot, focus: Option<&str>) -> String {
    let mut out = String::from("flowchart TD\n");
    let mut selected = std::collections::BTreeSet::new();
    for e in &s.objects {
        if e.record.archived {
            continue;
        }
        if focus.is_none()
            || focus == Some(e.record.id.as_str())
            || e.record
                .data
                .references()
                .iter()
                .any(|id| Some(*id) == focus)
        {
            selected.insert(e.record.id.clone());
            selected.extend(e.record.data.references().into_iter().map(str::to_string));
        }
    }
    for e in &s.objects {
        if selected.contains(&e.record.id) && !matches!(e.record.data, Data::Link { .. }) {
            let label = e
                .record
                .title
                .replace('&', "&amp;")
                .replace('"', "&quot;")
                .replace(['\n', '\r', '<', '>', '[', ']'], " ");
            out.push_str(&format!(
                "  {}[\"{}: {}\"]\n",
                e.record.id.replace('-', "_"),
                e.record.data.kind(),
                label
            ));
        }
    }
    for e in &s.objects {
        if !selected.contains(&e.record.id) {
            continue;
        }
        match &e.record.data {
            Data::Link { from, to, relation } => out.push_str(&format!(
                "  {} -->|{}| {}\n",
                from.replace('-', "_"),
                relation,
                to.replace('-', "_")
            )),
            d => {
                if let Some(owner) = d.owner() {
                    out.push_str(&format!(
                        "  {} --> {}\n",
                        owner.replace('-', "_"),
                        e.record.id.replace('-', "_")
                    ));
                }
            }
        }
    }
    out
}
pub fn markdown(s: &Snapshot) -> String {
    let mut out = String::from("# hyp — research notebook\n\n");
    for e in &s.objects {
        out.push_str(&format!(
            "## {}\n\n`{}` · {}{}\n\n{}\n\n",
            e.record.title,
            e.record.id,
            e.record.data.kind(),
            if e.record.archived {
                " · archived"
            } else {
                ""
            },
            e.record.body
        ));
        if let Some(state) = s.hypotheses.get(&e.record.id) {
            out.push_str(&format!(
                "Assessment: {} · needs review: {}\n\n",
                state.judgment, state.needs_review
            ));
        }
        let mut data = serde_yaml::to_value(&e.record.data).unwrap_or_default();
        // As `hyp link --relation` spells it, not as stored.
        if let Data::Link { relation, .. } = &e.record.data {
            data["relation"] = relation.as_str().into();
        }
        out.push_str(&format!(
            "```yaml\n{}```\n\n",
            serde_yaml::to_string(&data).unwrap_or_default()
        ));
    }
    out
}
pub fn seed_demo(store: &Store) -> Result<()> {
    let mut h = Record::new(
        "DMA timeout is caused by cache coherency",
        Data::Hypothesis {
            scope: "Board revision C · Cortex-M7 · firmware 0.8".into(),
            assumptions: "The cache-disabled build otherwise has identical configuration.".into(),
            lifecycle: Lifecycle::Draft,
            untestable_reason: String::new(),
        },
    );
    h.tags = vec!["firmware".into(), "dma".into()];
    h.body="Intermittent DMA timeouts appear after sustained traffic. Test whether stale cache lines explain the failures.".into();
    let f = Record::new(
        "Timeout reproduces with D-cache disabled",
        Data::Criterion {
            hypothesis: h.id.clone(),
        },
    );
    let p = Record::new(
        "Clean + invalidate eliminates failures",
        Data::Prediction {
            hypothesis: h.id.clone(),
            conditions: "10,000 transfers at 80 MHz, board revision C".into(),
        },
    );
    let g = Record::new(
        "Does disabling cache change DMA timing?",
        Data::Gap {
            hypothesis: h.id.clone(),
            resolved: false,
        },
    );
    let mut alt = Record::new(
        "DMA timeout is caused by bus contention",
        Data::Hypothesis {
            scope: "Board revision C under simultaneous USB traffic".into(),
            assumptions: String::new(),
            lifecycle: Lifecycle::Draft,
            untestable_reason: String::new(),
        },
    );
    alt.tags = vec!["firmware".into()];
    let mut l = Record::new(
        "Alternative explanation",
        Data::Link {
            from: h.id.clone(),
            to: alt.id.clone(),
            relation: Relation::CompetesWith,
        },
    );
    l.body = "Compare both explanations under controlled bus load.".into();
    let s = store.snapshot()?;
    store.commit(
        vec![
            create(&s, h.clone()),
            create(&s, f.clone()),
            create(&s, p.clone()),
            create(&s, g),
            create(&s, alt.clone()),
            create(&s, l),
        ],
        None,
    )?;
    let s = store.snapshot()?;
    let mut x = Record::new(
        "Run 10,000 transfers with cache disabled",
        Data::Experiment {
            hypothesis: h.id.clone(),
            targets: vec![s.find(&p.id)?.frozen(), s.find(&f.id)?.frozen()],
            status: ExperimentStatus::Completed,
        },
    );
    x.body="1. Disable D-cache.\n2. Keep clock and DMA configuration fixed.\n3. Run 10,000 transfers.\n4. Record timeouts and bus load.".into();
    store.commit(vec![create(&s, x.clone())], None)?;
    let mut e = Record::new(
        "Timeout reproduced at transfer 8,142",
        Data::Evidence {
            source: "demo:run-142 (illustrative data)".into(),
            locator: "transfer 8142".into(),
            observed_at: chrono::Utc::now().to_rfc3339(),
            attachments: vec![],
        },
    );
    e.body="Illustrative observation: one timeout occurred with cache disabled. This example is synthetic.".into();
    let mut l = Record::new(
        "Cache-disabled failure contradicts hypothesis",
        Data::Link {
            from: e.id.clone(),
            to: h.id.clone(),
            relation: Relation::Contradicts,
        },
    );
    l.body = "The timeout persists when stale D-cache lines should be impossible.".into();
    let mut l2 = Record::new(
        "Failure is compatible with bus contention",
        Data::Link {
            from: e.id.clone(),
            to: alt.id.clone(),
            relation: Relation::Qualifies,
        },
    );
    l2.body = "Compatible, but bus contention has not been isolated.".into();
    let s = store.snapshot()?;
    let run = Record::new(
        "Cache-disabled run #142",
        Data::Run {
            experiment: x.id.clone(),
            plan: s.find(&x.id)?.frozen(),
            outcome: Outcome::Observed,
            evidence: vec![e.id.clone()],
        },
    );
    store.commit(
        vec![
            create(&s, e.clone()),
            create(&s, l),
            create(&s, l2),
            create(&s, run),
        ],
        None,
    )?;
    let mut a = Record::new(
        "Cache hypothesis weakened",
        Data::Assessment {
            hypothesis: h.id,
            judgment: Judgment::Weakened,
            confidence: Some(0.2),
            evidence: vec![e.id],
            criterion: Some(f.id),
            based_on: String::new(),
            supersedes: vec![],
        },
    );
    a.body="The observation contradicts the prediction. Keep the hypothesis weakened until the cache-disabled build and timing confound are independently checked.".into();
    store.commit(vec![create(&store.snapshot()?, a)], None)?;
    Ok(())
}
/// Process exit code for a failed command: 3 when a write lost a race
/// (a `Conflict`, retry after re-reading), otherwise 1.
pub fn exit_code(err: &anyhow::Error) -> i32 {
    if crate::store::Conflict::in_chain(err) {
        3
    } else {
        1
    }
}
