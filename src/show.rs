//! Plain `hyp show`: a summary for people reading a terminal. `hyp --json
//! show` is the complete, stable form for programs; this text may change.
use crate::model::*;

/// An RFC 3339 timestamp to the second; anything else as it is.
fn when(t: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(t).map_or_else(
        |_| t.to_string(),
        |d| d.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    )
}
/// `name: value`, or nothing for an empty value. Continuation lines of a
/// multi-line value are indented under it.
fn field(out: &mut Vec<String>, indent: &str, name: &str, value: &str) {
    let value = value.trim();
    if !value.is_empty() {
        out.push(format!(
            "{indent}{name}: {}",
            value.replace('\n', &format!("\n{indent}  "))
        ));
    }
}
/// A multi-line text (a body or rationale), indented, or nothing when empty.
fn text(out: &mut Vec<String>, indent: &str, text: &str) {
    for line in text.trim().lines() {
        out.push(format!("{indent}{line}").trim_end().to_string());
    }
}
fn header(out: &mut Vec<String>, r: &Record) {
    out.push(format!("{}  {}", r.id, r.data.kind()));
    field(out, "", "title", &r.title);
}
fn footer(out: &mut Vec<String>, r: &Record) {
    field(out, "", "tags", &r.tags.join(", "));
    let (created, updated) = (when(&r.created_at), when(&r.updated_at));
    let mut times = format!("created {created}");
    if updated != created {
        times.push_str(&format!(" · updated {updated}"));
    }
    out.push(times);
    if r.archived {
        out.push("archived".into());
    }
    if !r.body.trim().is_empty() {
        out.push(String::new());
        text(out, "", &r.body);
    }
}
/// A data field for the compact list: strings, numbers and booleans as they
/// are, a list or a frozen copy by its IDs (or paths); None when empty.
fn value(v: &serde_json::Value) -> Option<String> {
    use serde_json::Value;
    let s = match v {
        Value::Null => return None,
        Value::String(s) => s.clone(),
        Value::Array(items) => items
            .iter()
            .filter_map(value)
            .collect::<Vec<_>>()
            .join(", "),
        Value::Object(map) => map
            .get("id")
            .or_else(|| map.get("path"))
            .and_then(value)
            .unwrap_or_else(|| v.to_string()),
        other => other.to_string(),
    };
    (!s.is_empty()).then_some(s)
}
fn active<'a>(
    s: &'a Snapshot,
    pred: impl Fn(&Data) -> bool + 'a,
) -> impl Iterator<Item = &'a Entry> {
    s.objects
        .iter()
        .filter(move |e| !e.record.archived && pred(&e.record.data))
}
fn archived(e: &Entry) -> &'static str {
    if e.record.archived { " [archived]" } else { "" }
}
fn title_of<'a>(s: &'a Snapshot, id: &str) -> &'a str {
    s.get(id).map_or("", |e| e.record.title.as_str())
}
fn section(out: &mut Vec<String>, name: &str, lines: Vec<String>) {
    if !lines.is_empty() {
        out.push(String::new());
        out.push(name.to_string());
        out.extend(lines);
    }
}

