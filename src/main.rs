use anyhow::Context;
use askama_axum::Template;
use axum::async_trait;
use axum::extract::{FromRequestParts, Path, Query, State};
use axum::http::request::Parts;
use axum::http::{header, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{Html, IntoResponse};
use axum::routing::{get, post};
use axum::{Form, Router};
use clap::Parser;
use cookie::Cookie;
use futures_util::stream::Stream;
use serde::Deserialize;
use tokio_stream::StreamExt as _;
use tower_http::compression::CompressionLayer;

use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::convert::Infallible;
use std::ops::Deref;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

// import macros first before other modules
#[macro_use]
mod macros;

mod config;
mod middleware;
mod session;
mod template_helpers;
mod transmission;

#[cfg(target_os = "linux")]
mod unix_sock;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = config::Args::parse();

    let config = std::fs::read_to_string(&args.config).context(format!(
        r#"Failed to read configuration file "{}""#,
        args.config.display()
    ))?;

    // don't provide error context here since the toml error will be self explanatory
    let config: config::Config = toml::from_str(&config)?;

    let bind_addr = config.connection.bind_address.clone();
    let bind_unix_perms = config.connection.bind_unix_perms;
    let shared_state = Arc::new(AppState::new(config));

    #[rustfmt::skip]
    let app = Router::new()
        .route("/", get(index_get))
        .route("/login", get(login_get))
        .route("/login", post(login_post))
        .route("/logout", post(logout_post))
        .route("/start-torrent", post(start_torrent_post))
        .route("/pause-torrent", post(pause_torrent_post))
        .route("/verify-torrent", post(verify_torrent_post))
        .route("/add-torrent", get(add_torrent_get))
        .route("/add-torrent", post(add_torrent_post))
        .route("/torrent/:hash", get(torrent_get))
        .route("/stub/torrent", get(stub_torrent_get))
        .route("/stub/torrents", get(stub_torrents_get))
        .route("/sse/torrent", get(sse_torrent_get))
        .route("/sse/torrents", get(sse_torrents_get))
        .route("/static/app/manifest.json", json!("static/app/manifest.json"))
        .route("/static/css/base.css", css!("static/css/base.css"))
        .route("/static/css/index.css", css!("static/css/index.css"))
        .route("/static/js/htmx.js", js!("static/js/htmx.js"))
        .route("/static/js/sse.js", js!("static/js/sse.js"))
        .layer(axum::middleware::from_fn(middleware::unauthorized_redirect))
        .layer(axum::middleware::from_fn(middleware::compress_sse))
        .layer(CompressionLayer::new())
        .with_state(shared_state);

    match bind_addr {
        config::CompatSocketAddr::Ip(bind_addr) => {
            let listener = tokio::net::TcpListener::bind(bind_addr)
                .await
                .context(format!("Failed to bind to TCP address {bind_addr}"))?;

            axum::serve(listener, app)
                .await
                .context("Failed to serve the service")?
        }
        config::CompatSocketAddr::Unix(bind_addr) => {
            let bind_addr = bind_addr.path();

            #[cfg(target_os = "linux")]
            unix_sock::serve(bind_addr, bind_unix_perms, app).await?;

            // bsd and windows have support for path-based unix sockets, but they work a bit
            // differently so they would need more testing and changes to support
            #[cfg(not(target_os = "linux"))]
            return anyhow::anyhow!("Unix sockets aren't supported on this platform");
        }
    }

    Ok(())
}

#[derive(Debug)]
struct AppState {
    config: config::Config,
    sessions: session::SessionManager<transmission::rpc::TransmissionRpc>,
    // reqwest says that a `Client` is a pool of connections and we should reuse it, so we'll use it
    // for all rpc connections across all sessions
    http_client: reqwest::Client,
}

