//! `hyp edit`: a record's file in the user's editor. A rejected edit reopens
//! with the error at the top, like `git commit`; the user's text is never
//! silently dropped.
use crate::{
    model::*,
    store::{self, Change, Committed, Conflict, FrontMatter, Store},
};
use anyhow::{Context, Result, bail, ensure};
use std::io::IsTerminal;

/// The start of the lines hyp adds to the file it reopens. They sit in the
/// YAML front matter, as comments, and are removed before the text is read.
const NOTE: &str = "# hyp:";

/// Opens `e`'s file in $VISUAL, else $EDITOR, else vi, and commits the edited
/// record. On a terminal (stdin and stderr), a rejected record reopens the
/// editor with the error until the record is accepted or the user aborts:
/// saves the reopened file unchanged, empties it, or the editor fails.
/// Otherwise the first error fails the command, so a scripted editor cannot
/// loop. Failures name the last error and a file keeping the edited text.
pub fn edit(store: &Store, e: &Entry) -> Result<Committed> {
    let r = &e.record;
    let kind = r.data.kind();
    ensure!(
        !matches!(r.data, Data::Assessment { .. } | Data::Run { .. }),
        "{} is {} {kind}: assessments and runs are immutable, so hyp edit cannot \
         change it; record a new one instead",
        r.id,
        article(kind)
    );
    let interactive = std::io::stdin().is_terminal() && std::io::stderr().is_terminal();
    let pristine = store::encode(r)?;
    let file = tempfile::Builder::new()
        .prefix(&format!("hyp-edit-{}-", r.id.get(..10).unwrap_or(&r.id)))
        .suffix(".md")
        .tempfile()?;
    let path = file.path().to_path_buf();
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or("vi".into());
    let mut presented = pristine.clone();
    // The previous round's text without notes, and the error it got.
    let mut rejected: Option<(String, String)> = None;
    loop {
        std::fs::write(&path, &presented)
            .with_context(|| format!("cannot write {}", path.display()))?;
        let status = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("{editor} \"$1\""))
            .arg("hyp-edit")
            .arg(&path)
            .status()
            .with_context(|| format!("cannot run the editor {editor:?}"))?;
        // An editor may have removed the file; that reads as emptied.
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let edited = without_notes(&text);
        let aborted = if !status.success() {
            Some(format!("the editor exited unsuccessfully ({status})"))
        } else if edited.trim().is_empty() {
            Some("the file was emptied".into())
        } else if rejected
            .as_ref()
            .is_some_and(|(before, _)| *before == edited)
        {
            Some("the file was saved unchanged".into())
        } else {
            None
        };
        if let Some(why) = aborted {
            let kept = keep(file, &[&text, &presented], &pristine)?;
            let last = rejected.map_or(String::new(), |(_, err)| format!(". Last error: {err}"));
            bail!("edit aborted ({why}); nothing was written{last}{kept}");
        }
        match save(store, e, &edited) {
            Ok(committed) => return Ok(committed),
            Err(err) => {
                if let Some(Conflict(message)) = err.chain().find_map(|c| c.downcast_ref()) {
                    let kept = keep(file, &[&text], &pristine)?;
                    bail!(Conflict(format!("{message}{kept}")));
                }
                if !interactive {
                    let kept = keep(file, &[&text], &pristine)?;
                    bail!("{err:#}; nothing was written{kept}");
                }
                presented = with_notes(&edited, &err);
                rejected = Some((edited, format!("{err:#}")));
            }
        }
    }
}

/// The record in `text`, committed as an update of `e`.
fn save(store: &Store, e: &Entry, text: &str) -> Result<Committed> {
    let old = e.record.data.kind();
    if let Some(new) = kind_in(text).filter(|k| k != old) {
        bail!("cannot change the kind of a record ({old} to {new}); create a new record instead");
    }
    let r = store::decode(text)?;
    ensure!(r.id == e.record.id, "cannot change the ID of a record");
    store.commit_written(
        vec![Change::Update {
            record: r,
            expected_revision: e.revision.clone(),
        }],
        None,
    )
}

/// The `kind` field of the front matter, if it parses that far.
fn kind_in(text: &str) -> Option<String> {
    let (header, _) = text.strip_prefix("---\n")?.split_once("\n---\n")?;
    let yaml: serde_yaml::Value = serde_yaml::from_str(header).ok()?;
    yaml.get("kind")?.as_str().map(str::to_string)
}

/// `text` without the note lines `with_notes` put after its opening `---`
/// (or at its top, when it had none).
fn without_notes(text: &str) -> String {
    let (open, mut rest) = match text.strip_prefix("---\n") {
        Some(rest) => ("---\n", rest),
        None => ("", text),
    };
    while rest.starts_with(NOTE) {
        rest = rest.split_once('\n').map_or("", |(_, after)| after);
    }
    format!("{open}{rest}")
}

/// `text` with `err` and how to go on as note lines after its opening `---`.
/// A YAML error's line numbers are moved down by the notes, so that they
/// count lines in the file the user sees.
fn with_notes(text: &str, err: &anyhow::Error) -> String {
    let message = format!("{err:#}");
    let count = message.lines().count() + 1;
    let message = match err.downcast_ref::<FrontMatter>() {
        Some(_) if text.starts_with("---\n") => store::shift_lines(&message, count),
        _ => message,
    };
    let mut notes: Vec<String> = message
        .lines()
        .enumerate()
        .map(|(i, line)| match i {
            0 => format!("{NOTE} error: {line}"),
            _ => format!("{NOTE}   {line}"),
        })
        .collect();
    notes.push(format!(
        "{NOTE} fix it and save to retry; save it unchanged or empty it to abort"
    ));
    let notes = notes.join("\n");
    match text.strip_prefix("---\n") {
        Some(rest) => format!("---\n{notes}\n{rest}"),
        None => format!("{notes}\n{text}"),
    }
}

/// Keeps the temporary file with the first of `texts` that holds edits (not
/// empty, not the record as it was), and says where; or says nothing and
/// lets the file be deleted.
fn keep(file: tempfile::NamedTempFile, texts: &[&str], pristine: &str) -> Result<String> {
    let Some(text) = texts
        .iter()
        .find(|t| !t.trim().is_empty() && without_notes(t) != pristine)
    else {
        return Ok(String::new());
    };
    std::fs::write(file.path(), text)
        .with_context(|| format!("cannot write {}", file.path().display()))?;
    let (_, path) = file.keep().context("cannot keep the edited file")?;
    Ok(format!("; your text is kept in {}", path.display()))
}
