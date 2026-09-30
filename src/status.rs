//! `hyp status`: where the investigation stands, in one read. For an agent
//! resuming work: what needs review, what is open, what is missing, and
//! whether the project accepts writes. It prints no review token: a token
//! comes from the `hyp show` output that was reviewed (HYPO-0074).
use crate::model::*;
use serde::Serialize;

/// A listed hypothesis: an open one (not archived, not closed), or a
/// closed one that needs review. What it has is listed by full ID, never
/// counted: an agent acts on IDs, and a count is the list's length.
#[derive(Debug, Serialize)]
pub struct Row {
    pub id: String,
    pub title: String,
    pub lifecycle: Lifecycle,
    pub judgment: Judgment,
    pub confidence: Option<f64>,
    pub needs_review: bool,
    /// Active falsification criteria.
    pub criteria: Vec<String>,
    pub untestable_reason: String,
    /// No active criterion and no untestable reason.
    pub missing_criterion: bool,
    /// `Snapshot::linked_evidence` that loaded: what an assessment may
    /// cite. Evidence whose file is missing or malformed is left out; `hyp
    /// check` reports it.
    pub linked_evidence: Vec<String>,
    /// Unresolved, active gaps (without `resolved_by`).
    pub open_gaps: Vec<GapRow>,
    /// Resolved, active gaps, with the evidence that resolved each.
    pub resolved_gaps: Vec<GapRow>,
    /// Active experiments, not cancelled, without an active run.
    pub experiments_without_runs: Vec<String>,
}
/// A gap of a `Row`: `{"id"}` while open, `{"id", "resolved_by"}` once
/// resolved (`resolved_by` empty when it was resolved without evidence).
#[derive(Debug, Serialize)]
pub struct GapRow {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_by: Option<Vec<String>>,
}
/// Hypotheses `Report` does not list: closed ones that need no review, and
/// archived ones; `needs_review` counts those of them that need review
/// (only archived ones can).
#[derive(Debug, Serialize)]
pub struct NotShown {
    pub closed: usize,
    pub archived: usize,
    pub needs_review: usize,
}
/// An observation no live hypothesis accounts for (`Snapshot::unexplained`).
#[derive(Debug, Serialize)]
pub struct Observation {
    pub id: String,
    pub title: String,
}
#[derive(Debug, Serialize)]
pub struct Report {
    /// Open hypotheses and closed ones needing review; those needing review
    /// first, otherwise in project order.
    pub hypotheses: Vec<Row>,
    pub not_shown: NotShown,
    /// Every unexplained observation, in project order; the plain text
    /// lists the first `SHOWN_OBSERVATIONS`.
    pub unexplained_observations: Vec<Observation>,
    pub writes_blocked: bool,
    /// The diagnostics that block every write (`Code::blocks_writes`).
    pub blocking: Vec<Diagnostic>,
    /// All errors (blocking ones included) and warnings, as `hyp check`
    /// reports them.
    pub errors: usize,
    pub warnings: usize,
    pub revision: String,
}

