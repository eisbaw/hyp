use crate::{
    model::Snapshot,
    store::{Change, Store},
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
use serde::Deserialize;
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
#[derive(Debug)]
struct ApiError(anyhow::Error);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let message = format!("{:#}", self.0);
        let status = if message.starts_with("conflict:") {
            StatusCode::CONFLICT
        } else {
            StatusCode::UNPROCESSABLE_ENTITY
        };
        (status, Json(serde_json::json!({"error":message}))).into_response()
    }
}
impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        Self(e)
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
async fn snapshot(State(s): State<Arc<AppState>>) -> std::result::Result<Json<Snapshot>, ApiError> {
    let store = s.store.clone();
    let snap = tokio::task::spawn_blocking(move || store.snapshot())
        .await
        .map_err(anyhow::Error::from)??;
    Ok(Json(snap))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Transaction {
    expected_revision: String,
    changes: Vec<Change>,
}
async fn mutate(
    State(s): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(tx): Json<Transaction>,
) -> std::result::Result<Json<Snapshot>, ApiError> {
    if headers.get("x-hyp-token").and_then(|v| v.to_str().ok()) != Some(s.token.as_str()) {
        return Err(ApiError(anyhow::anyhow!("invalid session token")));
    }
    if tx.changes.is_empty() || tx.changes.len() > 100 {
        return Err(ApiError(anyhow::anyhow!(
            "transaction must contain 1–100 changes"
        )));
    }
    let store = s.store.clone();
    let snap =
        tokio::task::spawn_blocking(move || store.commit(tx.changes, Some(&tx.expected_revision)))
            .await
            .map_err(anyhow::Error::from)??;
    s.events.send_replace(snap.revision.clone());
    Ok(Json(snap))
}
async fn events(
    State(s): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = std::result::Result<Event, Infallible>>> {
    let stream = WatchStream::new(s.events.subscribe())
        .map(|revision| Ok(Event::default().event("change").data(revision)));
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}
async fn report(State(s): State<Arc<AppState>>) -> std::result::Result<Html<String>, ApiError> {
    Ok(Html(export_html(&s.store.snapshot()?)?))
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
    let mut watcher = notify::recommended_watcher(move |_: notify::Result<notify::Event>| {
        let _ = tx.send(());
    })?;
    watcher.watch(&store.root.join("hyp"), notify::RecursiveMode::Recursive)?;
    let monitor = state.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(2));
        loop {
            tokio::select! {_ = tick.tick()=>{},event=rx.recv()=>{if event.is_none(){break;}tokio::time::sleep(Duration::from_millis(120)).await;while rx.try_recv().is_ok(){}}}
            let store = monitor.store.clone();
            match tokio::task::spawn_blocking(move || store.snapshot()).await {
                Ok(Ok(snap)) => {
                    if *monitor.events.borrow() != snap.revision {
                        monitor.events.send_replace(snap.revision);
                    }
                }
                _ => {
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
pub fn export_html(s: &Snapshot) -> Result<String> {
    let json = serde_json::to_string(s)?
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
