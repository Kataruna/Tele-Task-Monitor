//! HTTP: static files from the web folder, `/api/stream` (SSE), and `/api/settings` (GET/POST).

use std::convert::Infallible;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tokio::sync::mpsc::UnboundedSender;
use tokio_stream::wrappers::WatchStream;
use tokio_stream::{Stream, StreamExt};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

use crate::sampler::SampleRx;
use crate::settings::{SettingsStore, UserSettings};
use crate::viewers::Viewers;

#[derive(Clone)]
pub struct AppState {
    /// Pre-serialized `Info` JSON, sent first on every connection.
    pub info_json: Arc<str>,
    pub samples: SampleRx,
    pub viewers: Arc<Viewers>,
    pub settings: SettingsStore,
    pub bind_ip: IpAddr,
    pub port_change_tx: Option<UnboundedSender<std::net::TcpListener>>,
}

pub fn router(state: AppState, web_dir: PathBuf) -> Router {
    Router::new()
        .route("/api/stream", get(stream))
        .route("/api/settings", get(get_settings).post(post_settings))
        .with_state(state)
        .fallback_service(ServeDir::new(web_dir))
        // no-cache: Safari revalidates on every refresh, so edits to web/ show up immediately.
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache"),
        ))
}

fn json_response(status: StatusCode, body: String) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, HeaderValue::from_static("application/json"))],
        body,
    )
        .into_response()
}

async fn get_settings(State(st): State<AppState>) -> Response {
    let cur = st.settings.get();
    let body = serde_json::to_string(&cur).unwrap_or_else(|_| "{}".into());
    json_response(StatusCode::OK, body)
}

async fn post_settings(State(st): State<AppState>, body: Bytes) -> Response {
    let incoming: UserSettings = match serde_json::from_slice(&body) {
        Ok(s) => s,
        Err(e) => {
            return json_response(
                StatusCode::BAD_REQUEST,
                format!(r#"{{"error":"Invalid JSON: {}"}}"#, e),
            );
        }
    };

    let prev = st.settings.get();
    let clean = incoming.sanitize();
    let port_changed = clean.port != prev.port;

    let mut bound_new_listener = None;
    if port_changed && st.port_change_tx.is_some() {
        match std::net::TcpListener::bind((st.bind_ip, clean.port)) {
            Ok(l) => {
                let _ = l.set_nonblocking(true);
                bound_new_listener = Some(l);
            }
            Err(e) => {
                return json_response(
                    StatusCode::BAD_REQUEST,
                    format!(
                        r#"{{"error":"Cannot bind to port {}: {}"}}"#,
                        clean.port, e
                    ),
                );
            }
        }
    }

    let saved = match st.settings.update(clean) {
        Ok(s) => s,
        Err(e) => {
            return json_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!(r#"{{"error":"{}"}}"#, e),
            );
        }
    };

    if let (Some(listener), Some(tx)) = (bound_new_listener, st.port_change_tx.clone()) {
        let new_port = saved.port;
        crate::tray::update_port(new_port);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(250)).await;
            let _ = tx.send(listener);
        });
    }

    let resp = serde_json::to_string(&saved).unwrap_or_else(|_| "{}".into());
    json_response(StatusCode::OK, resp)
}

async fn stream(State(st): State<AppState>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let guard = st.viewers.enter();
    let info = Event::default().event("info").data(&*st.info_json);
    let samples = WatchStream::from_changes(st.samples.clone())
        .filter_map(|s| s.map(|json| Event::default().event("sample").data(&*json)));
    let events = tokio_stream::once(info).chain(samples).map(move |e| {
        let _keep_viewer_alive = &guard; // dropped together with the stream when the client leaves
        Ok(e)
    });
    Sse::new(events).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use http_body_util::BodyExt;
    use tokio::sync::watch;
    use tower::ServiceExt;
    use crate::settings::SettingsStore;
    use crate::viewers::Viewers;

    fn temp_web_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ttm-web-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("index.html"), "<h1>hi</h1>").unwrap();
        dir
    }

    fn state() -> (AppState, watch::Sender<Option<Arc<str>>>) {
        let (tx, rx) = watch::channel(None);
        (
            AppState {
                info_json: r#"{"v":1}"#.into(),
                samples: rx,
                viewers: Viewers::new(),
                settings: SettingsStore::in_memory(),
                bind_ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                port_change_tx: None,
            },
            tx,
        )
    }

    async fn next_text(body: &mut Body) -> String {
        let frame = tokio::time::timeout(Duration::from_secs(2), body.frame())
            .await
            .expect("frame in time")
            .unwrap()
            .unwrap();
        String::from_utf8(frame.into_data().unwrap().to_vec()).unwrap()
    }

    #[tokio::test]
    async fn stream_sends_info_then_samples_and_tracks_viewers() {
        let (st, tx) = state();
        let viewers = Arc::clone(&st.viewers);
        let app = router(st, temp_web_dir("stream"));
        let res = app
            .oneshot(Request::get("/api/stream").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()[header::CONTENT_TYPE], "text/event-stream");

        let mut body = res.into_body();
        assert_eq!(next_text(&mut body).await, "event: info\ndata: {\"v\":1}\n\n");
        assert_eq!(viewers.count(), 1);

        tx.send_replace(Some(r#"{"ts":1}"#.into()));
        assert_eq!(next_text(&mut body).await, "event: sample\ndata: {\"ts\":1}\n\n");

        drop(body);
        assert_eq!(viewers.count(), 0, "disconnect releases the viewer");
    }

    #[tokio::test]
    async fn serves_index_with_no_cache() {
        let (st, _tx) = state();
        let app = router(st, temp_web_dir("index"));
        let res = app.oneshot(Request::get("/").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()[header::CACHE_CONTROL], "no-cache");
        let body = res.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"<h1>hi</h1>");
    }

    #[tokio::test]
    async fn missing_file_is_404() {
        let (st, _tx) = state();
        let app = router(st, temp_web_dir("missing"));
        let res = app.oneshot(Request::get("/nope.js").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn serves_nested_theme_files_with_no_cache() {
        let (st, _tx) = state();
        let dir = temp_web_dir("themes");
        let theme_dir = dir.join("themes").join("doraemon");
        std::fs::create_dir_all(&theme_dir).unwrap();
        std::fs::write(theme_dir.join("template.html"), "<div class='farm'></div>").unwrap();

        let app = router(st, dir);
        let res = app
            .oneshot(Request::get("/themes/doraemon/template.html").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()[header::CACHE_CONTROL], "no-cache");
        let body = res.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"<div class='farm'></div>");
    }

    #[tokio::test]
    async fn get_and_post_settings_round_trip() {
        let (st, _tx) = state();
        let app = router(st.clone(), temp_web_dir("settings"));
        let res = app
            .oneshot(
                Request::post("/api/settings")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"port":8810,"defaultTheme":"doraemon","overviewLimit":6,"pageLimit":20}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let app2 = router(st, temp_web_dir("settings2"));
        let res2 = app2
            .oneshot(Request::get("/api/settings").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res2.status(), StatusCode::OK);
        let bytes = res2.into_body().collect().await.unwrap().to_bytes();
        let got: UserSettings = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(got.port, 8810);
        assert_eq!(got.default_theme, "doraemon");
        assert_eq!(got.overview_limit_or(), (6, 20));
    }
}

#[cfg(test)]
impl UserSettings {
    fn overview_limit_or(&self) -> (usize, usize) {
        (self.overview_limit, self.page_limit)
    }
}
