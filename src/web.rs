use crate::{
    model::Snapshot,
    store::{Change, Conflict, Store},
};
use anyhow::Result;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{
        Html, IntoResponse, Response, Sse,
        sse::{Event, KeepAlive},
    },
    routing::{get, post},
};
use notify::Watcher;
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, sync::Arc, time::Duration};
use tokio::sync::watch;
use tokio_stream::{StreamExt, wrappers::WatchStream};

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub token: String,
    pub port: u16,
    pub events: watch::Sender<String>,
}
/// An error answered as `hyp --json` prints it (`error::to_json`: `{"error",
/// "kind"}`, and `ids` for a conflict). The status comes from the error's
/// type: 409 for a `Conflict` anywhere in the chain, otherwise 422.
#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    error: anyhow::Error,
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(crate::error::to_json(&self.error))).into_response()
    }
}
impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        let status = if Conflict::in_chain(&error) {
            StatusCode::CONFLICT
        } else {
            StatusCode::UNPROCESSABLE_ENTITY
        };
        Self { status, error }
    }
}
async fn guard(
    State(state): State<Arc<AppState>>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default();
    let allowed = [
        format!("127.0.0.1:{}", state.port),
        format!("localhost:{}", state.port),
    ];
    if !allowed.iter().any(|h| h == host) {
        return (StatusCode::FORBIDDEN, "Untrusted Host").into_response();
    }
    if let Some(origin) = req.headers().get(header::ORIGIN) {
        let origin = origin.to_str().unwrap_or_default();
        if !allowed.iter().any(|h| format!("http://{h}") == origin) {
            return (StatusCode::FORBIDDEN, "Untrusted Origin").into_response();
        }
    }
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_SECURITY_POLICY,"default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'".parse().unwrap());
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    headers.insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
}
async fn index() -> Html<&'static str> {
    Html(include_str!("../web/index.html"))
}
async fn css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css")],
        include_str!("../web/style.css"),
    )
}
async fn js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript")],
        include_str!("../web/app.js"),
    )
}
async fn session(State(s): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({"token":s.token}))
}
/// The snapshot as the WebUI reads it: every field of `Snapshot`, plus what
/// the UI shows that only the server derives. Built for each answer, never
/// stored; `hyp export --format json` prints the plain `Snapshot`.
#[derive(Serialize)]
struct WebSnapshot<'a> {
    #[serde(flatten)]
    snapshot: &'a Snapshot,
    /// The IDs of `Snapshot::unexplained`, the observations `hyp status`
    /// lists as unexplained, in project order. `Snapshot` must not get a
    /// field of this name: flattened, the JSON would hold the key twice.
    unexplained: Vec<&'a str>,
}
impl<'a> WebSnapshot<'a> {
    fn of(snapshot: &'a Snapshot) -> Self {
        let unexplained = snapshot
            .unexplained()
            .into_iter()
            .map(|e| e.record.id.as_str())
            .collect();
        Self {
            snapshot,
            unexplained,
        }
    }
}
async fn snapshot(State(s): State<Arc<AppState>>) -> std::result::Result<Response, ApiError> {
    let store = s.store.clone();
    let snap = tokio::task::spawn_blocking(move || store.snapshot())
        .await
        .map_err(anyhow::Error::from)??;
    Ok(Json(WebSnapshot::of(&snap)).into_response())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Transaction {
    /// Optional whole-project precondition. Each change already carries the
    /// preconditions it depends on, so ordinary edits omit this; sending it
    /// makes any concurrent write, related or not, a conflict.
    #[serde(default)]
    expected_revision: Option<String>,
    changes: Vec<Change>,
}
async fn mutate(
    State(s): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(tx): Json<Transaction>,
) -> std::result::Result<Response, ApiError> {
    if headers.get("x-hyp-token").and_then(|v| v.to_str().ok()) != Some(s.token.as_str()) {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            error: anyhow::anyhow!("invalid session token"),
        });
    }
    if tx.changes.is_empty() || tx.changes.len() > 100 {
        return Err(anyhow::anyhow!("transaction must contain 1–100 changes").into());
    }
    let store = s.store.clone();
    let snap = tokio::task::spawn_blocking(move || {
        store.commit(tx.changes, tx.expected_revision.as_deref())
    })
    .await
    .map_err(anyhow::Error::from)??;
    s.events.send_replace(snap.revision.clone());
    Ok(Json(WebSnapshot::of(&snap)).into_response())
}
async fn events(
    State(s): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = std::result::Result<Event, Infallible>>> {
    let stream = WatchStream::new(s.events.subscribe())
        .map(|revision| Ok(Event::default().event("change").data(revision)));
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}
async fn report(State(s): State<Arc<AppState>>) -> std::result::Result<Html<String>, ApiError> {
    let store = s.store.clone();
    let html = tokio::task::spawn_blocking(move || export_html(&store.snapshot()?))
        .await
        .map_err(anyhow::Error::from)??;
    Ok(Html(html))
}
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/style.css", get(css))
        .route("/app.js", get(js))
        .route("/api/session", get(session))
        .route("/api/snapshot", get(snapshot))
        .route("/api/transaction", post(mutate))
        .route("/api/events", get(events))
        .route("/report.html", get(report))
        .layer(DefaultBodyLimit::max(4 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(), guard))
        .with_state(state)
}
pub async fn serve(store: Store, port: u16) -> Result<()> {
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    let port = listener.local_addr()?.port();
    let (events, _) = watch::channel(store.snapshot()?.revision);
    let state = Arc::new(AppState {
        store: store.clone(),
        port,
        token: uuid::Uuid::new_v4().to_string(),
        events,
    });
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        match event {
            Err(err) => {
                eprintln!("hyp web: file watcher error (changes are still polled every 2 s): {err}")
            }
            Ok(event) if !changes_files(&event) => return,
            Ok(_) => {}
        }
        // Fails only after the monitor stopped, when nobody needs the wake-up.
        let _ = tx.send(());
    })?;
    watcher.watch(&store.root.join("hyp"), notify::RecursiveMode::Recursive)?;
    let monitor = state.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(2));
        // Logged once per distinct cause, not on every poll while it persists.
        let mut last_error: Option<String> = None;
        loop {
            tokio::select! {_ = tick.tick()=>{},event=rx.recv()=>{if event.is_none(){break;}tokio::time::sleep(Duration::from_millis(120)).await;while rx.try_recv().is_ok(){}}}
            let store = monitor.store.clone();
            let result = tokio::task::spawn_blocking(move || store.snapshot())
                .await
                .map_err(anyhow::Error::from)
                .and_then(|snapshot| snapshot);
            match result {
                Ok(snap) => {
                    if let Some(cause) = last_error.take() {
                        eprintln!("hyp web: project readable again (was: {cause})");
                    }
                    if *monitor.events.borrow() != snap.revision {
                        monitor.events.send_replace(snap.revision);
                    }
                }
                Err(err) => {
                    let cause = format!("{err:#}");
                    if last_error.as_ref() != Some(&cause) {
                        eprintln!("hyp web: cannot read project: {cause}");
                    }
                    last_error = Some(cause);
                    // Clients re-fetch /api/snapshot, whose error body carries the cause.
                    monitor.events.send_replace("unavailable".into());
                }
            }
        }
    });
    println!(
        "hyp WebUI: http://127.0.0.1:{port}\nProject: {}\nCtrl-C to stop",
        store.root.display()
    );
    // Dropping the server on Ctrl-C also closes long-lived SSE connections.
    // Tokio waits for in-flight blocking store operations when the runtime exits.
    tokio::select! {
        result = std::future::IntoFuture::into_future(axum::serve(listener, router(state))) => result?,
        _ = tokio::signal::ctrl_c() => {},
    }
    drop(watcher);
    Ok(())
}
/// Whether a file event may mean the project changed. On Linux, notify also
/// reports files being opened and read (inotify `IN_OPEN`), which every
/// snapshot does: counting those, each read woke the monitor for another
/// read, and an idle `hyp web` kept a core busy (HYPO-0004). Of the access
/// events only closing a file written to counts.
fn changes_files(event: &notify::Event) -> bool {
    use notify::event::{AccessKind, AccessMode, EventKind};
    match event.kind {
        EventKind::Access(access) => access == AccessKind::Close(AccessMode::Write),
        _ => true,
    }
}
pub fn export_html(s: &Snapshot) -> Result<String> {
    let json = serde_json::to_string(&WebSnapshot::of(s))?
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026");
    Ok(include_str!("../web/index.html")
        .replace(
            "<link rel=\"stylesheet\" href=\"/style.css\" />",
            &format!("<style>{}</style>", include_str!("../web/style.css")),
        )
        .replace(
            "<script src=\"/app.js\" defer></script>",
            &format!(
                "<script>window.HYP_EXPORT={json};</script><script defer>{}</script>",
                include_str!("../web/app.js")
            ),
        ))
}

#[cfg(test)]
mod tests {
    use super::changes_files;
    use notify::event::{AccessKind, AccessMode, CreateKind, EventKind, ModifyKind, RemoveKind};

    #[test]
    fn reading_files_is_not_a_change_but_writing_them_is() {
        let event = |kind| notify::Event::new(kind);
        for read in [
            AccessKind::Open(AccessMode::Any),
            AccessKind::Read,
            AccessKind::Close(AccessMode::Read),
            AccessKind::Any,
        ] {
            assert!(!changes_files(&event(EventKind::Access(read))), "{read:?}");
        }
        for change in [
            EventKind::Access(AccessKind::Close(AccessMode::Write)),
            EventKind::Create(CreateKind::File),
            EventKind::Modify(ModifyKind::Any),
            EventKind::Remove(RemoveKind::File),
            EventKind::Any,
            EventKind::Other,
        ] {
            assert!(changes_files(&event(change)), "{change:?}");
        }
    }
}
