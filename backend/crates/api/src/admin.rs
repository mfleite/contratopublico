use axum::{
    Router,
    extract::{Multipart, State},
    http::StatusCode,
    response::IntoResponse,
    routing::post,
};
use serde::Serialize;
use tracing::{error, info};

use crate::{
    error::AppError,
    state::AppState,
};

pub fn admin_router() -> Router<AppState> {
    Router::new().route("/api/admin/ingest", post(ingest_handler))
}

#[derive(Debug, Serialize)]
struct IngestResponse {
    inserted: usize,
    skipped: usize,
    search_failures: usize,
    files_processed: usize,
}

#[axum::debug_handler]
async fn ingest_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    let mut total_inserted = 0usize;
    let mut total_skipped = 0usize;
    let mut total_search_failures = 0usize;
    let mut files_processed = 0usize;

    while let Ok(Some(field)) = multipart.next_field().await {
        let file_name = field
            .file_name()
            .unwrap_or("unknown.json")
            .to_string();

        if !file_name.ends_with(".json") {
            info!("Skipping non-JSON file: {file_name}");
            continue;
        }

        info!("Ingesting uploaded file: {file_name}");

        let bytes = field.bytes().await.map_err(|e| {
            error!("Failed to read uploaded file {file_name}: {e}");
            AppError::UploadReadError(e.to_string())
        })?;

        let raw_text = String::from_utf8(bytes.to_vec()).map_err(|e| {
            AppError::UploadReadError(format!("Invalid UTF-8 in {file_name}: {e}"))
        })?;

        let raw_contracts: Vec<scraper::ingest::JsonContract> =
            serde_json::from_str(&raw_text).map_err(|e| {
                AppError::JsonParseError(format!("Failed to parse {file_name}: {e}"))
            })?;

        let contract_db = state.contract_database();
        let search_db = state.search_database();

        let mut file_inserted = 0usize;
        let mut file_skipped = 0usize;
        let mut file_search_failures = 0usize;

        for raw_contract in raw_contracts {
            let contract = match common::Contract::try_from(raw_contract) {
                Ok(c) => c,
                Err(_e) => {
                    file_skipped += 1;
                    continue;
                }
            };

            if let Err(e) = contract_db.insert_contract(&contract).await {
                error!("Failed to insert contract {}: {e}", contract.id);
                file_skipped += 1;
                continue;
            }

            if let Err(e) = search_db.save_contract(contract).await {
                error!("Failed to index contract in search: {e}");
                file_search_failures += 1;
            }

            file_inserted += 1;
        }

        info!(
            "File {} processed: {} inserted, {} skipped",
            file_name, file_inserted, file_skipped
        );

        total_inserted += file_inserted;
        total_skipped += file_skipped;
        total_search_failures += file_search_failures;
        files_processed += 1;
    }

    let response = IngestResponse {
        inserted: total_inserted,
        skipped: total_skipped,
        search_failures: total_search_failures,
        files_processed,
    };

    Ok((StatusCode::OK, axum::Json(response)))
}