impl AppState {
    pub fn new(config: config::Config) -> Self {
        Self {
            config,
            sessions: Default::default(),
            http_client: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginQuery {
    username: String,
    password: String,
}

#[derive(Debug, Clone, Deserialize)]
struct TorrentQuery {
    hash: String,
}

#[derive(Debug, Clone, Deserialize)]
struct TorrentListQuery {
    #[serde(rename = "q")]
    filter: Option<String>,
    #[serde(rename = "dir")]
    sort_direction: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct AddTorrentQuery {
    magnet: String,
    paused: Option<String>,
}

#[derive(Template)]
#[template(path = "partials/torrent.html")]
struct TorrentPartialTemplate {
    details: BTreeMap<transmission::types::TorrentGetKey, serde_json::Value>,
}

#[derive(Template)]
#[template(path = "stubs/torrent.html")]
struct TorrentStubTemplate {
    hash: String,
    partial: TorrentPartialTemplate,
}

#[derive(Template)]
#[template(path = "partials/torrent-list.html")]
struct TorrentListPartialTemplate {
    torrents: Vec<BTreeMap<transmission::types::TorrentGetKey, serde_json::Value>>,
}

#[derive(Template)]
#[template(path = "stubs/torrent-list.html")]
struct TorrentListStubTemplate {
    filter: Option<String>,
    partial: TorrentListPartialTemplate,
}

async fn index_get(
    State(state): State<Arc<AppState>>,
    SessionArc(session): SessionArc,
    Query(TorrentListQuery {
        filter,
        sort_direction,
    }): Query<TorrentListQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let filter_str = filter.as_deref();
    let torrents = torrent_list(session.data(), &state.http_client, filter_str).await?;

    #[derive(Template)]
    #[template(path = "index.html")]
    struct IndexTemplate {
        ascending: bool,
        stub: TorrentListStubTemplate,
    }

    Ok(IndexTemplate {
        ascending: sort_direction.map(|x| x == "ascend").unwrap_or(false),
        stub: TorrentListStubTemplate {
            filter,
            partial: torrents,
        },
    })
}

async fn torrent_get(
    State(state): State<Arc<AppState>>,
    SessionArc(session): SessionArc,
    Path(hash): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let torrent = torrent_details(session.data(), &state.http_client, &hash).await?;

    let Some(torrent) = torrent else {
        return Err(StatusCode::NOT_FOUND);
    };

    #[derive(Template)]
    #[template(path = "torrent.html")]
    struct TorrentTemplate {
        stub: TorrentStubTemplate,
    }

    Ok(TorrentTemplate {
        stub: TorrentStubTemplate {
            hash,
            partial: torrent,
        },
    })
}

async fn login_get(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    #[derive(Template)]
    #[template(path = "login.html")]
    struct LoginTemplate {
        secure_cookie_attribute: bool,
    }

    LoginTemplate {
        secure_cookie_attribute: state.config.security.secure_cookie_attribute,
    }
}

async fn login_post(
    State(state): State<Arc<AppState>>,
    Form(login): Form<LoginQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let transmission_auth = transmission::rpc::TransmissionAuth {
        username: login.username,
        password: login.password,
    };

    let rpc = transmission::rpc::TransmissionRpc::new(
        state.config.connection.rpc_url.clone(),
        transmission_auth,
    );

    let session = session::Session::new(rpc);

    let request = transmission::types::Request::session_get(vec![
        transmission::types::SessionGetKey::Version,
    ]);
    let resp = session
        .data()
        .request::<transmission::types::SessionGetResponse>(&state.http_client, &request)
        .await;

    if matches!(resp, Err(StatusCode::UNAUTHORIZED)) {
        // could be wrong username/password
        return Ok((StatusCode::UNAUTHORIZED, None, "Not authorized"));
    }

    if matches!(resp, Err(StatusCode::FORBIDDEN)) {
        // could be the server connecting from a non-whitelisted IP
        return Ok((StatusCode::FORBIDDEN, None, "Forbidden"));
    }

    // make sure to raise any other errors
    let _resp = resp?;

    // if for some reason we can't compute the duration until the expiration, we'll just return a
    // session cookie instead of a persistent cookie
    let expire = session.expires().duration_since(SystemTime::now()).ok();

    let secret = state.sessions.new_session(session);
    let secret = secret.as_cookie(state.config.security.secure_cookie_attribute, expire);

    let cookie = format!("session_secret={secret}");
    let location = "/".to_string();

    Ok((
        StatusCode::SEE_OTHER,
        Some([(header::SET_COOKIE, cookie), (header::LOCATION, location)]),
        "Success",
    ))
}

async fn logout_post(
    State(state): State<Arc<AppState>>,
    headers: header::HeaderMap,
) -> Result<impl IntoResponse, StatusCode> {
    let session_secret = session_secret_from_headers(&headers)?;

    let _session = state
        .sessions
        .remove_session(session_secret)
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let cookie = "session_secret=; Secure; HttpOnly; SameSite=Lax; Max-Age=-1;";

    let html = r#"<meta http-equiv="refresh" content="0; url=/login"> Success. Redirecting."#;

    Ok(([(header::SET_COOKIE, cookie)], Html(html)))
}

async fn start_torrent_post(
    State(state): State<Arc<AppState>>,
    SessionArc(session): SessionArc,
    Form(TorrentQuery { hash }): Form<TorrentQuery>,
) -> Result<(), StatusCode> {
    #[derive(Deserialize)]
    struct Empty {}

    let request = transmission::types::Request::torrent_start(Some(vec![hash]));
    let _torrent_resp = session
        .data()
        .request::<Empty>(&state.http_client, &request)
        .await?;

    session.data().notify.notify_waiters();

    Ok(())
}

async fn pause_torrent_post(
    State(state): State<Arc<AppState>>,
    SessionArc(session): SessionArc,
    Form(TorrentQuery { hash }): Form<TorrentQuery>,
) -> Result<(), StatusCode> {
    #[derive(Deserialize)]
    struct Empty {}

    let request = transmission::types::Request::torrent_stop(Some(vec![hash]));
    let _torrent_resp = session
        .data()
        .request::<Empty>(&state.http_client, &request)
        .await?;

    session.data().notify.notify_waiters();

    Ok(())
}

async fn verify_torrent_post(
    State(state): State<Arc<AppState>>,
    SessionArc(session): SessionArc,
    Form(TorrentQuery { hash }): Form<TorrentQuery>,
) -> Result<(), StatusCode> {
    #[derive(Deserialize)]
    struct Empty {}

    let request = transmission::types::Request::torrent_verify(Some(vec![hash]));
    let _torrent_resp = session
        .data()
        .request::<Empty>(&state.http_client, &request)
        .await?;

    session.data().notify.notify_waiters();

    Ok(())
}

async fn add_torrent_get(
    // needed to verify that the user is logged in
    SessionArc(_session): SessionArc,
) -> Result<impl IntoResponse, StatusCode> {
    #[derive(Template)]
    #[template(path = "add-torrent.html")]
    struct AddTorrentTemplate;

    Ok(AddTorrentTemplate)
}

async fn add_torrent_post(
    State(state): State<Arc<AppState>>,
    SessionArc(session): SessionArc,
    Form(AddTorrentQuery { magnet, paused }): Form<AddTorrentQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    if !magnet.starts_with("magnet:?xt=urn:btih:") {
        println!(r#"Incorrect format for magnet link "{magnet}""#);
        return Err(StatusCode::BAD_REQUEST);
    }

    let paused = match paused.as_deref() {
        Some("on") => true,
        Some(_) => return Err(StatusCode::BAD_REQUEST),
        None => false,
    };

    let request = transmission::types::Request::torrent_add(
        transmission::types::TorrentAddRequired::Filename(magnet),
        /* paused= */ paused,
    );

    let resp = session
        .data()
        .request::<transmission::types::TorrentAddResponse>(&state.http_client, &request)
        .await?;

    session.data().notify.notify_waiters();

    // make sure we're not injecting weird content into the header
    let hash = resp.arguments.hash_string();
    assert!(hash.chars().all(char::is_alphanumeric));

    let location = format!("/torrent/{hash}");

    Ok((
        StatusCode::SEE_OTHER,
        Some([(header::LOCATION, location)]),
        "Success",
    ))
}

async fn stub_torrents_get(
    State(state): State<Arc<AppState>>,
    SessionArc(session): SessionArc,
    Query(TorrentListQuery {
        filter,
        sort_direction: _,
    }): Query<TorrentListQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let filter_str = filter.as_deref();
    let torrents = torrent_list(session.data(), &state.http_client, filter_str).await?;

    Ok(TorrentListStubTemplate {
        filter,
        partial: torrents,
    })
}

async fn stub_torrent_get(
    State(state): State<Arc<AppState>>,
    SessionArc(session): SessionArc,
    Form(TorrentQuery { hash }): Form<TorrentQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let torrent = torrent_details(session.data(), &state.http_client, &hash).await?;

    let Some(torrent) = torrent else {
        return Err(StatusCode::NOT_FOUND);
    };

    Ok(TorrentStubTemplate {
        hash,
        partial: torrent,
    })
}

async fn sse_torrents_get(
    State(state): State<Arc<AppState>>,
    SessionArc(session): SessionArc,
    Query(TorrentListQuery {
        filter,
        sort_direction: _,
    }): Query<TorrentListQuery>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = futures_util::stream::unfold(
        (session, state, filter, None),
        |(session, state, filter, last)| async move {
            let html = loop {
                let interval = Duration::from_millis(state.config.performance.poll_interval_ms);
                let _ = tokio::time::timeout(interval, session.data().notify.notified()).await;

                if session.expired() {
                    return None;
                }

                let filter = filter.as_deref();
                let torrents = torrent_list(session.data(), &state.http_client, filter)
                    .await
                    .ok()?;

                let html = torrents.render().unwrap();

                if let Some(ref last) = last {
                    if html != *last {
                        break html;
                    }
                } else {
                    break html;
                }
            };

            let event = Event::default().event("list").data(html.clone());
            Some((event, (session, state, filter, Some(html))))
        },
    )
    .map(Ok);

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("keep-alive-text"),
    )
}

async fn sse_torrent_get(
    State(state): State<Arc<AppState>>,
    SessionArc(session): SessionArc,
    Query(query): Query<TorrentQuery>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = futures_util::stream::unfold(
        (session, state, query, None),
        |(session, state, query, last)| async move {
            let html = loop {
                let interval = Duration::from_millis(state.config.performance.poll_interval_ms);
                let _ = tokio::time::timeout(interval, session.data().notify.notified()).await;

                if session.expired() {
                    return None;
                }

                let torrent = torrent_details(session.data(), &state.http_client, &query.hash)
                    .await
                    .ok()?;

                let Some(torrent) = torrent else {
                    return Some((
                        Event::default().event("removed").data("<b>Removed</b>"),
                        (session, state, query, None),
                    ));
                };

                let html = torrent.render().unwrap();

                if let Some(ref last) = last {
                    if html != *last {
                        break html;
                    }
                } else {
                    break html;
                }
            };

            let event = Event::default().event("details").data(html.clone());
            Some((event, (session, state, query, Some(html))))
        },
    )
    .map(Ok);

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("keep-alive-text"),
    )
}

async fn torrent_list(
    rpc: &transmission::rpc::TransmissionRpc,
    client: &reqwest::Client,
    filter: Option<&str>,
) -> Result<TorrentListPartialTemplate, StatusCode> {
    let request = transmission::types::Request::torrent_get(
        transmission::types::TorrentGetFormat::Objects,
        vec![
            transmission::types::TorrentGetKey::DateCreated,
            transmission::types::TorrentGetKey::AddedDate,
            transmission::types::TorrentGetKey::Id,
            transmission::types::TorrentGetKey::Name,
            transmission::types::TorrentGetKey::HashString,
            transmission::types::TorrentGetKey::PercentComplete,
            transmission::types::TorrentGetKey::PercentDone,
            transmission::types::TorrentGetKey::TotalSize,
            transmission::types::TorrentGetKey::Eta,
            transmission::types::TorrentGetKey::Status,
            transmission::types::TorrentGetKey::Labels,
        ],
        None,
    );
    let mut torrent_resp = rpc
        .request::<transmission::types::TorrentGetResponse>(client, &request)
        .await?;

    if let Some(filter) = filter {
        torrent_resp.arguments.torrents.retain(|torrent| {
            torrent
                .get(&transmission::types::TorrentGetKey::Name)
                .unwrap()
                .as_str()
                .unwrap()
                .to_lowercase()
                .contains(&filter.to_lowercase())
        });
    }

    torrent_resp.arguments.torrents.sort_by_cached_key(|x| {
        x.get(&transmission::types::TorrentGetKey::AddedDate)
            .and_then(|a| a.as_u64())
            .map(|a| u64::MAX - a)
    });

    Ok(TorrentListPartialTemplate {
        torrents: torrent_resp.arguments.torrents,
    })
}

async fn torrent_details(
    rpc: &transmission::rpc::TransmissionRpc,
    client: &reqwest::Client,
    hash: &str,
) -> Result<Option<TorrentPartialTemplate>, StatusCode> {
    let request = transmission::types::Request::torrent_get(
        transmission::types::TorrentGetFormat::Objects,
        vec![
            transmission::types::TorrentGetKey::DateCreated,
            transmission::types::TorrentGetKey::AddedDate,
            transmission::types::TorrentGetKey::Id,
            transmission::types::TorrentGetKey::Name,
            transmission::types::TorrentGetKey::HashString,
            transmission::types::TorrentGetKey::PercentComplete,
            transmission::types::TorrentGetKey::PercentDone,
            transmission::types::TorrentGetKey::Status,
        ],
        Some(vec![hash.to_string()]),
    );
    let mut torrent_resp = rpc
        .request::<transmission::types::TorrentGetResponse>(client, &request)
        .await?;

    if torrent_resp.arguments.torrents.is_empty() {
        return Ok(None);
    }

    Ok(Some(TorrentPartialTemplate {
        details: torrent_resp.arguments.torrents.swap_remove(0),
    }))
}

struct SessionArc(pub Arc<session::Session<transmission::rpc::TransmissionRpc>>);

#[async_trait]
impl<S: Send + Sync + Deref<Target = AppState>> FromRequestParts<S> for SessionArc {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(session_from_headers(state, &parts.headers)?))
    }
}

fn session_from_headers(
    state: &AppState,
    headers: &header::HeaderMap,
) -> Result<Arc<session::Session<transmission::rpc::TransmissionRpc>>, StatusCode> {
    let session_secret = session_secret_from_headers(headers)?;

    state
        .sessions
        .session(session_secret)
        .ok_or(StatusCode::UNAUTHORIZED)
}

fn session_secret_from_headers(
    headers: &header::HeaderMap,
) -> Result<session::SessionSecret, StatusCode> {
    let cookies = headers
        .get(header::COOKIE)
        .ok_or(StatusCode::UNAUTHORIZED)?
        .to_str()
        .or(Err(StatusCode::BAD_REQUEST))?;

    let mut cookies = Cookie::split_parse(cookies);

    let session_secret = cookies
        .find_map(|c| c.ok().filter(|c| c.name() == "session_secret"))
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let session_secret = session_secret.value();
    let session_secret = session_secret.parse().or(Err(StatusCode::BAD_REQUEST))?;

    Ok(session::SessionSecret::new(session_secret))
}