pub fn report(s: &Snapshot) -> Report {
    let active = |d: fn(&Data) -> bool| {
        s.objects
            .iter()
            .filter(move |e| !e.record.archived && d(&e.record.data))
    };
    let mut hypotheses = Vec::new();
    let (mut closed, mut archived, mut hidden_review) = (0, 0, 0);
    for e in &s.objects {
        let r = &e.record;
        let Data::Hypothesis {
            lifecycle,
            untestable_reason,
            ..
        } = &r.data
        else {
            continue;
        };
        let Some(state) = s.hypotheses.get(&r.id) else {
            continue;
        };
        if r.archived {
            archived += 1;
            hidden_review += usize::from(state.needs_review);
            continue;
        }
        if *lifecycle == Lifecycle::Closed && !state.needs_review {
            closed += 1;
            continue;
        }
        let owned = |x: &&Entry| x.record.data.owner() == Some(r.id.as_str());
        let criteria: Vec<String> = active(|d| matches!(d, Data::Criterion { .. }))
            .filter(owned)
            .map(|x| x.record.id.clone())
            .collect();
        let (mut open_gaps, mut resolved_gaps) = (Vec::new(), Vec::new());
        for gap in active(|d| matches!(d, Data::Gap { .. })).filter(owned) {
            if let Data::Gap {
                resolved,
                resolved_by,
                ..
            } = &gap.record.data
            {
                let id = gap.record.id.clone();
                if *resolved {
                    resolved_gaps.push(GapRow {
                        id,
                        resolved_by: Some(resolved_by.clone()),
                    });
                } else {
                    open_gaps.push(GapRow {
                        id,
                        resolved_by: None,
                    });
                }
            }
        }
        let experiments_without_runs = active(|d| {
            matches!(d, Data::Experiment { status, .. } if *status != ExperimentStatus::Cancelled)
        })
        .filter(owned)
        .filter(|x| {
            !active(|d| matches!(d, Data::Run { .. })).any(
                |run| matches!(&run.record.data, Data::Run { experiment, .. } if *experiment == x.record.id),
            )
        })
        .map(|x| x.record.id.clone())
        .collect();
        hypotheses.push(Row {
            id: r.id.clone(),
            title: r.title.clone(),
            lifecycle: *lifecycle,
            judgment: state.judgment,
            confidence: state.confidence,
            needs_review: state.needs_review,
            untestable_reason: untestable_reason.trim().to_string(),
            missing_criterion: criteria.is_empty() && untestable_reason.trim().is_empty(),
            criteria,
            linked_evidence: s
                .linked_evidence(&r.id)
                .into_iter()
                .filter(|id| {
                    s.get(id)
                        .is_some_and(|e| matches!(e.record.data, Data::Evidence { .. }))
                })
                .map(str::to_string)
                .collect(),
            open_gaps,
            resolved_gaps,
            experiments_without_runs,
        });
    }
    // Stable: otherwise in project order.
    hypotheses.sort_by_key(|row| !row.needs_review);
    let blocking: Vec<Diagnostic> = s
        .diagnostics
        .iter()
        .filter(|d| d.blocks_writes)
        .cloned()
        .collect();
    let count = |severity: &str| {
        s.diagnostics
            .iter()
            .filter(|d| d.severity == severity)
            .count()
    };
    Report {
        hypotheses,
        not_shown: NotShown {
            closed,
            archived,
            needs_review: hidden_review,
        },
        unexplained_observations: s
            .unexplained()
            .into_iter()
            .map(|e| Observation {
                id: e.record.id.clone(),
                title: e.record.title.clone(),
            })
            .collect(),
        writes_blocked: !blocking.is_empty(),
        blocking,
        errors: count("error"),
        warnings: count("warning"),
        revision: s.revision.clone(),
    }
}

/// An ID shortened to its kind letter and 8 hex digits, a prefix hyp accepts.
fn short(id: &str) -> &str {
    id.get(..10).unwrap_or(id)
}
/// "open gaps 0", or "open gaps 2: G-…, G-…".
fn counted(name: &str, ids: &[String]) -> String {
    if ids.is_empty() {
        return format!("{name} 0");
    }
    let ids: Vec<&str> = ids.iter().map(|id| short(id)).collect();
    format!("{name} {}: {}", ids.len(), ids.join(", "))
}
/// "resolved gaps 2: G-… by E-…, E-…; G-…", only listed when there are some.
fn resolved(gaps: &[GapRow]) -> String {
    let items: Vec<String> = gaps
        .iter()
        .map(|g| match g.resolved_by.as_deref().unwrap_or_default() {
            [] => short(&g.id).to_string(),
            by => {
                let by: Vec<&str> = by.iter().map(|id| short(id)).collect();
                format!("{} by {}", short(&g.id), by.join(", "))
            }
        })
        .collect();
    format!("resolved gaps {}: {}", gaps.len(), items.join("; "))
}
fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}
/// How many unexplained observations the plain text lists; `--json` has all.
const SHOWN_OBSERVATIONS: usize = 5;

