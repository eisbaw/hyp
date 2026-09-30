//! The error kinds of the machine contract (decision-0002, HYPO-0078).
//! `hyp --json` prints a failed command's error as `{"error", "kind"}`, plus
//! `ids` for a conflict whose failing records are known, and the WebUI API
//! answers with the same body. The kind comes from the error's type anywhere
//! in its chain, decided where the error arises, never from message text.
use serde::Serialize;

/// What an agent does next: re-read and retry (`Conflict`), fix the input
/// (`InvalidInput`, `NotFound`, `AmbiguousId`), repair files first
/// (`Blocked`), or look at the file system (`Io`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// A precondition failed: something changed since the caller read it.
    /// The only kind that exits 3 (HTTP 409).
    Conflict,
    /// Any other input the rules reject; the default kind.
    InvalidInput,
    /// A named record, or the project, does not exist.
    NotFound,
    /// An ID prefix matches more than one record.
    AmbiguousId,
    /// A diagnostic blocks every write (`Code::blocks_writes`).
    Blocked,
    /// Reading or writing a file failed.
    Io,
}

/// An error whose kind its source decided. `Conflict` has its own type,
/// which also carries the IDs.
#[derive(Debug)]
pub struct Classified {
    pub kind: ErrorKind,
    pub message: String,
}
impl Classified {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}
impl std::fmt::Display for Classified {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for Classified {}

/// A write precondition failed: the project or an object changed since the
/// caller read it. The CLI exits with code 3 and the API answers 409 when this
/// type is anywhere in the error chain, so wrapping it in `.context()` does not
/// change its classification. The message starts with "conflict:", which the
/// WebUI and agents read. `ids` are the records whose statements failed, when
/// known (empty for a whole-project precondition).
#[derive(Debug)]
pub struct Conflict {
    pub message: String,
    pub ids: Vec<String>,
}
impl Conflict {
    /// A conflict not tied to particular records.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            ids: vec![],
        }
    }
    /// A conflict over the records `ids`.
    pub fn on(ids: Vec<String>, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            ids,
        }
    }
    /// The first `Conflict` in `err`'s chain.
    pub fn find(err: &anyhow::Error) -> Option<&Self> {
        err.chain().find_map(|cause| cause.downcast_ref())
    }
    pub fn in_chain(err: &anyhow::Error) -> bool {
        Self::find(err).is_some()
    }
}
impl std::fmt::Display for Conflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "conflict: {}", self.message)
    }
}
impl std::error::Error for Conflict {}

/// The kind of `err`: a `Conflict` anywhere in the chain wins, then the
/// outermost `Classified`, then an I/O error; anything else is input to fix.
pub fn kind_of(err: &anyhow::Error) -> ErrorKind {
    if Conflict::in_chain(err) {
        return ErrorKind::Conflict;
    }
    if let Some(c) = err
        .chain()
        .find_map(|cause| cause.downcast_ref::<Classified>())
    {
        return c.kind;
    }
    if err.chain().any(|cause| cause.is::<std::io::Error>()) {
        return ErrorKind::Io;
    }
    ErrorKind::InvalidInput
}

/// `{"error": message, "kind": kind}`, with `"ids"` for a conflict over
/// known records: what `hyp --json` prints on stderr and the API answers.
pub fn to_json(err: &anyhow::Error) -> serde_json::Value {
    let mut body = serde_json::json!({"error": format!("{err:#}"), "kind": kind_of(err)});
    if let Some(conflict) = Conflict::find(err).filter(|c| !c.ids.is_empty()) {
        body["ids"] = serde_json::json!(conflict.ids);
    }
    body
}
