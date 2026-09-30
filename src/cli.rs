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
    io::{Read, Write},
    path::PathBuf,
};

#[derive(Parser)]
#[command(
    version,
    about = "Hypothesis tracking for coding and research agents: plain files in any directory, Git-friendly"
)]
pub struct Cli {
    #[arg(long, global = true, default_value = ".")]
    pub project: PathBuf,
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
#[derive(Subcommand)]
pub enum Command {
    /// Create a project, optionally populated with a firmware debugging example.
    Init {
        #[arg(long)]
        demo: bool,
        /// Also install the hyp skill for these coding agents (see `hyp agents`):
        /// claude, codex, both comma-separated, or none (the default).
        #[arg(long, value_name = "LIST", value_parser = parse_init_agents)]
        agents: Option<AgentList>,
    },
    /// Teach coding agents to use hyp: print the hyp skill, or install it
    /// as a project skill for Claude Code (.claude/skills/hyp/SKILL.md) and
    /// Codex (.agents/skills/hyp/SKILL.md).
    Agents {
        #[command(subcommand)]
        command: AgentsCommand,
    },
    /// Capture a hypothesis. Use '-' as text to read stdin.
    Add {
        title: String,
        #[arg(long, default_value = "")]
        body: String,
        #[arg(long, default_value = "")]
        scope: String,
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
    },
    Predict {
        hypothesis: String,
        title: String,
        #[arg(long, default_value = "")]
        conditions: String,
    },
    FalsifyIf {
        hypothesis: String,
        title: String,
    },
    Gap {
        hypothesis: String,
        title: String,
    },
    Evidence {
        #[command(subcommand)]
        command: EvidenceCommand,
    },
    Experiment {
        #[command(subcommand)]
        command: ExperimentCommand,
    },
    Run {
        experiment: String,
        title: String,
        #[arg(long, value_enum, default_value = "observed")]
        outcome: Outcome,
        #[arg(long, value_delimiter = ',')]
        evidence: Vec<String>,
        #[arg(long, default_value = "")]
        body: String,
    },
    Link {
        from: String,
        to: String,
        #[arg(long, value_enum)]
        relation: Relation,
        #[arg(long)]
        reason: String,
    },
    /// Record a judgment of a hypothesis, based on the state you reviewed.
    Assess {
        hypothesis: String,
        /// The review token of the hypothesis state you reviewed:
        /// .state.review_token of `hyp --json show H-…` (or `hyp --json list`).
        /// If its basis (.basis: claim, criteria, predictions, links, linked
        /// evidence, runs) or its current assessments changed since, nothing
        /// is written and the command exits 3: review the change and assess again.
        #[arg(long, value_name = "TOKEN")]
        reviewed: String,
        #[arg(long, value_enum)]
        status: Judgment,
        #[arg(long)]
        confidence: Option<f64>,
        /// Evidence already linked to the hypothesis or its criteria or
        /// predictions (`hyp link` first). Required unless --status untested.
        #[arg(long, value_delimiter = ',')]
        evidence: Vec<String>,
        #[arg(long)]
        criterion: Option<String>,
        #[arg(long)]
        reason: String,
    },
    /// Change lifecycle, experiment status, title, notes or tags.
    Set {
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        body: Option<String>,
        #[arg(long, value_enum)]
        lifecycle: Option<Lifecycle>,
        #[arg(long)]
        untestable_reason: Option<String>,
        #[arg(long, value_enum)]
        experiment_status: Option<ExperimentStatus>,
        #[arg(long, value_delimiter = ',')]
        tags: Option<Vec<String>>,
        #[arg(long)]
        resolved: Option<bool>,
    },
    /// Show a record. Plain output is a summary for people (for a
    /// hypothesis with its review token); --json is the complete form.
    Show {
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
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        needs_review: bool,
        #[arg(long)]
        archived: bool,
    },
    Search {
        query: String,
    },
    Edit {
        id: String,
    },
    Archive {
        id: String,
    },
    Restore {
        id: String,
    },
    /// Permanently delete an archived, unreferenced object.
    Delete {
        id: String,
    },
    /// Apply a JSON array of create/update/archive/delete changes from stdin.
    /// Each states what it depends on: `expected_revision`, or for creating an
    /// assessment, experiment or run an `expected` object (see the README).
    Apply {
        #[arg(long)]
        expected_revision: Option<String>,
    },
    Check {
        #[arg(long)]
        strict: bool,
    },
    Web {
        #[arg(long, default_value_t = 7432)]
        port: u16,
    },
    Graph {
        #[arg(long)]
        focus: Option<String>,
    },
    Export {
        #[arg(long,default_value="markdown",value_parser=["markdown","json","html"])]
        format: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}
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
    Add {
        hypothesis: String,
        title: String,
        #[arg(long)]
        source: String,
        #[arg(long, default_value = "")]
        locator: String,
        #[arg(long)]
        against: bool,
        #[arg(long, conflicts_with = "against")]
        qualifies: bool,
        #[arg(long, default_value = "")]
        reason: String,
        #[arg(long, default_value = "")]
        body: String,
    },
    Attach {
        id: String,
        path: PathBuf,
    },
}
#[derive(Subcommand)]
pub enum ExperimentCommand {
    Add {
        hypothesis: String,
        title: String,
        #[arg(long, value_delimiter = ',')]
        targets: Vec<String>,
        #[arg(long, default_value = "")]
        body: String,
    },
}
fn input(s: String) -> Result<String> {
    if s == "-" {
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
fn print_committed(c: &Committed, json: bool) -> Result<()> {
    print_written(&c.written, &c.snapshot, json)
}
fn ids(s: &Snapshot, ids: &[String]) -> Result<Vec<String>> {
    ids.iter()
        .map(|id| s.find(id).map(|e| e.record.id.clone()))
        .collect()
}
/// A create with its preconditions stated from `s`, the snapshot the command read.
fn create(s: &Snapshot, r: Record) -> Change {
    Change::create_seen(r, s)
}
fn hypothesis(s: &Snapshot, id: &str) -> Result<String> {
    let e = s.find(id)?;
    ensure!(
        matches!(e.record.data, Data::Hypothesis { .. }),
        "expected a hypothesis"
    );
    Ok(e.record.id.clone())
}
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
            let mut r = Record::new(
                input(title)?,
                Data::Hypothesis {
                    scope,
                    assumptions: String::new(),
                    lifecycle: Lifecycle::Draft,
                    untestable_reason: String::new(),
                },
            );
            r.body = input(body)?;
            r.tags = tags;
            changes.push(create(&s, r));
        }
        Command::Predict {
            hypothesis: h,
            title,
            conditions,
        } => changes.push(create(
            &s,
            Record::new(
                input(title)?,
                Data::Prediction {
                    hypothesis: hypothesis(&s, &h)?,
                    conditions,
                },
            ),
        )),
        Command::FalsifyIf {
            hypothesis: h,
            title,
        } => changes.push(create(
            &s,
            Record::new(
                input(title)?,
                Data::Criterion {
                    hypothesis: hypothesis(&s, &h)?,
                },
            ),
        )),
        Command::Gap {
            hypothesis: h,
            title,
        } => changes.push(create(
            &s,
            Record::new(
                input(title)?,
                Data::Gap {
                    hypothesis: hypothesis(&s, &h)?,
                    resolved: false,
                },
            ),
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
                let target = s.find(&h)?.record.id.clone();
                let mut r = Record::new(
                    input(title)?,
                    Data::Evidence {
                        source,
                        locator,
                        observed_at: chrono::Utc::now().to_rfc3339(),
                        attachments: vec![],
                    },
                );
                r.body = input(body)?;
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
                    input(reason)?
                };
                changes.extend([create(&s, r), create(&s, l)]);
            }
            EvidenceCommand::Attach { id, path } => {
                return print_committed(&store.attach(&id, &path)?, cli.json);
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
            let mut r = Record::new(
                input(title)?,
                Data::Experiment {
                    hypothesis: hypothesis(&s, &h)?,
                    targets: targets
                        .iter()
                        .map(|id| s.find(id).map(Entry::frozen))
                        .collect::<Result<_>>()?,
                    status: ExperimentStatus::Planned,
                },
            );
            r.body = input(body)?;
            changes.push(create(&s, r));
        }
        Command::Run {
            experiment,
            title,
            outcome,
            evidence,
            body,
        } => {
            let e = s.find(&experiment)?;
            let mut r = Record::new(
                input(title)?,
                Data::Run {
                    experiment: e.record.id.clone(),
                    plan: e.frozen(),
                    outcome,
                    evidence: ids(&s, &evidence)?,
                },
            );
            r.body = input(body)?;
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
                    from: s.find(&from)?.record.id.clone(),
                    to: s.find(&to)?.record.id.clone(),
                    relation,
                },
            );
            r.body = input(reason)?;
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
            ensure!(
                reviewed.len() == 64 && reviewed.bytes().all(|b| b.is_ascii_hexdigit()),
                "--reviewed must be the 64-hex-digit .state.review_token of `hyp --json show {h}`"
            );
            let state = s
                .hypotheses
                .get(&h)
                .with_context(|| format!("no derived state for hypothesis {h}"))?;
            // The token covers the agent's review up to this command's read;
            // the preconditions stated below cover this read up to the write.
            if !reviewed.eq_ignore_ascii_case(&state.review_token) {
                return Err(Conflict(format!(
                    "hypothesis {h} changed since you reviewed it (its records or current \
                     assessments; the review token differs); re-read `hyp --json show {h}`, \
                     review what changed and retry"
                ))
                .into());
            }
            let mut r = Record::new(
                format!("Assessment: {status}"),
                Data::Assessment {
                    hypothesis: h.clone(),
                    judgment: status,
                    confidence,
                    evidence: ids(&s, &evidence)?,
                    criterion: criterion
                        .map(|c| s.find(&c).map(|e| e.record.id.clone()))
                        .transpose()?,
                    based_on: String::new(),
                    supersedes: vec![],
                },
            );
            r.body = input(reason)?;
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
            if let Some(t) = title {
                r.title = input(t)?;
            }
            if let Some(b) = body {
                r.body = input(b)?;
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
            let e = s.find(&id)?;
            let mut tmp = tempfile::Builder::new().suffix(".md").tempfile()?;
            tmp.write_all(crate::store::encode(&e.record)?.as_bytes())?;
            let editor = std::env::var("VISUAL")
                .or_else(|_| std::env::var("EDITOR"))
                .unwrap_or("vi".into());
            let status = std::process::Command::new("sh")
                .arg("-c")
                .arg(format!("{editor} \"$1\""))
                .arg("hyp-edit")
                .arg(tmp.path())
                .status()?;
            ensure!(status.success(), "editor exited unsuccessfully");
            let r = crate::store::decode(&std::fs::read_to_string(tmp.path())?)?;
            ensure!(r.id == e.record.id, "cannot change ID");
            changes.push(Change::Update {
                record: r,
                expected_revision: e.revision.clone(),
            });
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
            let committed = store.commit_written(changes, expected_revision.as_deref())?;
            return print_committed(&committed, cli.json);
        }
        Command::Check { strict } => {
            if cli.json {
                print_json(&s.diagnostics)?;
            } else {
                for d in &s.diagnostics {
                    println!("{} {}: {}", d.severity, d.path, d.message);
                }
                println!("Checked {} objects", s.objects.len());
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
    print_committed(&store.commit_written(changes, None)?, cli.json)
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
        out.push_str(&format!(
            "```yaml\n{}```\n\n",
            serde_yaml::to_string(&e.record.data).unwrap_or_default()
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
