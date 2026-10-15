use crate::spend::{SpendState, UsageEntry};
use axum::{
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};

type Response = (StatusCode, Json<Value>);
fn error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({"error":message})))
}
fn authorized(state: &SpendState, headers: &HeaderMap) -> bool {
    let Ok(info) = state.info.lock() else {
        return false;
    };
    headers.get("authorization").and_then(|h| h.to_str().ok())
        == Some(format!("Bearer {}", info.token).as_str())
}

async fn health() -> Json<Value> {
    Json(json!({"service":"astra-dev-editor","mode":"usage-ingestion"}))
}

async fn usage(
    State(state): State<SpendState>,
    headers: HeaderMap,
    Json(entry): Json<UsageEntry>,
) -> Response {
    if !authorized(&state, &headers) {
        return error(StatusCode::UNAUTHORIZED, "Bearer token required");
    }
    let kinds = match crate::providers::accepted_provider_names(&entry.provider, &entry.model) {
        Ok(names) => names,
        Err(e) => return error(StatusCode::BAD_REQUEST, &e),
    };
    match state.record_allowing(&entry, &kinds) {
        Ok(inserted) => (
            StatusCode::OK,
            Json(json!({"recorded":inserted})),
        ),
        Err(e) => error(StatusCode::BAD_REQUEST, &e),
    }
}

pub fn router(state: SpendState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/usage", post(usage))
        .layer(DefaultBodyLimit::max(64 * 1024))
        .with_state(state)
}

pub fn start(state: SpendState) {
    tauri::async_runtime::spawn(async move {
        match tokio::net::TcpListener::bind("127.0.0.1:4000").await {
            Ok(listener) => {
                if let Ok(mut info) = state.info.lock() {
                    info.status = "listening".into();
                }
                if let Err(e) = axum::serve(listener, router(state.clone())).await {
                    if let Ok(mut info) = state.info.lock() {
                        info.status = format!("stopped: {e}");
                    }
                }
            }
            Err(e) => {
                if let Ok(mut info) = state.info.lock() {
                    info.status = format!("unavailable: {e}");
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn http_usage_end_to_end() {
        let dir = tempfile::tempdir().unwrap();
        let state = SpendState::open(dir.path().join("usage.db")).unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let token = state.info.lock().unwrap().token.clone();
        let server_state = state.clone();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let server = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    axum::serve(
                        tokio::net::TcpListener::from_std(listener).unwrap(),
                        router(server_state),
                    )
                    .with_graceful_shutdown(async {
                        let _ = shutdown_rx.await;
                    })
                    .await
                    .unwrap();
                });
        });
        let client = reqwest::blocking::Client::new();
        let url = format!("http://{addr}");
        assert_eq!(
            client
                .post(format!("{url}/v1/usage"))
                .json(&json!({"request_id":"r","provider":"hermes","model":"m","input_tokens":1,"output_tokens":1,"cost":0.0}))
                .send()
                .unwrap()
                .status(),
            401
        );
        let entry = json!({"request_id":"real-1","provider":"hermes","model":"test","input_tokens":25,"output_tokens":5,"cost":0.002,"session_id":"s1","reasoning_effort":"low","agent":"hermes"});
        for _ in 0..2 {
            assert!(client
                .post(format!("{url}/v1/usage"))
                .bearer_auth(&token)
                .json(&entry)
                .send()
                .unwrap()
                .status()
                .is_success());
        }
        assert_eq!(state.summary().unwrap().len(), 1);
        assert_eq!(state.summary().unwrap()[0].requests, 1);
        let bad = json!({"request_id":"bad","provider":"claude","model":"m","input_tokens":1,"output_tokens":1,"cost":0.0});
        assert_eq!(
            client
                .post(format!("{url}/v1/usage"))
                .bearer_auth(&token)
                .json(&bad)
                .send()
                .unwrap()
                .status(),
            400
        );
        shutdown_tx.send(()).unwrap();
        server.join().unwrap();
    }
}