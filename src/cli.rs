use crate::{
    agents::{self, Agent, Operation},
    model::*,
    store::{self, Change, Committed, Conflict, Store, Written},
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
    /// Machine-readable output; errors as {"error": "...", "kind": "..."} on
    /// stderr.
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
/// Help for the --data argument of the commands that create or change a record.
const DATA: &str = "Data records this record draws on: D- IDs or unique prefixes, as \
                    `hyp capture` prints them; comma-separated or repeated";
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
    ///
    /// Prints the hypothesis's ID, then those of the links it creates:
    /// first one per --explains, then one per --competes-with.
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
        /// Observations the claim would explain (`hyp observe` prints their
        /// E- IDs): each gets a supports link to the new hypothesis, which
        /// makes it part of the hypothesis's basis. Comma-separated or
        /// repeated.
        #[arg(long, value_delimiter = ',', value_name = "EVIDENCE")]
        explains: Vec<String>,
        /// Why the claim would explain the observations, the reason of each
        /// --explains link (default: "Proposed as an explanation of this
        /// observation"; '-' reads stdin).
        #[arg(long, requires = "explains")]
        reason: Option<String>,
        /// Rival hypotheses: each gets a competes-with link from the new
        /// one. Comma-separated or repeated.
        #[arg(long, value_delimiter = ',', value_name = "HYPOTHESIS")]
        competes_with: Vec<String>,
        #[arg(long, value_delimiter = ',', value_name = "DATA", help = DATA)]
        data: Vec<String>,
    },
    /// Record an observation before any hypothesis explains it.
    ///
    /// Creates evidence without links and prints its E- ID. `hyp status`
    /// lists it as unexplained until a live hypothesis (not archived, not
    /// falsified) accounts for it: it counts for or qualifies that
    /// hypothesis through an active link. Add each candidate explanation with
    /// `hyp add "<claim>" --explains E-…`, or link it with `hyp link`.
    Observe {
        /// The observation. One line; '-' reads stdin: its first line is
        /// the title, the rest is appended to the body.
        title: String,
        /// Where it comes from: a file, URL, command or log.
        #[arg(long)]
        source: String,
        /// Where in the source: a line, timestamp or page.
        #[arg(long, default_value = "", hide_default_value = true)]
        locator: String,
        /// The observation in detail ('-' reads stdin).
        #[arg(long, default_value = "", hide_default_value = true)]
        body: String,
        /// When it was observed: an RFC 3339 timestamp or a date
        /// (YYYY-MM-DD), or '' for unknown. Default: now.
        #[arg(long, value_name = "WHEN", value_parser = parse_observed_at)]
        observed_at: Option<String>,
        #[arg(long, value_delimiter = ',', value_name = "DATA", help = DATA)]
        data: Vec<String>,
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
        #[arg(long, value_delimiter = ',', value_name = "DATA", help = DATA)]
        data: Vec<String>,
    },
    /// Add a falsification criterion: what would refute the hypothesis.
    FalsifyIf {
        #[arg(help = HYPOTHESIS)]
        hypothesis: String,
        /// The refuting observation. One line; '-' reads stdin: its first
        /// line is the title, the rest is appended to the body.
        title: String,
        #[arg(long, value_delimiter = ',', value_name = "DATA", help = DATA)]
        data: Vec<String>,
    },
    /// Note a gap: an open question about a hypothesis.
    Gap {
        #[arg(help = HYPOTHESIS)]
        hypothesis: String,
        #[arg(help = TITLE)]
        title: String,
        #[arg(long, value_delimiter = ',', value_name = "DATA", help = DATA)]
        data: Vec<String>,
    },
    /// Copy a file's bytes (or stdin's) into the project as a data record.
    ///
    /// Prints the new record's D- ID. The bytes are stored once per SHA-256
    /// under hyp/assets/, at most 32 MiB; the record (title, origin, media
    /// type, size, sha256, the time of capture) cannot be changed later.
    /// Name it from any record with --data D-…. Repeating a capture of the
    /// same bytes with the same title, origin, media type and note prints
    /// the existing ID and writes nothing.
    Capture {
        /// The file to copy, or '-' for stdin (`cmd | hyp capture - --origin cmd`).
        file: String,
        /// Where the bytes came from: a path, URL, host, or the command that
        /// produced them (hyp never runs it).
        #[arg(long)]
        origin: String,
        /// One line (default: the file name; for stdin, the origin's first line).
        #[arg(long)]
        title: Option<String>,
        /// type/subtype (default: guessed from the file name, else text/plain
        /// for UTF-8 text and application/octet-stream for anything else).
        #[arg(long, value_name = "TYPE")]
        media_type: Option<String>,
        /// A note about the data ('-' reads stdin, unless FILE is '-').
        #[arg(long, default_value = "", hide_default_value = true)]
        body: String,
        /// Record empty data (0 bytes); without it an empty capture is an
        /// error, as an empty pipe usually means the command before it failed.
        #[arg(long)]
        allow_empty: bool,
    },
    /// Read a data record's bytes.
    Data {
        #[command(subcommand)]
        command: DataCommand,
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
        #[arg(long, value_enum, default_value_t = new_outcome())]
        outcome: Outcome,
        /// Comma-separated E- IDs of the evidence it produced.
        #[arg(long, value_delimiter = ',')]
        evidence: Vec<String>,
        /// What was done and seen ('-' reads stdin).
        #[arg(long, default_value = "", hide_default_value = true)]
        body: String,
        /// The revision of the experiment as you reviewed it: the 12 hex
        /// digits after "revision" on the first line of `hyp show X-…`
        /// (.entry.revision with --json, whole or its first 12 or more hex
        /// digits). If the experiment changed since, nothing is written and
        /// the command exits 3. Without it, the plan is frozen as this
        /// command reads it.
        #[arg(long, value_name = "REVISION")]
        reviewed: Option<String>,
        #[arg(long, value_delimiter = ',', value_name = "DATA", help = DATA)]
        data: Vec<String>,
    },
    /// Link evidence to a claim, or relate two hypotheses.
    ///
    /// Evidence linked to an active criterion or prediction counts toward its
    /// hypothesis like evidence linked to the hypothesis itself: it is part of
    /// the hypothesis's basis and an assessment of it may cite it.
    Link {
        /// The E- evidence, or the H- hypothesis the relation starts at.
        from: String,
        /// The H-, P- or F- record it bears on, or the other H- hypothesis.
        /// Evidence that supports a criterion F-… meets it, which counts
        /// against the hypothesis.
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
        /// The review token of the hypothesis state you reviewed: the 12 hex
        /// digits after "review" on the first line of `hyp show H-…`
        /// (.state.review_token with --json, whole or its first 12 or more
        /// hex digits). If the basis (claim, criteria, predictions, links,
        /// linked evidence, runs) or the current assessments changed since,
        /// nothing is written and the command exits 3: review the change and
        /// assess again.
        #[arg(long, value_name = "TOKEN")]
        reviewed: String,
        /// The judgment; each needs --reason and what its value below says
        /// (--evidence, --criterion).
        #[arg(long, value_parser = PossibleValuesParser::new(judgment_values())
            .map(|s| Judgment::from_str(&s, false).expect("a value from judgment_values")))]
        status: Judgment,
        /// Subjective confidence in the judgment, from 0.0 to 1.0 (not a
        /// percentage).
        #[arg(long, value_parser = parse_confidence)]
        confidence: Option<f64>,
        /// Evidence already linked to the hypothesis or its criteria or
        /// predictions (`hyp link` first). Required unless --status untested.
        #[arg(long, value_delimiter = ',')]
        evidence: Vec<String>,
        /// The F- criterion the evidence meets (required for falsified: a
        /// cited evidence must have a supports link to it).
        #[arg(long)]
        criterion: Option<String>,
        /// The rationale ('-' reads stdin).
        #[arg(long)]
        reason: String,
        #[arg(long, value_delimiter = ',', value_name = "DATA", help = DATA)]
        data: Vec<String>,
    },
    /// Change title, body, tags, lifecycle, status, data; resolve a gap.
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
        /// Whether a gap is resolved; false also forgets --by.
        #[arg(long)]
        resolved: Option<bool>,
        /// The evidence that resolved a gap: E- IDs or unique prefixes,
        /// comma-separated or repeated, replacing earlier ones. The gap must
        /// be resolved (with --resolved true, or already).
        #[arg(long, value_delimiter = ',', value_name = "EVIDENCE")]
        by: Vec<String>,
        /// When evidence was observed: an RFC 3339 timestamp or a date
        /// (YYYY-MM-DD), or '' for unknown.
        #[arg(long, value_name = "WHEN", value_parser = parse_observed_at)]
        observed_at: Option<String>,
        /// Data records the record draws on: D- IDs or unique prefixes,
        /// comma-separated or repeated, replacing earlier ones. Not for
        /// assessments, runs or data records, which cannot change.
        #[arg(long, value_delimiter = ',', value_name = "DATA")]
        data: Vec<String>,
    },
    /// Where the investigation stands: start here when resuming work.
    ///
    /// Per open hypothesis (not archived, not closed), and per closed one
    /// that needs review: its judgment, whether it needs review, a missing
    /// criterion or its untestable reason, how much evidence is linked, open
    /// gaps and experiments without runs. First, what blocks writes; last,
    /// unexplained observations: an observation is unexplained until a live
    /// hypothesis (not archived, not falsified) accounts for it, i.e. it
    /// counts for or qualifies that hypothesis through an active link. No
    /// review token: take that from `hyp show H-…`, the output you review
    /// before assessing.
    Status,
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
    /// on records the batch does not create, an `expected` object. A create
    /// may name its record "@name" for later changes of the batch.
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
  {"op": "patch",   "id": ID, "expected_revision": REV, "set": {FIELD: VALUE}}
  {"op": "update",  "record": RECORD, "expected_revision": REV}
  {"op": "archive", "id": ID, "archived": true, "expected_revision": REV}
  {"op": "delete",  "id": ID, "expected_revision": REV}

