use axum::{
    Json, Router,
    extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tracing::info;

use banana_i18n::parser::Parser;
use banana_i18n_mt::{
    GoogleTranslateProvider, MachineTranslator, MintProvider, Reassembler, prepare_for_translation,
};

/// Identifier of the backend used when a request omits `backend`.
const DEFAULT_BACKEND: &str = "mint";

#[derive(Serialize, Deserialize)]
pub struct TranslateRequest {
    pub message: String,
    pub target_language: String,
    pub key: String,
    /// Backend id (e.g. "mint", "google"). Defaults to MinT when omitted.
    #[serde(default)]
    pub backend: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct TranslateResponse {
    pub translated: String,
    pub source: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// One available translation backend, as advertised by `/api/backends`.
#[derive(Clone, Serialize)]
pub struct BackendInfo {
    /// Stable id sent back in translate requests (e.g. "mint").
    pub id: String,
    /// Human-readable name for display (e.g. "MinT").
    pub name: String,
}

#[derive(Serialize)]
pub struct BackendsResponse {
    pub backends: Vec<BackendInfo>,
    pub default: String,
}

#[derive(Clone)]
pub struct AppState {
    /// Available backends keyed by id, for per-request lookup.
    pub translators: Arc<HashMap<String, Arc<dyn MachineTranslator>>>,
    /// Backends in display order, for the `/api/backends` listing.
    pub backend_list: Arc<Vec<BackendInfo>>,
    /// Backend used when a request omits `backend`.
    pub default_backend: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("info".parse().unwrap()),
        )
        .init();

    // Assemble the available backends. MinT needs no API key and is always
    // available; Google is added only when GOOGLE_TRANSLATE_API_KEY is set.
    let mut translators: HashMap<String, Arc<dyn MachineTranslator>> = HashMap::new();
    let mut backend_list: Vec<BackendInfo> = Vec::new();

    let mint = MintProvider::from_env().map_err(|e| format!("Failed to initialize MinT: {}", e))?;
    backend_list.push(BackendInfo {
        id: "mint".to_string(),
        name: mint.provider_name().to_string(),
    });
    translators.insert("mint".to_string(), Arc::new(mint));

    match GoogleTranslateProvider::from_env() {
        Ok(google) => {
            backend_list.push(BackendInfo {
                id: "google".to_string(),
                name: google.provider_name().to_string(),
            });
            translators.insert("google".to_string(), Arc::new(google));
        }
        Err(_) => {
            info!("Google Translate disabled (GOOGLE_TRANSLATE_API_KEY not set)");
        }
    }

    let state = AppState {
        translators: Arc::new(translators),
        backend_list: Arc::new(backend_list),
        default_backend: DEFAULT_BACKEND.to_string(),
    };

    info!("🍌 Starting banana-i18n MT Web Server");
    info!(
        "Available backends: {}",
        state
            .backend_list
            .iter()
            .map(|b| b.id.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    // Build router
    let app = Router::new()
        .route("/", get(serve_index))
        .route("/api/translate", post(translate_message))
        .route("/api/backends", get(list_backends))
        .nest_service("/static", ServeDir::new("banana-i18n-mt-web/src/static"))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    info!("🚀 Server running at http://127.0.0.1:3000");

    axum::serve(listener, app).await?;

    Ok(())
}

async fn serve_index() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        include_str!("static/index.html"),
    )
}

/// List the translation backends available on this server, in display order.
async fn list_backends(State(state): State<AppState>) -> Json<BackendsResponse> {
    Json(BackendsResponse {
        backends: state.backend_list.as_ref().clone(),
        default: state.default_backend.clone(),
    })
}

async fn translate_message(
    State(state): State<AppState>,
    Json(request): Json<TranslateRequest>,
) -> Result<Json<TranslateResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Resolve the requested backend, falling back to the default.
    let backend_id = request
        .backend
        .clone()
        .unwrap_or_else(|| state.default_backend.clone());
    let translator = state.translators.get(&backend_id).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Unknown or unavailable backend: '{}'", backend_id),
            }),
        )
    })?;

    info!(
        "Translating message '{}' to {} (key: {}, backend: {})",
        &request.message, &request.target_language, &request.key, &backend_id
    );

    // Parse the source message
    let mut parser = Parser::new(&request.message);
    let ast = parser.parse();

    // Prepare for translation (expand to variants)
    let mut context = prepare_for_translation(&ast, "en", &request.key).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Failed to prepare message for translation: {}", e),
            }),
        )
    })?;

    // Get source texts for translation
    let source_texts = context.source_texts();

    // Translate using the selected provider
    let translated_texts = translator
        .translate_as_block(&source_texts, "en", &request.target_language)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Translation service error: {}", e),
                }),
            )
        })?;

    // Update context with translations
    context.update_translations(translated_texts);

    // Reassemble back to wikitext
    let reassembler = Reassembler::new(context.variable_types.clone());
    let translated_message = reassembler.reassemble(context.variants).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to reassemble message: {}", e),
            }),
        )
    })?;

    info!(
        "Successfully translated: {} → {}",
        &request.message, &translated_message
    );

    Ok(Json(TranslateResponse {
        translated: translated_message,
        source: request.message,
    }))
}
