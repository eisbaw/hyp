use crate::{
    model::*,
    store::{Change, Store},
    web,
};
use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use std::{
    io::{Read, Write},
    path::PathBuf,
};

#[derive(Parser)]
#[command(
    version,
    about = "Git-native hypothesis tracking for humans and agents"
)]
pub struct Cli {
    #[arg(long, global = true, default_value = ".")]
    pub project: PathBuf,
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Command,
}
#[derive(Subcommand)]
pub enum Command {
    /// Create a project, optionally populated with a firmware debugging example.
    Init {
        #[arg(long)]
        demo: bool,
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
    Assess {
        hypothesis: String,
        #[arg(long, value_enum)]
        status: Judgment,
        #[arg(long)]
        confidence: Option<f64>,
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
    Show {
        id: String,
    },
    List {
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        status: Option<String>,
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
    /// Apply a JSON array of create/update/archive/delete operations from stdin.
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
fn ids(s: &Snapshot, ids: &[String]) -> Result<Vec<String>> {
    ids.iter()
        .map(|id| s.find(id).map(|e| e.record.id.clone()))
        .collect()
}
fn create(r: Record) -> Change {
    Change::Create { record: r }
}
fn hypothesis(s: &Snapshot, id: &str) -> Result<String> {
    let e = s.find(id)?;
    ensure!(
        matches!(e.record.data, Data::Hypothesis { .. }),
        "expected a hypothesis"
    );
    Ok(e.record.id.clone())
}
pub async fn run(cli: Cli) -> Result<()> {
    if let Command::Init { demo } = cli.command {
        let store = Store::init(&cli.project)?;
        if demo {
            seed_demo(&store)?;
        }
        if cli.json {
            print_json(&store.snapshot()?)?;
        } else {
            println!("Initialized {}", store.root.display());
        }
        return Ok(());
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
            changes.push(create(r));
        }
        Command::Predict {
            hypothesis: h,
            title,
            conditions,
        } => changes.push(create(Record::new(
            input(title)?,
            Data::Prediction {
                hypothesis: hypothesis(&s, &h)?,
                conditions,
            },
        ))),
        Command::FalsifyIf {
            hypothesis: h,
            title,
        } => changes.push(create(Record::new(
            input(title)?,
            Data::Criterion {
                hypothesis: hypothesis(&s, &h)?,
            },
        ))),
        Command::Gap {
            hypothesis: h,
            title,
        } => changes.push(create(Record::new(
            input(title)?,
            Data::Gap {
                hypothesis: hypothesis(&s, &h)?,
                resolved: false,
            },
        ))),
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
                changes.extend([create(r), create(l)]);
            }
            EvidenceCommand::Attach { id, path } => {
                let result = store.attach(&id, &path)?;
                if cli.json {
                    print_json(&result)?;
                } else {
                    println!("Attached {}", path.display());
                }
                return Ok(());
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
            changes.push(create(r));
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
            changes.push(create(r));
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
            changes.push(create(r));
        }
        Command::Assess {
            hypothesis: h,
            status,
            confidence,
            evidence,
            criterion,
            reason,
        } => {
            let h = hypothesis(&s, &h)?;
            let mut r = Record::new(
                format!("Assessment: {status}"),
                Data::Assessment {
                    hypothesis: h,
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
            changes.push(create(r));
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
                print_json(
                    &serde_json::json!({"entry":e,"state":s.hypotheses.get(&e.record.id),"related":s.objects.iter().filter(|x|x.record.data.references().contains(&e.record.id.as_str())).collect::<Vec<_>>()}),
                )?;
            } else {
                println!("{}", crate::store::encode(&e.record)?);
                if let Some(state) = s.hypotheses.get(&e.record.id) {
                    println!(
                        "\nAssessment: {}{}",
                        state.judgment,
                        if state.needs_review {
                            " (needs review)"
                        } else {
                            ""
                        }
                    );
                }
                for x in &s.objects {
                    if x.record.data.references().contains(&e.record.id.as_str()) {
                        println!(
                            "  {}  {}  {}",
                            x.record.id,
                            x.record.data.kind(),
                            x.record.title
                        );
                    }
                }
            }
            return Ok(());
        }
        Command::List {
            kind,
            status,
            tag,
            needs_review,
            archived,
        } => {
            let rows: Vec<_> = s
                .objects
                .iter()
                .filter(|e| {
                    let r = &e.record;
                    let state = s.hypotheses.get(&r.id);
                    (archived || !r.archived)
                        && kind.as_ref().is_none_or(|k| r.data.kind() == k)
                        && tag.as_ref().is_none_or(|t| r.tags.contains(t))
                        && (!needs_review || state.is_some_and(|x| x.needs_review))
                        && status.as_ref().is_none_or(|x| {
                            state.is_some_and(|a| a.judgment.to_string() == *x)
                                || match &r.data {
                                    Data::Hypothesis { lifecycle, .. } => {
                                        lifecycle.to_string() == *x
                                    }
                                    Data::Experiment { status, .. } => status.to_string() == *x,
                                    _ => false,
                                }
                        })
                })
                .collect();
            list(&rows, cli.json)?;
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
            list(&rows, cli.json)?;
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
            let result = store.commit(changes, expected_revision.as_deref())?;
            print_json(&result)?;
            return Ok(());
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
        Command::Init { .. } | Command::Web { .. } => unreachable!(),
    }
    let affected: Vec<String> = changes
        .iter()
        .map(|c| match c {
            Change::Create { record } | Change::Update { record, .. } => record.id.clone(),
            Change::Archive { id, .. } | Change::Delete { id, .. } => id.clone(),
        })
        .collect();
    let result = store.commit(changes, Some(&s.revision))?;
    if cli.json {
        print_json(&serde_json::json!({"ids":affected,"snapshot":result}))?;
    } else {
        for id in affected {
            println!("{id}");
        }
    }
    Ok(())
}
fn list(rows: &[&Entry], json: bool) -> Result<()> {
    if json {
        return print_json(&rows);
    }
    for e in rows {
        println!(
            "{}  {:11} {}{}",
            e.record.id,
            e.record.data.kind(),
            e.record.title,
            if e.record.archived { " [archived]" } else { "" }
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
    store.commit(
        vec![
            create(h.clone()),
            create(f.clone()),
            create(p.clone()),
            create(g),
            create(alt.clone()),
            create(l),
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
    store.commit(vec![create(x.clone())], None)?;
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
    let run = Record::new(
        "Cache-disabled run #142",
        Data::Run {
            experiment: x.id.clone(),
            plan: store.snapshot()?.find(&x.id)?.frozen(),
            outcome: Outcome::Observed,
            evidence: vec![e.id.clone()],
        },
    );
    store.commit(
        vec![create(e.clone()), create(l), create(l2), create(run)],
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
    store.commit(vec![create(a)], None)?;
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
