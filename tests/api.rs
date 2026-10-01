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
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let error: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(error["error"].as_str().unwrap().starts_with("conflict:"));
    // The same machine-readable kind as `hyp --json` (HYPO-0078); a
    // whole-project precondition names no records.
    assert_eq!(error["kind"], "conflict", "{error}");
    assert_eq!(error.get("ids"), None, "{error}");
}
/// POSTs a transaction and returns the status and the decoded JSON body.
async fn post(app: &axum::Router, body: serde_json::Value) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(req("POST", "/api/transaction", body))
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}
#[tokio::test]
async fn transactions_need_only_the_preconditions_of_their_changes() {
    let (_dir, app, store) = app();
    let hypothesis = |title: &str| serde_json::json!({"kind":"hypothesis","title":title,"scope":"","assumptions":"","lifecycle":"draft","untestable_reason":""});
    let (status, body) = post(
        &app,
        serde_json::json!({"changes":[{"op":"create","record":hypothesis("No project revision")}]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let read = store.snapshot().unwrap();
    let entry = read.objects[0].clone();
    // An unrelated write (another agent, through the CLI store) lands after the read.
    store
        .commit(
            vec![hyp::store::Change::create_seen(
                serde_json::from_value(hypothesis("Unrelated")).unwrap(),
                &read,
            )],
            None,
        )
        .unwrap();
    let mut record = serde_json::to_value(&entry.record).unwrap();
    record["title"] = "Edited after an unrelated write".into();
    let update =
        serde_json::json!({"op":"update","record":record,"expected_revision":entry.revision});
    let (status, body) = post(&app, serde_json::json!({"changes":[update.clone()]})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // The same per-object revision is now stale.
    let (status, body) = post(&app, serde_json::json!({"changes":[update]})).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(body["error"].as_str().unwrap().starts_with("conflict:"));
    assert_eq!(body["kind"], "conflict", "{body}");
    assert_eq!(body["ids"], serde_json::json!([entry.record.id]), "{body}");
    // A missing precondition is a request to fix (422), not a race to retry (409).
    let assessment = serde_json::json!({"kind":"assessment","title":"Unstated","body":"Why","hypothesis":entry.record.id,"judgment":"inconclusive"});
    let (status, body) = post(
        &app,
        serde_json::json!({"changes":[{"op":"create","record":assessment}]}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("requires `expected`"),
        "{body}"
    );
    assert_eq!(body["kind"], "invalid_input", "{body}");
}
/// Decision-0003 item 4 holds for the WebUI's API as for the CLI: a judgment
/// other than untested without evidence is input to fix (422).
#[tokio::test]
async fn a_judgment_without_evidence_is_unprocessable() {
    let (_dir, app, store) = app();
    hyp::cli::seed_demo(&store).unwrap();
    let s = store.snapshot().unwrap();
    let (h, state) = s.hypotheses.iter().next().unwrap();
    let change = |judgment: &str| {
        serde_json::json!({"changes":[{"op":"create",
            "record":{"kind":"assessment","title":"No evidence","body":"Why","hypothesis":h,"judgment":judgment},
            "expected":{"hypotheses":{h.clone():{"review_token":state.review_token}}}}]})
    };
    let (status, body) = post(&app, change("supported")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("a supported assessment must cite evidence"),
        "{body}"
    );
    let (status, body) = post(&app, change("untested")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
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
    assert_eq!(
        app.clone().oneshot(request).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let r = serde_json::json!({"kind":"hypothesis","title":"Forged","scope":"","assumptions":"","lifecycle":"draft","untestable_reason":""});
    let mut request = req(
        "POST",
        "/api/transaction",
        serde_json::json!({"expected_revision":store.snapshot().unwrap().revision,"changes":[{"op":"create","record":r}]}),
    );
    request
        .headers_mut()
        .insert("x-hyp-token", "wrong-token".parse().unwrap());
    assert_eq!(
        app.oneshot(request).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    assert!(store.snapshot().unwrap().objects.is_empty());
}
#[tokio::test]
async fn report_does_not_block_the_runtime_while_the_store_is_locked() {
    use fs2::FileExt;
    let (_dir, app, store) = app();
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(store.root.join(".hyp/write.lock"))
        .unwrap();
    lock.lock_exclusive().unwrap();
    let holder = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(1));
        drop(lock);
    });
    // On this single-threaded test runtime a handler that blocks on the lock
    // stalls the timer too, so the request would complete instead of timing out.
    let pending = tokio::time::timeout(
        std::time::Duration::from_millis(200),
        app.clone()
            .oneshot(req("GET", "/report.html", serde_json::Value::Null)),
    )
    .await;
    assert!(pending.is_err(), "report blocked the async runtime");
    holder.join().unwrap();
    let response = app
        .oneshot(req("GET", "/report.html", serde_json::Value::Null))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&bytes).contains("HYP_EXPORT"));
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
/// HYPO-0090: the WebUI gets a text preview of `text/*` data records inside
/// the snapshot JSON and no route serves stored bytes, so nothing captured
/// can reach the browser as content of its own type.
#[tokio::test]
async fn data_is_previewed_as_json_text_and_its_bytes_are_never_served() {
    let (_dir, app, store) = app();
    let capture = |bytes: &[u8], media_type: Option<&str>| {
        let c = store
            .capture(hyp::store::Capture {
                bytes: bytes.to_vec(),
                title: "Capture".into(),
                origin: "test".into(),
                media_type: media_type.map(str::to_string),
                name: None,
                note: String::new(),
                allow_empty: false,
            })
            .unwrap();
        c.written[0].id.clone()
    };
    let html = "<script>alert(1)</script>";
    let text = capture(html.as_bytes(), None);
    let as_html = capture(b"<b>x</b>", Some("text/html"));
    let binary = capture(&[0, 255], None);
    let get = |path: &str| {
        Request::builder()
            .uri(path)
            .header("host", "127.0.0.1:7432")
            .body(Body::empty())
            .unwrap()
    };
    let response = app.clone().oneshot(get("/api/snapshot")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response.headers()["content-type"]
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        content_type.starts_with("application/json"),
        "{content_type}"
    );
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let snapshot: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(snapshot["previews"][&text], html);
    assert_eq!(snapshot["previews"][&as_html], "<b>x</b>");
    assert!(snapshot["previews"].get(&binary).is_none());
    let sha = hyp::model::hash(html);
    for path in [
        format!("/assets/{sha}"),
        format!("/hyp/assets/{sha}"),
        format!("/api/data/{text}"),
    ] {
        let response = app.clone().oneshot(get(&path)).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
    }
}