/// The plain text: a header line for the project, one line per diagnostic
/// that blocks writes, two lines per hypothesis, then the first
/// `SHOWN_OBSERVATIONS` unexplained observations.
pub fn plain(report: &Report) -> String {
    let open = report
        .hypotheses
        .iter()
        .filter(|r| r.lifecycle != Lifecycle::Closed)
        .count();
    let mut header = vec![plural(open, "open hypothesis", "open hypotheses")];
    let review = report.hypotheses.iter().filter(|r| r.needs_review).count();
    if review > 0 {
        header.push(format!("{review} needs review"));
    }
    let NotShown {
        closed,
        archived,
        needs_review,
    } = report.not_shown;
    match (closed, archived) {
        (0, 0) => {}
        (c, 0) => header.push(format!("{c} closed not shown")),
        (0, a) => header.push(format!("{a} archived not shown")),
        (c, a) => header.push(format!("{c} closed and {a} archived not shown")),
    }
    if needs_review > 0 {
        header.push(format!("{needs_review} archived needs review"));
    }
    let unexplained = &report.unexplained_observations;
    if !unexplained.is_empty() {
        header.push(plural(
            unexplained.len(),
            "unexplained observation",
            "unexplained observations",
        ));
    }
    // Not finished while an archived hypothesis still needs review or an
    // observation is unexplained.
    if report.hypotheses.is_empty() && closed > 0 && needs_review == 0 && unexplained.is_empty() {
        header.push("investigation finished: hyp list shows the conclusions".into());
    }
    header.push(if report.writes_blocked {
        format!(
            "writes blocked by {} (fix these files first)",
            plural(report.blocking.len(), "error", "errors")
        )
    } else {
        "writes not blocked".into()
    });
    let others = report.errors - report.blocking.len();
    let mut checks = Vec::new();
    if others > 0 {
        checks.push(plural(others, "other error", "other errors"));
    }
    if report.warnings > 0 {
        checks.push(plural(report.warnings, "warning", "warnings"));
    }
    if !checks.is_empty() {
        header.push(format!("{} (hyp check)", checks.join(", ")));
    }
    let mut out = vec![header.join(" · ")];
    for d in &report.blocking {
        out.push(format!("blocks writes: {}: {}", d.path, d.message));
    }
    for r in &report.hypotheses {
        let mut judgment = r.judgment.to_string();
        if let Some(c) = r.confidence {
            judgment.push_str(&format!(" (confidence {c})"));
        }
        if r.needs_review {
            judgment.push_str(" · needs review");
        }
        out.push(format!(
            "{}  {judgment} · {}  {}",
            short(&r.id),
            r.lifecycle,
            r.title
        ));
        let criterion = if !r.criteria.is_empty() {
            format!("criteria {}", r.criteria.len())
        } else if r.missing_criterion {
            "no criterion".into()
        } else {
            format!("untestable: {}", r.untestable_reason)
        };
        let mut line = format!(
            "  {criterion} · linked evidence {} · {}",
            r.linked_evidence.len(),
            counted(
                "open gaps",
                &r.open_gaps.iter().map(|g| g.id.clone()).collect::<Vec<_>>()
            ),
        );
        if !r.resolved_gaps.is_empty() {
            line.push_str(&format!(" · {}", resolved(&r.resolved_gaps)));
        }
        line.push_str(&format!(
            " · {}",
            counted("experiments without runs", &r.experiments_without_runs)
        ));
        out.push(line);
    }
    if !unexplained.is_empty() {
        out.push(
            "unexplained observations (no live hypothesis accounts for them; hyp add \"…\" \
             --explains E-…, or hyp link E-… H-…):"
                .into(),
        );
        for o in unexplained.iter().take(SHOWN_OBSERVATIONS) {
            out.push(format!("  {}  {}", short(&o.id), o.title));
        }
        if unexplained.len() > SHOWN_OBSERVATIONS {
            out.push(format!(
                "  ({} more; hyp --json status lists all)",
                unexplained.len() - SHOWN_OBSERVATIONS
            ));
        }
    }
    out.join("\n")
}