RECORD is a record as `hyp --json show ID` prints it (.entry.record): "kind",
"title", optional "body" and "tags", and the fields of its kind. A create
needs what the matching command asks for; the rest gets its defaults (a
hypothesis is a draft, an experiment is planned and targets its hypothesis, a
run is observed, a gap open) and the server sets the times. A patch sets only
the fields in "set" and keeps the others; it cannot change "id", "kind" or
"created_at". An update gives the whole record as read, changed. Assessments,
runs and data records cannot be changed. REV is .entry.revision of `hyp --json show ID`,
or a revision a --json write printed. "archived": false restores. Use full
IDs, not prefixes, or references.

References: a create may set "id": "@name" (letters, digits, - and _), and
later changes in the batch write "@name" in record fields that take an ID
("hypothesis", "from", "to", "experiment", "evidence", "criterion", target
"id"s, "resolved_by", "data") and in "expected" keys; hyp replaces it with the full ID
it generates. With --json, "written" lists the full IDs in change order, a
create's "ref" with it. Records the batch creates need no statement in
"expected", and cannot also be patched, updated, archived or deleted in it:
put their values in the create. A hypothesis, its criterion, an experiment
and a run with its evidence, in one batch:
  [{"op": "create", "record": {"id": "@h", "kind": "hypothesis",
     "title": "The cache causes the timeouts"}},
   {"op": "create", "record": {"id": "@f", "kind": "criterion",
     "hypothesis": "@h", "title": "Timeouts persist with the cache off"}},
   {"op": "create", "record": {"id": "@x", "kind": "experiment",
     "hypothesis": "@h", "title": "Disable the cache", "targets": [{"id": "@f"}]}},
   {"op": "create", "record": {"id": "@e", "kind": "evidence",
     "title": "3/500 transfers time out, cache off", "source": "bench.log"}},
   {"op": "create", "record": {"kind": "link", "title": "Criterion met",
     "from": "@e", "to": "@f", "relation": "supports", "body": "Why …"}},
   {"op": "create", "record": {"kind": "run", "title": "Run 1",
     "experiment": "@x", "evidence": ["@e"]}}]

