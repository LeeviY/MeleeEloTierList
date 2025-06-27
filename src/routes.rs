use axum::{
    Router,
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::{Html, IntoResponse, Json},
    routing::{get, post},
};
use chrono::NaiveDateTime;
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use std::{fs, sync::Arc};
use tokio::sync::{Mutex, mpsc};
use tower_http::services::ServeDir;
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

use crate::{AppState, LastResult, Matchup, files, glicko, settings};

#[derive(OpenApi)]
#[openapi(
    paths(
        get_character_ratings, query_matches,
    ),
    components(
        schemas(MatchesQuery, MatchesQueryPlayer, files::Match, files::PlayerInfo, glicko::Player, files::Stage)
    ),
    tags(
        (name = "Matches", description = "Match-related endpoints")
    )
)]
struct ApiDoc;

#[derive(Deserialize, ToSchema)]
pub struct MatchesQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub min_length: Option<usize>,
    pub stage: Option<files::Stage>,
    pub players: (Option<MatchesQueryPlayer>, Option<MatchesQueryPlayer>),
    pub is_online: Option<bool>,
}

#[derive(Deserialize, ToSchema)]
pub struct MatchesQueryPlayer {
    pub code: Option<String>,
    pub character: Option<files::CSSCharacter>,
    pub stocks: Option<u8>,
    pub won: Option<bool>,
}

fn match_filter(m: &files::Match, q: &MatchesQuery) -> bool {
    let parse_timestamp = |s: &str| {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
            .map(|dt| dt.and_utc().timestamp())
            .ok()
    };

    if q.start_date
        .as_deref()
        .and_then(parse_timestamp)
        .is_some_and(|ts| m.datetime < ts)
        || q.end_date
            .as_deref()
            .and_then(parse_timestamp)
            .is_some_and(|ts| m.datetime > ts)
        || q.min_length.is_some_and(|min| m.frames < min)
        || q.stage.is_some_and(|st| m.stage != st)
        || q.is_online.is_some_and(|flag| m.is_online != flag)
    {
        return false;
    }

    let (q1, q2) = &q.players;
    let (p1, p2) = &m.players;

    q1.as_ref().is_none_or(|q| player_matches(p1, q))
        && q2.as_ref().is_none_or(|q| player_matches(p2, q))
}

fn player_matches(p: &files::PlayerInfo, q: &MatchesQueryPlayer) -> bool {
    q.code.as_ref().is_none_or(|c| &p.code == c)
        && q.character.is_none_or(|ch| p.character == ch)
        && q.stocks.is_none_or(|s| p.stocks == s)
        && q.won.is_none_or(|w| p.won == w)
}

pub fn create_tier_update_message(
    character_ratings: (Vec<glicko::Player>, Vec<glicko::Player>),
) -> String {
    serde_json::json!({
        "event": "tier_update",
        "data": {"P1": character_ratings.0, "P2": character_ratings.1}
    })
    .to_string()
}

pub fn create_results_update_message(last_results: (LastResult, LastResult)) -> String {
    serde_json::json!({
        "event": "results_update",
        "data": serde_json::json!({"P1": last_results.0, "P2": last_results.1})
    })
    .to_string()
}

pub fn create_matchup_update_message(
    matchup_chart: Vec<Vec<Matchup>>,
    last_match: Option<files::Match>,
) -> String {
    let winner = if let Some(m) = last_match {
        if m.players.0.won {
            settings::P1
        } else {
            settings::P2
        }
    } else {
        ""
    };

    serde_json::json!({
        "event": "matchup_update",
        "data": {
            "matchups": matchup_chart,
            "winner": winner,
        },
    })
    .to_string()
}

pub async fn handle_socket(mut socket: WebSocket, state: Arc<Mutex<AppState>>) {
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    let mut state_guard = state.lock().await;
    state_guard.clients.push(tx.clone());
    let character_ratings = state_guard.get_character_ratings_data();
    let matchup_chart = state_guard.get_matchup_update_data();
    let last_match = state_guard.last_match.clone();
    drop(state_guard);

    let _ = socket
        .send(Message::Text(
            create_tier_update_message(character_ratings).into(),
        ))
        .await;

    let _ = socket
        .send(Message::Text(
            create_matchup_update_message(matchup_chart, last_match).into(),
        ))
        .await;

    let (mut sender, mut receiver) = socket.split();
    let write_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(msg)) = receiver.next().await {
        if matches!(msg, Message::Close(_)) {
            break;
        }
    }

    write_task.abort();
}

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<Mutex<AppState>>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

pub fn create_router(state: Arc<Mutex<AppState>>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/ws", get(websocket_handler))
        .route("/matchup_chart", get(matchup_chart))
        .route("/stats", get(stats))
        .route("/character_ratings", get(get_character_ratings))
        .route("/matches/query", post(query_matches))
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state)
}

const FILE_NOT_FOUND: &str = "<html><body>File not found</body></html>";

// Route Handlers
pub async fn index() -> Html<String> {
    Html(fs::read_to_string("templates/index.html").unwrap_or_else(|_| FILE_NOT_FOUND.to_string()))
}

pub async fn matchup_chart() -> Html<String> {
    Html(
        fs::read_to_string("templates/matchup.html").unwrap_or_else(|_| FILE_NOT_FOUND.to_string()),
    )
}

pub async fn stats() -> Html<String> {
    Html(fs::read_to_string("templates/stats.html").unwrap_or_else(|_| FILE_NOT_FOUND.to_string()))
}

#[utoipa::path(
    get,
    path = "/character_ratings",
    responses(
        (status = 200, body = [Vec<glicko::Player>])
    )
)]
pub async fn get_character_ratings(
    State(state): State<Arc<Mutex<AppState>>>,
) -> Json<serde_json::Value> {
    let character_ratings = state.lock().await.get_character_ratings_data();
    Json(json!({
        "P1": character_ratings.0,
        "P2": character_ratings.1,
    }))
}

#[utoipa::path(
    post,
    path = "/matches/query",
    request_body = MatchesQuery,
    responses(
        (status = 200, body = [Vec<files::Match>]),
    )
)]
pub async fn query_matches(
    State(state): State<Arc<Mutex<AppState>>>,
    Json(query): Json<MatchesQuery>,
) -> Json<serde_json::Value> {
    Json(json!(
        state
            .lock()
            .await
            .get_values_filtered()
            .iter()
            .filter(|m| match_filter(m, &query))
            .cloned()
            .collect::<Vec<files::Match>>()
    ))
}
