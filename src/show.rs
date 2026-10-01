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
/// The first line: ID and kind, for a hypothesis also the first 12 hex
/// digits of its review token, what `hyp assess --reviewed` takes; then the
/// title.
fn header(out: &mut Vec<String>, r: &Record, state: Option<&HypothesisState>) {
    let mut first = format!("{}  {}", r.id, r.data.kind());
    if let Some(state) = state {
        first.push_str(&format!("  review {}", &state.review_token[..12]));
    }
    out.push(first);
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
/// the review token for `hyp assess --reviewed` (`header`), and its criteria
/// and predictions (archived ones marked), linked evidence with its
/// observation and what each link means for the hypothesis (`evidence`),
/// experiments with their runs, gaps, links to other hypotheses and current
/// assessments. Other records get their fields and the records that refer to
/// them; evidence first the hypotheses it bears on and how (`bears_on`).
/// Empty fields and other archived records are left out.
pub fn plain(s: &Snapshot, e: &Entry) -> String {
    let r = &e.record;
    let mut out = Vec::new();
    header(&mut out, r, s.hypotheses.get(&r.id));
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
            footer(&mut out, r);
            data_section(&mut out, s, r);
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
                    // The fingerprint of the basis the assessment was based on,
                    // not the review token that `hyp assess --reviewed` took.
                    let name = if name == "based_on" {
                        "based on fingerprint"
                    } else {
                        name
                    };
                    field(&mut out, "", name, v.as_deref().unwrap_or_default());
                }
            }
            footer(&mut out, r);
            data_section(&mut out, s, r);
            // Evidence: its links to claims are listed under "Bears on".
            let shown = match data {
                Data::Evidence { .. } => bears_on(&mut out, s, e),
                _ => vec![],
            };
            // Every referrer of a data record, archived ones too: each keeps
            // it from being deleted.
            let every = matches!(data, Data::Captured { .. });
            let refs = s
                .objects
                .iter()
                .filter(|x| every || !x.record.archived)
                .filter(|x| x.record.references().contains(&r.id.as_str()))
                .filter(|x| !shown.contains(&x.record.id.as_str()))
                .map(|x| {
                    format!(
                        "  {}  {:11} {}{}",
                        x.record.id,
                        x.record.data.kind(),
                        x.record.title,
                        archived(x)
                    )
                })
                .collect();
            section(&mut out, "Referenced by", refs);
        }
    }
    out.join("\n")
}
/// The data records `r` references (`Record::data_refs`), one line each:
/// ID, media type, size and title; a missing one as such.
fn data_section(out: &mut Vec<String>, s: &Snapshot, r: &Record) {
    let lines = r
        .data_refs
        .iter()
        .map(|id| match s.get(id) {
            Some(d) => match &d.record.data {
                Data::Captured {
                    media_type, size, ..
                } => format!(
                    "  {id}  {media_type} · {size} bytes  {}{}",
                    d.record.title,
                    archived(d)
                ),
                _ => format!("  {id}  (not a data record)"),
            },
            None => format!("  {id}  (missing)"),
        })
        .collect();
    section(out, "Data", lines);
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

    section(out, "Evidence", evidence(s, h));

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
            Data::Gap {
                resolved,
                resolved_by,
                ..
            } => Some(format!(
                "  {}  {} [{}]",
                x.record.id,
                x.record.title,
                match (resolved, resolved_by.as_slice()) {
                    (false, _) => "open".to_string(),
                    (true, []) => "resolved".to_string(),
                    (true, by) => format!("resolved by {}", by.join(", ")),
                }
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
/// What an assessment may cite (`Snapshot::evidence_bearings`): each
/// observation once, grouped by what it means for hypothesis `h` ("against
/// H", "for H", "qualifies H", "mixed: its links disagree"), with its
/// provenance and every link that brings it in, each with its meaning.
fn evidence(s: &Snapshot, h: &str) -> Vec<String> {
    let bearings = s.evidence_bearings(h);
    let mut out = Vec::new();
    for (stance, heading) in [
        (Stance::Against, "against H"),
        (Stance::For, "for H"),
        (Stance::Qualifies, "qualifies H"),
        (Stance::Mixed, "mixed: its links disagree"),
    ] {
        let mut group = Vec::new();
        for b in bearings.iter().filter(|b| b.stance == stance) {
            let Some(ev) = s.get(&b.evidence) else {
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
            if !ev.record.data_refs.is_empty() {
                provenance.push(format!("data {}", ev.record.data_refs.join(", ")));
            }
            group.push(format!("      {}", provenance.join(" · ")));
            bearing_lines(&mut group, "      ", s, h, ev, b);
        }
        if !group.is_empty() {
            out.push(format!("  {heading}"));
            out.extend(group);
        }
    }
    out
}
/// Each link of `b` (observation `ev` read for hypothesis `h`): its meaning
/// and ID, the criterion or prediction it goes through, and its reason.
fn bearing_lines(
    out: &mut Vec<String>,
    indent: &str,
    s: &Snapshot,
    h: &str,
    ev: &Entry,
    b: &EvidenceBearing,
) {
    let deeper = format!("{indent}  ");
    for link in &b.bearings {
        out.push(format!("{indent}{} · link {}", link.meaning, link.link));
        if link.via != h {
            if let Some(via) = s.get(&link.via) {
                field(out, &deeper, via.record.data.kind(), &via.record.title);
            }
        }
        // `hyp evidence add` without --reason repeats the title.
        let reason = s
            .get(&link.link)
            .map(|l| l.record.body.as_str())
            .filter(|body| *body != ev.record.title)
            .unwrap_or_default();
        field(out, &deeper, "reason", reason);
    }
}
/// The "Bears on" section of evidence `ev`: each hypothesis it bears on
/// (`Snapshot::bears_on`) with its stance, the hypothesis's judgment and
/// lifecycle, and its links; and whether no live hypothesis accounts for it
/// (`Snapshot::unexplained`). Returns the IDs of the
/// links it listed.
fn bears_on<'a>(out: &mut Vec<String>, s: &'a Snapshot, ev: &Entry) -> Vec<&'a str> {
    let id = ev.record.id.as_str();
    let mut lines = Vec::new();
    let mut links = Vec::new();
    for (h, b) in s.bears_on(id) {
        let mut how = b.stance.as_str().to_string();
        if let Some(state) = s.hypotheses.get(&h.record.id) {
            how.push_str(&format!(" · {}", state.judgment));
        }
        if let Data::Hypothesis { lifecycle, .. } = &h.record.data {
            how.push_str(&format!(" · {lifecycle}"));
        }
        lines.push(format!(
            "  {}  {how}  {}{}",
            h.record.id,
            h.record.title,
            archived(h)
        ));
        bearing_lines(&mut lines, "    ", s, &h.record.id, ev, b);
        links.extend(b.bearings.iter().map(|l| l.link.as_str()));
    }
    if s.unexplained().iter().any(|e| e.record.id == id) {
        lines.push(format!(
            "  unexplained: no live hypothesis accounts for it (hyp add \"…\" --explains {})",
            id.get(..10).unwrap_or(id)
        ));
    }
    section(out, "Bears on", lines);
    links
}
