use axum::{
    Json, Router,
    extract::State,
    http::{StatusCode, header},
    routing::{get, post},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tracing::info;

use banana_i18n::parser::Parser;
use banana_i18n_mt::{GoogleTranslateProvider, Reassembler, prepare_for_translation};

#[derive(Serialize, Deserialize)]
pub struct TranslateRequest {
    pub message: String,
    pub target_language: String,
    pub key: String,
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

#[derive(Clone)]
pub struct AppState {
    pub translator: Arc<GoogleTranslateProvider>,
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

    // Initialize Google Translate provider
    let translator = GoogleTranslateProvider::from_env()
        .map_err(|e| format!("Failed to initialize translator: {}", e))?;
    let state = AppState {
        translator: Arc::new(translator),
    };

    info!("🍌 Starting banana-i18n MT Web Server");

    // Build router
    let app = Router::new()
        .route("/", get(serve_index))
        .route("/api/translate", post(translate_message))
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

async fn translate_message(
    State(state): State<AppState>,
    Json(request): Json<TranslateRequest>,
) -> Result<Json<TranslateResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!(
        "Translating message '{}' to {} (key: {})",
        &request.message, &request.target_language, &request.key
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

    // Translate using the provider
    let translated_texts = state
        .translator
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