Retitle a hypothesis, archive a prediction:
  [{"op": "patch", "id": "H-…", "expected_revision": "…",
    "set": {"title": "The L2 cache causes the timeouts"}},
   {"op": "archive", "id": "P-…", "archived": true, "expected_revision": "…"}]

Creating an assessment, experiment or run states in "expected" what it was
based on; if that changed since, nothing is written and hyp exits 3. An
assessment states its hypothesis's .state.review_token (`hyp --json show H-…`)
and cites only evidence linked to the hypothesis or its criteria or
predictions; every judgment except untested cites some, and falsified also a
"criterion": "F-…" that one of them meets (links to with "supports"):
  [{"op": "create",
    "record": {"kind": "assessment", "title": "Weakened", "body": "Why …",
               "hypothesis": "H-…", "judgment": "weakened",
               "confidence": 0.3, "evidence": ["E-…"]},
    "expected": {"hypotheses": {"H-…": {"review_token": "…"}}}}]

Any record but a data record may name data records: "data": ["D-…"]. A data
record ("kind": "data", "title", "origin", "sha256", optional "media_type")
describes bytes hyp already stores (`hyp capture` stored them); the server sets
its "captured_at" and "size".

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
/// The judgments, each with its `Judgment::requirement` as help.
fn judgment_values() -> Vec<PossibleValue> {
    Judgment::value_variants()
        .iter()
        .filter_map(|j| j.to_possible_value().map(|v| v.help(j.requirement())))
        .collect()
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
    ///
    /// Evidence linked to an active criterion or prediction counts toward its
    /// hypothesis like evidence linked to the hypothesis itself: it is part of
    /// the hypothesis's basis and an assessment of it may cite it. No second
    /// link to the hypothesis is needed. `hyp show H-…` lists what each link
    /// means for the hypothesis.
    Add {
        /// The H-, P- or F- record it bears on: an ID or unique prefix. On a
        /// criterion F-…, supporting it (the default) means the observation
        /// meets the criterion, which counts against the hypothesis; --against
        /// means it does not meet it.
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
        /// When it was observed: an RFC 3339 timestamp or a date
        /// (YYYY-MM-DD), or '' for unknown. Default: now.
        #[arg(long, value_name = "WHEN", value_parser = parse_observed_at)]
        observed_at: Option<String>,
        #[arg(long, value_delimiter = ',', value_name = "DATA", help = DATA)]
        data: Vec<String>,
    },
    /// Capture a file as a data record and reference it from evidence.
    ///
    /// Prints the evidence's ID, then the data record's. Reuses a data
    /// record the evidence references, or any with the same bytes, before
    /// capturing a new one (title: the file name; origin: the path as given).
    /// Evidence that holds the bytes already is left as it is ("no
    /// changes"); if it holds them as an attachment of hyp 0.2.0, only its
    /// ID is printed.
    Attach {
        /// The evidence: an E- ID or unique prefix.
        id: String,
        /// The file (at most 32 MiB).
        path: PathBuf,
    },
}
#[derive(Subcommand)]
pub enum DataCommand {
    /// Write a data record's bytes to stdout, or to a file.
    ///
    /// Fails, writing nothing, if the stored bytes are missing or no longer
    /// match the record's sha256 (hyp check reports that too).
    Get {
        /// The data record: a D- ID or unique prefix.
        id: String,
        /// Write to this file instead of stdout (replacing it).
        #[arg(long, value_name = "FILE")]
        output: Option<PathBuf>,
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
        /// The review token of the hypothesis as you reviewed it: the 12 hex
        /// digits after "review" on the first line of `hyp show H-…`. It
        /// covers the claim, criteria and predictions this freezes as
        /// targets, but also the rest of the basis (linked evidence, links,
        /// runs) and the current assessments: if any of it changed since,
        /// nothing is written and the command exits 3; re-read and retry.
        /// Without it, the targets are frozen as this command reads them.
        #[arg(long, value_name = "TOKEN")]
        reviewed: Option<String>,
    },
}
/// A `--observed-at` value: an RFC 3339 timestamp or a date (YYYY-MM-DD),
/// kept as given (`is_observed_at`), or empty for unknown. A clap value
/// parser, so anything else is an argument error (exit 2). The write
/// applies the same rule for every writer, and refuses a value in the
/// future (`observed_at_refusal`, exit 1).
fn parse_observed_at(value: &str) -> Result<String, String> {
    if value.is_empty() || is_observed_at(value) {
        Ok(value.to_string())
    } else {
        Err(
            "expected an RFC 3339 timestamp (2026-09-12T14:03:00Z), a date (2026-09-12), \
             or '' for unknown"
                .into(),
        )
    }
}
/// A `--reviewed` value: 12 to 64 hex digits, lowercased, or an error
/// saying it must be `what`. A prefix of 12 hex digits (48 bits) is ample to
/// detect a change.
fn reviewed_hex(value: &str, what: String) -> Result<String> {
    let value = value.to_ascii_lowercase();
    ensure!(
        (12..=64).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_hexdigit()),
        "--reviewed must be {what}"
    );
    Ok(value)
}
/// The `--reviewed` review token of hypothesis `h` (`reviewed_hex`).
fn review_token(h: &str, value: &str) -> Result<String> {
    reviewed_hex(
        value,
        format!(
            "the review token that `hyp show {h}` prints after \"review\" on its first line, \
             or the first 12 or more hex digits of .state.review_token of `hyp --json show {h}`"
        ),
    )
}
/// Fails with a `Conflict` unless hypothesis `h`'s review token in `s`
/// starts with `reviewed`: the agent's review covers the time up to this
/// command's read, and the preconditions the command states cover the time
/// from it to the write. `again` says what to do after re-reading.
fn unchanged_since_review(s: &Snapshot, h: &str, reviewed: &str, again: &str) -> Result<()> {
    let state = s
        .hypotheses
        .get(h)
        .with_context(|| format!("no derived state for hypothesis {h}"))?;
    if state
        .review_token
        .to_ascii_lowercase()
        .starts_with(reviewed)
    {
        return Ok(());
    }
    Err(Conflict::on(
        vec![h.to_string()],
        format!(
            "hypothesis {h} changed since you reviewed it (its basis or its current \
             assessments; the review token differs), so nothing was written. \
             `hyp show {h}` lists its basis now (criteria, predictions, linked \
             evidence, runs, links to other hypotheses) and its current \
             assessments: compare them with what you reviewed, then {again} \
             with the review token it prints"
        ),
    )
    .into())
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
        Command::Add {
            title,
            body,
            reason,
            ..
        } => vec![
            (TITLE, Some(title)),
            ("--body", Some(body)),
            ("--reason", reason.as_ref()),
        ],
        Command::Run { title, body, .. }
        | Command::Observe { title, body, .. }
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
        Command::Capture { file, body, .. } => vec![("FILE", Some(file)), ("--body", Some(body))],
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
/// project revision, all after the write; plus `"migrated"`, in the same
/// form, when the write also converted legacy attachments (the data records
/// it created and the evidence it changed). Plain output lists only
/// `written`; `report` names the converted records on stderr.
fn print_written(
    written: &[Written],
    migrated: &[Written],
    after: &Snapshot,
    json: bool,
) -> Result<()> {
    if json {
        let mut out = serde_json::json!({"written": written, "revision": after.revision});
        if !migrated.is_empty() {
            out["migrated"] = serde_json::json!(migrated);
        }
        return print_json(&out);
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
            Change::Update { .. } | Change::Patch { .. } => Self::Updated,
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
    print_written(&c.written, &c.migrated, &c.snapshot, json)?;
    // The write changed records the command did not name: say which.
    if !json && !c.migrated.is_empty() {
        let of = |kind: Kind| -> Vec<&str> {
            c.migrated
                .iter()
                .filter(|w| w.kind == kind)
                .map(|w| w.id.as_str())
                .collect()
        };
        let raised = c.raised_to.map_or(String::new(), |n| {
            format!("raised the notebook to schema {n}; ")
        });
        let created = match of(Kind::Data).as_slice() {
            [] => String::new(),
            ids => format!("; created data records {}", ids.join(", ")),
        };
        eprintln!(
            "{raised}converted the attachments of evidence {} into data references{created}",
            of(Kind::Evidence).join(", ")
        );
    }
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
        // "evidence" and "data" are mass nouns: "expected evidence", not "an evidence".
        let a = match kinds[0] {
            Kind::Evidence | Kind::Data => String::new(),
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
/// `ids` without repeats, in first-seen order: an ID given twice, in full
/// or by two prefixes, gets one link.
fn distinct(ids: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    ids.into_iter()
        .filter(|id| seen.insert(id.clone()))
        .collect()
}
/// The reason of `hyp add --competes-with`'s link to `rival`: the
/// observations among `explains` that `rival` explains too, by full ID.
fn rival_reason(s: &Snapshot, rival: &str, explains: &[String]) -> String {
    let linked = s.linked_evidence(rival);
    let shared: Vec<&str> = explains
        .iter()
        .filter(|e| linked.contains(e.as_str()))
        .map(String::as_str)
        .collect();
    if shared.is_empty() {
        "Competing explanation".into()
    } else {
        format!("Competing explanations of {}", shared.join(", "))
    }
}
/// A create with its preconditions stated from `s`, the snapshot the command read.
fn create(s: &Snapshot, r: Record) -> Change {
    Change::create_seen(r, s)
}
fn hypothesis(s: &Snapshot, id: &str) -> Result<String> {
    find_kind(s, "<HYPOTHESIS>", id, &[Kind::Hypothesis])
}
/// The full IDs of the data records `--data` names, each once.
fn data_refs(s: &Snapshot, ids: &[String]) -> Result<Vec<String>> {
    Ok(distinct(find_all(s, "--data", ids, &[Kind::Data])?))
}
/// `r` referencing the data records `--data` names.
fn with_data(s: &Snapshot, mut r: Record, ids: &[String]) -> Result<Record> {
    r.data_refs = data_refs(s, ids)?;
    Ok(r)
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
            print_written(&written, &[], &s, true)?;
        } else {
            println!("Initialized {}", store.root.display());
            print_steps(&steps, false)?;
            if *demo {
                println!(
                    "Next: hyp status shows where the example investigation stands, \
                     hyp show H-… one hypothesis, hyp web all of it"
                );
            } else {
                println!(
                    "Next: hyp add \"<claim>\", then hyp falsify-if H-… \"<what would refute it>\" \
                     (or first hyp observe \"<what you saw>\" --source …, then hyp add \"<claim>\" \
                     --explains E-…); hyp agents install teaches coding agents to use hyp"
                );
            }
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
    // `hyp check` also hashes every stored byte; other commands check stored
    // bytes by metadata only (`store::Verify`, HYPO-0004).
    let s = match cli.command {
        Command::Check { .. } => store.read(store::Verify::Content)?,
        _ => store.snapshot()?,
    };
    let mut changes = Vec::new();
    match cli.command {
        Command::Add {
            title,
            body,
            scope,
            tags,
            explains,
            reason,
            competes_with,
            data,
        } => {
            let explains = distinct(find_all(&s, "--explains", &explains, &[Kind::Evidence])?);
            let rivals = distinct(find_all(
                &s,
                "--competes-with",
                &competes_with,
                &[Kind::Hypothesis],
            )?);
            let mut h = titled(
                title,
                body,
                Data::Hypothesis {
                    scope,
                    assumptions: String::new(),
                    lifecycle: new_lifecycle(),
                    untestable_reason: String::new(),
                },
            )?;
            h.tags = tags;
            h.data_refs = data_refs(&s, &data)?;
            let reason = match reason {
                Some(reason) => input("--reason", reason)?,
                None => "Proposed as an explanation of this observation".into(),
            };
            let mut links = Vec::new();
            for e in &explains {
                let mut l = Record::new(
                    format!("{} explains {}", short(&h.id), short(e)),
                    Data::Link {
                        from: e.clone(),
                        to: h.id.clone(),
                        relation: Relation::Supports,
                    },
                );
                l.body.clone_from(&reason);
                links.push(l);
            }
            for rival in &rivals {
                let mut l = Record::new(
                    format!("{} competes with {}", short(&h.id), short(rival)),
                    Data::Link {
                        from: h.id.clone(),
                        to: rival.clone(),
                        relation: Relation::CompetesWith,
                    },
                );
                l.body = rival_reason(&s, rival, &explains);
                links.push(l);
            }
            changes.push(create(&s, h));
            changes.extend(links.into_iter().map(|l| create(&s, l)));
        }
        Command::Observe {
            title,
            source,
            locator,
            body,
            observed_at,
            data,
        } => changes.push(create(
            &s,
            with_data(
                &s,
                titled(
                    title,
                    body,
                    Data::Evidence {
                        source,
                        locator,
                        observed_at: observed_at.unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
                        attachments: vec![],
                    },
                )?,
                &data,
            )?,
        )),
        Command::Predict {
            hypothesis: h,
            title,
            conditions,
            data,
        } => {
            let data_kind = Data::Prediction {
                hypothesis: hypothesis(&s, &h)?,
                conditions,
            };
            let r = titled(title, String::new(), data_kind)?;
            changes.push(create(&s, with_data(&s, r, &data)?));
        }
        Command::FalsifyIf {
            hypothesis: h,
            title,
            data,
        } => {
            let data_kind = Data::Criterion {
                hypothesis: hypothesis(&s, &h)?,
            };
            let r = titled(title, String::new(), data_kind)?;
            changes.push(create(&s, with_data(&s, r, &data)?));
        }
        Command::Gap {
            hypothesis: h,
            title,
            data,
        } => {
            let data_kind = Data::Gap {
                hypothesis: hypothesis(&s, &h)?,
                resolved: false,
                resolved_by: vec![],
            };
            let r = titled(title, String::new(), data_kind)?;
            changes.push(create(&s, with_data(&s, r, &data)?));
        }
        Command::Capture {
            file,
            origin,
            title,
            media_type,
            body,
            allow_empty,
        } => {
            let (bytes, name) = if file == "-" {
                (store::read_capped(std::io::stdin(), "stdin")?, None)
            } else {
                let path = PathBuf::from(&file);
                let f = std::fs::File::open(&path)
                    .with_context(|| format!("cannot read {}", path.display()))?;
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| file.clone());
                (store::read_capped(f, &file)?, Some(name))
            };
            let title = match title {
                Some(t) => t,
                None => name.clone().unwrap_or_else(|| {
                    let first = origin.trim().lines().next().unwrap_or_default();
                    first.chars().take(120).collect()
                }),
            };
            let capture = store::Capture {
                bytes,
                title,
                origin,
                media_type,
                name,
                note: input("--body", body)?,
                allow_empty,
            };
            return report(&[Action::Created], &store.capture(capture)?, cli.json);
        }
        Command::Data {
            command: DataCommand::Get { id, output },
        } => {
            let id = find_kind(&s, "<ID>", &id, &[Kind::Data])?;
            let Some(Data::Captured { sha256, .. }) = s.get(&id).map(|e| &e.record.data) else {
                unreachable!("find_kind returned a data record");
            };
            let bytes = store
                .blob(sha256)
                .with_context(|| format!("cannot read the bytes of {id} (see hyp check)"))?;
            match output {
                Some(path) => write_outside(&store, &path, &bytes)?,
                None => {
                    use std::io::Write;
                    let mut out = std::io::stdout().lock();
                    out.write_all(&bytes)?;
                    out.flush()?;
                }
            }
            return Ok(());
        }
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
                observed_at,
                data,
            } => {
                let target = find_kind(&s, "<HYPOTHESIS>", &h, &CLAIM)?;
                let mut r = titled(
                    title,
                    body,
                    Data::Evidence {
                        source,
                        locator,
                        observed_at: observed_at.unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
                        attachments: vec![],
                    },
                )?;
                r.data_refs = data_refs(&s, &data)?;
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
                let actions = [Action::Updated, Action::Created];
                return report(&actions, &store.attach(&id, &path)?, cli.json);
            }
        },
        Command::Experiment {
            command:
                ExperimentCommand::Add {
                    hypothesis: h,
                    title,
                    targets,
                    body,
                    reviewed,
                },
        } => {
            let hypothesis = hypothesis(&s, &h)?;
            // The targets are the hypothesis and its own criteria and
            // predictions (`validate` rejects others), whose content its
            // review token covers.
            if let Some(reviewed) = reviewed {
                let reviewed = review_token(&hypothesis, &reviewed)?;
                unchanged_since_review(&s, &hypothesis, &reviewed, "plan the experiment again")?;
            }
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
                    status: new_experiment_status(),
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
            reviewed,
            data,
        } => {
            let e = s.find(&find_kind(
                &s,
                "<EXPERIMENT>",
                &experiment,
                &[Kind::Experiment],
            )?)?;
            if let Some(reviewed) = reviewed {
                let x = &e.record.id;
                let reviewed = reviewed_hex(
                    &reviewed,
                    format!(
                        "the revision that `hyp show {x}` prints after \"revision\" on its \
                         first line, or the first 12 or more hex digits of .entry.revision of \
                         `hyp --json show {x}`"
                    ),
                )?;
                if !e.revision.starts_with(&reviewed) {
                    return Err(Conflict::on(
                        vec![x.clone()],
                        format!(
                            "experiment {x} changed since you reviewed it (its revision \
                             differs), so nothing was written. `hyp show {x}` shows it now: \
                             compare its plan with what you ran, then record the run again \
                             with the revision it prints"
                        ),
                    )
                    .into());
                }
            }
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
            changes.push(create(&s, with_data(&s, r, &data)?));
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
            data,
        } => {
            let h = hypothesis(&s, &h)?;
            let reviewed = review_token(&h, &reviewed)?;
            confidence_in_range(confidence)?;
            unchanged_since_review(&s, &h, &reviewed, "assess again")?;
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
            r.data_refs = data_refs(&s, &data)?;
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
            by,
            observed_at,
            data,
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
                if let Data::Gap {
                    resolved,
                    resolved_by,
                    ..
                } = &mut r.data
                {
                    *resolved = value;
                    if !value {
                        resolved_by.clear();
                    }
                } else {
                    anyhow::bail!("resolved only applies to gaps");
                }
            }
            if !by.is_empty() {
                let Data::Gap {
                    resolved,
                    resolved_by,
                    ..
                } = &mut r.data
                else {
                    anyhow::bail!("--by only applies to gaps");
                };
                ensure!(
                    *resolved,
                    "--by names the evidence that resolved gap {}, which is open: \
                     add --resolved true",
                    e.record.id
                );
                *resolved_by = find_all(&s, "--by", &by, &[Kind::Evidence])?;
            }
            if let Some(when) = observed_at {
                let Data::Evidence { observed_at, .. } = &mut r.data else {
                    anyhow::bail!("--observed-at only applies to evidence");
                };
                *observed_at = when;
            }
            if !data.is_empty() {
                r.data_refs = data_refs(&s, &data)?;
            }
            changes.push(Change::Update {
                record: r,
                expected_revision: e.revision.clone(),
            });
        }
        Command::Status => {
            let report = crate::status::report(&s);
            if cli.json {
                print_json(&report)?;
            } else {
                println!("{}", crate::status::plain(&report));
            }
            return Ok(());
        }
        Command::Show { id } => {
            let e = s.find(&id)?;
            if cli.json {
                let related: Vec<&Entry> = s
                    .objects
                    .iter()
                    .filter(|x| x.record.references().contains(&e.record.id.as_str()))
                    .collect();
                // For a hypothesis, what its fingerprint hashes (`basis`), plus
                // in full the runs and evidence in it, which `related` does not
                // reach (a link or run names only IDs). Each evidence entry
                // also says what it means for the hypothesis: its `stance`
                // and the `bearings` of its links (`Snapshot::evidence_bearings`);
                // evidence only a run cites has none (null, []).
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
                        let bearings = s.evidence_bearings(&e.record.id);
                        let evidence: Vec<serde_json::Value> = of_kind("evidence")
                            .into_iter()
                            .map(|x| {
                                let b = bearings.iter().find(|b| b.evidence == x.record.id);
                                let mut entry = serde_json::json!(x);
                                entry["stance"] = serde_json::json!(b.map(|b| b.stance));
                                entry["bearings"] = serde_json::json!(
                                    b.map(|b| b.bearings.as_slice()).unwrap_or_default()
                                );
                                entry
                            })
                            .collect();
                        let runs = of_kind("run");
                        (Some(basis), Some(runs), Some(evidence))
                    }
                    _ => (None, None, None),
                };
                // For evidence, each hypothesis it bears on with its
                // `stance` and `bearings` (`Snapshot::bears_on`), and whether
                // no live hypothesis accounts for it (`Snapshot::unexplained`).
                let (bears_on, unexplained) = match e.record.data {
                    Data::Evidence { .. } => {
                        let bears_on: Vec<serde_json::Value> = s
                            .bears_on(&e.record.id)
                            .into_iter()
                            .map(|(h, b)| {
                                serde_json::json!({
                                    "hypothesis": h.record.id,
                                    "archived": h.record.archived,
                                    "judgment": s.hypotheses.get(&h.record.id).map(|x| x.judgment),
                                    "stance": b.stance,
                                    "bearings": b.bearings,
                                })
                            })
                            .collect();
                        let unexplained =
                            s.unexplained().iter().any(|x| x.record.id == e.record.id);
                        (Some(bears_on), Some(unexplained))
                    }
                    _ => (None, None),
                };
                print_json(&serde_json::json!({
                    "entry": e,
                    "state": s.hypotheses.get(&e.record.id),
                    "related": related,
                    "runs": runs,
                    "evidence": evidence,
                    "basis": basis,
                    "bears_on": bears_on,
                    "unexplained": unexplained,
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
            let changes: Vec<Change> = serde_json::from_str(&raw).context(
                "invalid changes (besides the fields of its kind, every record takes title, \
                 body, tags and archived; see hyp apply --help)",
            )?;
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
            let count = |severity: &str| {
                s.diagnostics
                    .iter()
                    .filter(|d| d.severity == severity)
                    .count()
            };
            let (errors, warnings) = (count("error"), count("warning"));
            let plural = |n: usize, what: &str| match n {
                1 => format!("1 {what}"),
                n => format!("{n} {what}s"),
            };
            let found = match (errors, strict && warnings > 0) {
                (0, false) => return Ok(()),
                (0, true) => plural(warnings, "warning"),
                (_, false) => plural(errors, "error"),
                (_, true) => format!(
                    "{} and {}",
                    plural(errors, "error"),
                    plural(warnings, "warning")
                ),
            };
            anyhow::bail!(crate::error::Classified::new(
                crate::error::ErrorKind::CheckFailed,
                format!("hyp check found {found} (listed on stdout)"),
            ));
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
/// Writes `bytes` to `path` atomically (a temporary file, then a rename),
/// refusing a path inside the notebook's `hyp/` directory, which only hyp
/// writes, record by record, or inside `.hyp/`, whose journal hyp would
/// replay as writes.
fn write_outside(store: &Store, path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    let name = path
        .file_name()
        .with_context(|| format!("--output {} names no file", path.display()))?;
    let parent = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => std::path::Path::new("."),
    };
    let parent = parent
        .canonicalize()
        .with_context(|| format!("cannot write {}: no such directory", path.display()))?;
    for own in ["hyp", ".hyp"] {
        ensure!(
            !parent.starts_with(store.root.join(own)),
            "refusing to write {} inside the notebook's {own}/ directory, which only hyp writes",
            path.display()
        );
    }
    store::atomic_readable(&parent.join(name), bytes)
        .with_context(|| format!("cannot write {}", path.display()))
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
            || e.record.references().iter().any(|id| Some(*id) == focus)
        {
            selected.insert(e.record.id.clone());
            selected.extend(e.record.references().into_iter().map(str::to_string));
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
        if !e.record.data_refs.is_empty() {
            data["data"] = serde_yaml::to_value(&e.record.data_refs).unwrap_or_default();
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
            resolved_by: vec![],
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
    if Conflict::in_chain(err) { 3 } else { 1 }
}
