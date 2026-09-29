use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use hyp::{
    store::Store,
    web::{AppState, router},
};
use std::sync::Arc;
use tower::ServiceExt;
fn app() -> (tempfile::TempDir, axum::Router, Store) {
    let dir = tempfile::TempDir::new().unwrap();
    let store = Store::init(dir.path()).unwrap();
    let (events, _) = tokio::sync::watch::channel(String::new());
    let state = Arc::new(AppState {
        store: store.clone(),
        port: 7432,
        token: "test-token".into(),
        events,
    });
    (dir, router(state), store)
}
fn req(method: &str, path: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:7432")
        .header("content-type", "application/json")
        .header("x-hyp-token", "test-token")
        .body(Body::from(body.to_string()))
        .unwrap()
}
#[tokio::test]
async fn api_and_cli_store_share_state_and_reject_stale_writes() {
    let (_dir, app, store) = app();
    let revision = store.snapshot().unwrap().revision;
    let r = serde_json::json!({"kind":"hypothesis","title":"Created via API","scope":"","assumptions":"","lifecycle":"draft","untestable_reason":""});
    let body =
        serde_json::json!({"expected_revision":revision,"changes":[{"op":"create","record":r}]});
    let response = app
        .clone()
        .oneshot(req("POST", "/api/transaction", body.clone()))
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    assert_eq!(
        store.snapshot().unwrap().objects[0].record.title,
        "Created via API"
    );
    let response = app
        .oneshot(req("POST", "/api/transaction", body))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
}
#[tokio::test]
async fn rejects_foreign_host_origin_and_missing_token() {
    let (_dir, app, store) = app();
    let mut request = req("GET", "/api/snapshot", serde_json::Value::Null);
    request
        .headers_mut()
        .insert("host", "attacker.example:7432".parse().unwrap());
    assert_eq!(
        app.clone().oneshot(request).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let mut request = req("GET", "/api/snapshot", serde_json::Value::Null);
    request
        .headers_mut()
        .insert("origin", "https://attacker.example".parse().unwrap());
    assert_eq!(
        app.clone().oneshot(request).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let mut request = req(
        "POST",
        "/api/transaction",
        serde_json::json!({"expected_revision":store.snapshot().unwrap().revision,"changes":[]}),
    );
    request.headers_mut().remove("x-hyp-token");
    assert_ne!(app.oneshot(request).await.unwrap().status(), StatusCode::OK);
}
#[tokio::test]
async fn sse_delivers_initial_revision_and_security_headers() {
    let (_dir, app, _) = app();
    let response = app
        .oneshot(req("GET", "/api/events", serde_json::Value::Null))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    assert!(response.headers().contains_key("content-security-policy"));
    let mut body = response.into_body();
    let frame = tokio::time::timeout(std::time::Duration::from_secs(1), body.frame())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let text = String::from_utf8(frame.into_data().unwrap().to_vec()).unwrap();
    assert!(text.contains("event: change"));
}
