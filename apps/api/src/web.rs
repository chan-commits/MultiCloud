use axum::{
    body::Body,
    extract::OriginalUri,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use multicloud_web_assets::Assets;

pub async fn serve(OriginalUri(uri): OriginalUri) -> Response {
    let requested_path = uri.path().trim_start_matches('/');
    let asset_path = if requested_path.is_empty() {
        "index.html"
    } else {
        requested_path
    };
    let Some((path, asset)) = Assets::get(asset_path)
        .map(|asset| (asset_path, asset))
        .or_else(|| Assets::get("index.html").map(|asset| ("index.html", asset)))
    else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let content_type = mime_guess::from_path(path)
        .first_or_octet_stream()
        .to_string();
    let cache_control = if path == "index.html" {
        "no-cache"
    } else {
        "public, max-age=31536000, immutable"
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, cache_control)
        .body(Body::from(asset.data.into_owned()))
        .expect("static asset response should be valid")
}