/// The plain `hyp show` text of `e`. A hypothesis gets its derived state,
/// the review token for `hyp assess --reviewed`, and its criteria and
/// predictions (archived ones marked), linked evidence with its observation,
/// experiments with their runs, gaps, links to other hypotheses and current
/// assessments. Other records get their fields and the records that refer to
/// them. Empty fields and other archived records are left out.
pub fn plain(s: &Snapshot, e: &Entry) -> String {
    let r = &e.record;
    let mut out = Vec::new();
    header(&mut out, r);
    match (&r.data, s.hypotheses.get(&r.id)) {
        (
            Data::Hypothesis {
                scope,
                assumptions,
                lifecycle,
                untestable_reason,
            },
            Some(state),
        ) => {
            field(&mut out, "", "scope", scope);
            field(&mut out, "", "assumptions", assumptions);
            field(&mut out, "", "lifecycle", lifecycle.as_str());
            field(&mut out, "", "untestable reason", untestable_reason);
            let mut judgment = state.judgment.to_string();
            if let Some(c) = state.confidence {
                judgment.push_str(&format!(" · confidence {c}"));
            }
            if state.needs_review {
                judgment.push_str(" · needs review");
            }
            field(&mut out, "", "judgment", &judgment);
            field(&mut out, "", "review token", &state.review_token);
            footer(&mut out, r);
            hypothesis(&mut out, s, &r.id, &state.assessment_ids);
        }
        (data, _) => {
            if let serde_json::Value::Object(mut fields) =
                serde_json::to_value(data).unwrap_or_default()
            {
                // As `hyp link --relation` spells it, not as stored.
                if let Data::Link { relation, .. } = data {
                    fields.insert("relation".into(), relation.as_str().into());
                }
                for (name, v) in &fields {
                    if name == "kind" {
                        continue;
                    }
                    let v = value(v).map(|v| if name.ends_with("_at") { when(&v) } else { v });
                    field(&mut out, "", name, v.as_deref().unwrap_or_default());
                }
            }
            footer(&mut out, r);
            let refs = active(s, |d| d.references().contains(&r.id.as_str()))
                .map(|x| {
                    format!(
                        "  {}  {:11} {}",
                        x.record.id,
                        x.record.data.kind(),
                        x.record.title
                    )
                })
                .collect();
            section(&mut out, "Referenced by", refs);
        }
    }
    out.join("\n")
}
fn hypothesis(out: &mut Vec<String>, s: &Snapshot, h: &str, current: &[String]) {
    let owned = |d: &Data| d.owner() == Some(h);
    let rows = |pred: fn(&Data) -> bool| -> Vec<&Entry> {
        active(s, move |d| owned(d) && pred(d)).collect()
    };
    // Archived criteria and predictions stay in the review basis, so they
    // are listed (marked): archiving one can be why a review is needed.
    let claim_rows = |pred: fn(&Data) -> bool| -> Vec<&Entry> {
        s.objects
            .iter()
            .filter(|x| owned(&x.record.data) && pred(&x.record.data))
            .collect()
    };
    let criteria: Vec<String> = claim_rows(|d| matches!(d, Data::Criterion { .. }))
        .iter()
        .map(|x| format!("  {}  {}{}", x.record.id, x.record.title, archived(x)))
        .collect();
    section(out, "Falsification criteria", criteria);
    let mut predictions = Vec::new();
    for x in claim_rows(|d| matches!(d, Data::Prediction { .. })) {
        predictions.push(format!(
            "  {}  {}{}",
            x.record.id,
            x.record.title,
            archived(x)
        ));
        if let Data::Prediction { conditions, .. } = &x.record.data {
            field(&mut predictions, "    ", "conditions", conditions);
        }
    }
    section(out, "Predictions", predictions);

    // What an assessment may cite (`Snapshot::evidence_links`), by relation.
    let links = s.evidence_links(h);
    let mut evidence = Vec::new();
    for relation in [
        Relation::Supports,
        Relation::Contradicts,
        Relation::Qualifies,
    ] {
        let mut group = Vec::new();
        for l in &links {
            let Data::Link {
                from,
                to,
                relation: rel,
            } = &l.record.data
            else {
                continue;
            };
            let Some(ev) = s.get(from).filter(|_| *rel == relation) else {
                continue;
            };
            let Data::Evidence {
                source,
                locator,
                observed_at,
                attachments,
            } = &ev.record.data
            else {
                continue;
            };
            group.push(format!(
                "    {}  {}{}",
                ev.record.id,
                ev.record.title,
                archived(ev)
            ));
            text(&mut group, "      ", &ev.record.body);
            let mut provenance = vec![format!("source: {source}")];
            if !locator.trim().is_empty() {
                provenance.push(format!("locator: {locator}"));
            }
            if !observed_at.is_empty() {
                provenance.push(format!("observed {}", when(observed_at)));
            }
            if !attachments.is_empty() {
                provenance.push(format!("{} attachment(s)", attachments.len()));
            }
            group.push(format!("      {}", provenance.join(" · ")));
            let mut via = format!("link {}", l.record.id);
            if to != h {
                via.push_str(&format!(" to {to} ({})", title_of(s, to)));
            }
            // `hyp evidence add` without --reason repeats the title.
            let reason = if l.record.body == ev.record.title {
                ""
            } else {
                &l.record.body
            };
            group.push(format!("      {via}"));
            field(&mut group, "      ", "reason", reason);
        }
        if !group.is_empty() {
            evidence.push(format!("  {relation}"));
            evidence.extend(group);
        }
    }
    section(out, "Evidence", evidence);

    let mut experiments = Vec::new();
    for x in rows(|d| matches!(d, Data::Experiment { .. })) {
        if let Data::Experiment { status, .. } = &x.record.data {
            experiments.push(format!("  {}  {} [{status}]", x.record.id, x.record.title));
        }
        for run in active(
            s,
            |d| matches!(d, Data::Run { experiment, .. } if *experiment == x.record.id),
        ) {
            if let Data::Run {
                outcome, evidence, ..
            } = &run.record.data
            {
                let mut line = format!("    {}  {}: {outcome}", run.record.id, run.record.title);
                if !evidence.is_empty() {
                    line.push_str(&format!(" · evidence {}", evidence.join(", ")));
                }
                experiments.push(line);
            }
        }
    }
    section(out, "Experiments and runs", experiments);

    let gaps = rows(|d| matches!(d, Data::Gap { .. }))
        .iter()
        .filter_map(|x| match &x.record.data {
            Data::Gap { resolved, .. } => Some(format!(
                "  {}  {} [{}]",
                x.record.id,
                x.record.title,
                if *resolved { "resolved" } else { "open" }
            )),
            _ => None,
        })
        .collect();
    section(out, "Gaps", gaps);

    let mut links = Vec::new();
    for l in active(s, |d| matches!(d, Data::Link { .. })) {
        let Data::Link { from, to, relation } = &l.record.data else {
            continue;
        };
        let line = if from == h && s.hypotheses.contains_key(to) {
            format!("  {relation} {to}  {}", title_of(s, to))
        } else if to == h && s.hypotheses.contains_key(from) {
            format!("  {from} {relation} this  {}", title_of(s, from))
        } else {
            continue;
        };
        links.push(line);
        field(&mut links, "    ", "reason", &l.record.body);
    }
    section(out, "Other hypotheses", links);

    let mut assessments = Vec::new();
    for id in current {
        let Some(a) = s.get(id) else { continue };
        let Data::Assessment {
            judgment,
            confidence,
            evidence,
            criterion,
            ..
        } = &a.record.data
        else {
            continue;
        };
        let mut line = format!("  {}  {judgment}", a.record.id);
        if let Some(c) = confidence {
            line.push_str(&format!(" · confidence {c}"));
        }
        if let Some(c) = criterion {
            line.push_str(&format!(" · criterion {c}"));
        }
        if !evidence.is_empty() {
            line.push_str(&format!(" · evidence {}", evidence.join(", ")));
        }
        line.push_str(&format!(" · {}", when(&a.record.created_at)));
        assessments.push(line);
        text(&mut assessments, "    ", &a.record.body);
    }
    let heading = if current.len() > 1 {
        "Current assessments (conflicting: assess again to settle)"
    } else {
        "Current assessment"
    };
    section(out, heading, assessments);
}
