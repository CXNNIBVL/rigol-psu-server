//! Serves the Svelte bundle, embedded into the binary at compile time.
//!
//! Embedding rather than shipping a directory means deployment is a single file
//! with no runtime asset paths to get wrong, and no way for the binary and the
//! UI to drift apart.

use axum::extract::Path as AxumPath;
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use rust_embed::Embed;

use crate::routes::AppState;

#[derive(Embed)]
#[folder = "$CARGO_MANIFEST_DIR/../../ui/dist"]
struct Assets;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/{*path}", get(asset))
}

async fn index() -> Response {
    serve("index.html")
}

async fn asset(AxumPath(path): AxumPath<String>, uri: Uri) -> Response {
    // Anything that is not a real file falls back to the SPA entry point, so a
    // deep link or a refresh lands on the app rather than a 404.
    if Assets::get(&path).is_some() {
        serve(&path)
    } else if uri.path().starts_with("/api") {
        (StatusCode::NOT_FOUND, "no such endpoint").into_response()
    } else {
        serve("index.html")
    }
}

fn serve(path: &str) -> Response {
    match Assets::get(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref())],
                file.data.into_owned(),
            )
                .into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            "the UI bundle is missing from this build",
        )
            .into_response(),
    }
}
