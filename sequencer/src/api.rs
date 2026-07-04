//! HTTP surface. Handlers translate requests into engine commands and back;
//! the engine thread does all state work. Encodings: Fr = 0x+64 lowercase
//! hex (canonical); amounts = decimal strings; addresses = strkey.

use crate::config::Config;
use crate::engine::{ApiError, Command, WireTx};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use std::sync::mpsc;
use tokio::sync::oneshot;

#[derive(Clone)]
pub struct AppState {
    pub engine: mpsc::Sender<Command>,
    pub cfg: Config,
}

impl ApiError {
    fn status(&self) -> StatusCode {
        match self {
            ApiError::BadField(_) | ApiError::BadSignature => StatusCode::BAD_REQUEST,
            ApiError::NonceMismatch { .. } | ApiError::InsufficientBalance { .. } => {
                StatusCode::CONFLICT
            }
            ApiError::RecipientUnknown | ApiError::AccountUnknown | ApiError::NotFound => {
                StatusCode::NOT_FOUND
            }
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
    fn code(&self) -> &'static str {
        match self {
            ApiError::BadField(_) => "BAD_FIELD",
            ApiError::BadSignature => "BAD_SIGNATURE",
            ApiError::NonceMismatch { .. } => "NONCE_MISMATCH",
            ApiError::InsufficientBalance { .. } => "INSUFFICIENT_BALANCE",
            ApiError::RecipientUnknown => "RECIPIENT_UNKNOWN",
            ApiError::AccountUnknown => "ACCOUNT_UNKNOWN",
            ApiError::NotFound => "NOT_FOUND",
            ApiError::Internal(_) => "INTERNAL",
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(serde_json::json!({
            "error": { "code": self.code(), "message": self.to_string() }
        }));
        (self.status(), body).into_response()
    }
}

/// Ask the engine to run a command, awaiting its oneshot reply.
async fn ask<T, F>(engine: &mpsc::Sender<Command>, build: F) -> Result<T, ApiError>
where
    F: FnOnce(oneshot::Sender<Result<T, ApiError>>) -> Command,
{
    let (tx, rx) = oneshot::channel();
    engine
        .send(build(tx))
        .map_err(|_| ApiError::Internal("engine offline".into()))?;
    rx.await.map_err(|_| ApiError::Internal("engine dropped reply".into()))?
}

/// Infallible variant for status/batch reads.
async fn ask_infallible<T, F>(engine: &mpsc::Sender<Command>, build: F) -> Result<T, ApiError>
where
    F: FnOnce(oneshot::Sender<T>) -> Command,
{
    let (tx, rx) = oneshot::channel();
    engine
        .send(build(tx))
        .map_err(|_| ApiError::Internal("engine offline".into()))?;
    rx.await.map_err(|_| ApiError::Internal("engine dropped reply".into()))
}

pub fn router(state: AppState) -> Router {
    use tower_http::cors::CorsLayer;
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/tx", post(post_tx))
        .route("/account/{pk_x}", get(get_account))
        .route("/da/{batch_num}", get(get_da))
        .route("/status", get(get_status))
        .route("/history/{pk_x}", get(get_history))
        .route("/batches", get(get_batches))
        .route("/params", get(get_params))
        // Permissive CORS is harmless behind the compose nginx /api proxy and
        // lets a Vercel-hosted build talk to the sequencer later.
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn post_tx(
    State(st): State<AppState>,
    Json(tx): Json<WireTx>,
) -> Result<impl IntoResponse, ApiError> {
    let receipt = ask(&st.engine, |reply| Command::SubmitTx(tx, reply)).await?;
    Ok(Json(receipt))
}

async fn get_account(
    State(st): State<AppState>,
    Path(pk_x): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let info = ask(&st.engine, |reply| Command::GetAccount(pk_x, reply)).await?;
    Ok(Json(info))
}

async fn get_da(
    State(st): State<AppState>,
    Path(batch_num): Path<u64>,
) -> Result<impl IntoResponse, ApiError> {
    let blob = ask(&st.engine, |reply| Command::GetDa(batch_num, reply)).await?;
    Ok(Json(blob))
}

async fn get_status(State(st): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let status = ask_infallible(&st.engine, Command::GetStatus).await?;
    Ok(Json(status))
}

async fn get_history(
    State(st): State<AppState>,
    Path(pk_x): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let entries = ask(&st.engine, |reply| Command::GetHistory(pk_x, reply)).await?;
    Ok(Json(serde_json::json!({ "entries": entries })))
}

async fn get_batches(State(st): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let batches = ask_infallible(&st.engine, Command::GetBatches).await?;
    Ok(Json(serde_json::json!({ "batches": batches })))
}

async fn get_params(State(st): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "contract_id": st.cfg.contract_id,
        "token_id": st.cfg.token_id,
        "network_passphrase": st.cfg.network_passphrase,
        "rpc_url": st.cfg.rpc_url,
        "batch": { "deposits": st.cfg.deposit_slots, "txs": st.cfg.tx_slots },
        "domains": { "leaf": 1, "tx": 2, "sig": 3, "dep": 4, "wd": 5, "addr": 6, "da": 7 },
    }))
}
