use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use rust_embed::RustEmbed;

/// Embedded dashboard assets from the frontend build
#[derive(RustEmbed)]
#[folder = "../../frontend/out"]
struct DashboardAssets;

/// Create routes for serving the embedded dashboard
pub fn dashboard_routes() -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/*path", get(serve_static))
}

/// Serve the index.html for the root path
async fn serve_index() -> impl IntoResponse {
    serve_file("index.html")
}

/// Serve static files or fallback to index.html for SPA routing
async fn serve_static(req: Request<Body>) -> impl IntoResponse {
    let path = req.uri().path().trim_start_matches('/');

    // Try to serve the exact file
    if let Some(response) = try_serve_file(path) {
        return response;
    }

    // For paths without extensions (SPA routes), try to serve the directory's index.html
    if !path.contains('.') {
        // Try path/index.html (for directory routes)
        let index_path = if path.is_empty() {
            "index.html".to_string()
        } else {
            format!("{}/index.html", path.trim_end_matches('/'))
        };

        if let Some(response) = try_serve_file(&index_path) {
            return response;
        }

        // Fallback to root index.html for client-side routing
        return serve_file("index.html");
    }

    // File not found
    (StatusCode::NOT_FOUND, "Not found").into_response()
}

/// Try to serve a file, returning None if not found
fn try_serve_file(path: &str) -> Option<Response> {
    DashboardAssets::get(path).map(|content| {
        let mime = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();

        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime)
            .header(header::CACHE_CONTROL, cache_control(path))
            .body(Body::from(content.data.into_owned()))
            .unwrap()
    })
}

/// Serve a specific file (for index.html)
fn serve_file(path: &str) -> Response {
    match DashboardAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path)
                .first_or_octet_stream()
                .to_string();

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .header(header::CACHE_CONTROL, cache_control(path))
                .body(Body::from(content.data.into_owned()))
                .unwrap()
        }
        None => (StatusCode::NOT_FOUND, "Not found").into_response(),
    }
}

/// Determine cache control header based on file type
fn cache_control(path: &str) -> &'static str {
    if path.starts_with("_next/static/") {
        // Immutable static assets (hashed filenames)
        "public, max-age=31536000, immutable"
    } else if path.ends_with(".html") {
        // HTML files should not be cached aggressively
        "public, max-age=0, must-revalidate"
    } else {
        // Other assets
        "public, max-age=3600"
    }
}
