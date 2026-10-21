pub mod auth;
pub mod files;
pub mod folders;
pub mod middleware;
pub mod users;
mod web;

use axum::{
    Router,
    body::Body,
    extract::DefaultBodyLimit,
    http::{HeaderName, HeaderValue, Method, StatusCode, header},
    middleware as axum_middleware,
    response::{IntoResponse, Response, Sse, sse::Event},
    routing::{delete, get, post, put},
};
use axum_extra::{TypedHeader, headers::Range};
use futures::Stream;
use std::{
    convert::Infallible, io::SeekFrom, ops::Bound, path::Path, str::FromStr, time::Duration,
};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt},
    sync::mpsc,
};
use tokio_stream::StreamExt;
use tokio_util::io::ReaderStream;
use tower_http::{
    cors::{AllowCredentials, CorsLayer},
    trace::TraceLayer,
};
use tracing::Span;

use crate::{error::AppError, file::service::DownloadBody, state::AppState};

pub fn router(state: AppState) -> Router {
    let upload_max = state.upload_max_bytes;

    let origins = state.http_config.cors_origins.iter().filter_map(|origin| {
        match HeaderValue::from_str(origin) {
            Ok(value) => Some(value),
            Err(error) => {
                tracing::warn!(%error, origin, "skipping invalid CORS origin");
                None
            }
        }
    });

    let headers =
        state
            .http_config
            .cors_headers
            .iter()
            .filter_map(|name| match HeaderName::from_str(name) {
                Ok(value) => Some(value),
                Err(error) => {
                    tracing::warn!(%error, name, "skipping invalid CORS header");
                    None
                }
            });

    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::AllowOrigin::list(origins))
        .allow_headers(tower_http::cors::AllowHeaders::list(headers))
        .expose_headers([HeaderName::from_static(
            crate::auth::model::OTP_TOKEN_HEADER,
        )])
        .allow_credentials(AllowCredentials::yes())
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::DELETE,
            Method::PUT,
            Method::PATCH,
        ]);

    let public = Router::new()
        .route("/auth/login", post(auth::credentials_login))
        .route("/auth/login/otp", post(auth::otp_login))
        .route("/auth/login/native", post(auth::finish_native_login))
        .route("/auth/login/passkey", post(auth::start_passkey_login))
        .route(
            "/auth/login/passkey/confirm",
            post(auth::finish_passkey_login),
        )
        .route(
            "/auth/login/passkey/confirm/native",
            post(auth::start_native_passkey_login),
        )
        .route("/auth/reset", post(auth::start_password_reset))
        .route("/auth/reset/{token}", post(auth::finish_password_reset))
        .route("/files/shared/{file_id}", get(files::shared_file))
        .route(
            "/files/shared/{file_id}/children",
            get(files::shared_children),
        )
        .route(
            "/files/shared/{file_id}/blob/download",
            get(files::shared_download),
        )
        .route(
            "/files/shared/{file_id}/blob/chunk",
            get(files::shared_download_chunk),
        )
        .route(
            "/files/shared/{file_id}/preview",
            get(files::shared_preview),
        );

    let session = Router::new()
        .route("/auth/me", get(auth::me).patch(auth::update_me))
        .route("/auth/logout", post(auth::logout))
        .route(
            "/auth/sessions",
            get(auth::list_sessions).delete(auth::expire_other_sessions),
        )
        .route("/auth/sessions/{id}", delete(auth::expire_session))
        .route("/auth/password", post(auth::change_password))
        .route("/auth/otp/setup", post(auth::start_otp_setup))
        .route("/auth/otp/setup/confirm", post(auth::finish_otp_setup))
        .route("/auth/otp/disable", post(auth::disable_otp))
        .route("/auth/passkeys", get(auth::list_passkeys))
        .route("/auth/passkeys/{id}", delete(auth::delete_passkey))
        .route("/auth/passkey/setup", post(auth::start_passkey_setup))
        .route(
            "/auth/passkey/setup/confirm",
            post(auth::finish_passkey_setup),
        )
        .route("/roots", get(files::roots))
        .route("/roots/{root_id}/users", get(folders::list_members))
        .route("/roots/{root_id}/users/all", get(folders::list_all_members))
        .route("/roots/{root_id}/usage", get(folders::usage))
        .route("/roots/{root_id}/download", get(files::download_root))
        .route("/roots/{root_id}/files/download", get(files::download_dir))
        .route("/roots/{root_id}", get(files::list))
        .route(
            "/roots/{root_id}",
            post(files::upload).layer(DefaultBodyLimit::max(upload_max)),
        )
        .route(
            "/roots/{root_id}/upload/dir",
            post(files::upload_dir).layer(DefaultBodyLimit::max(upload_max)),
        )
        .route(
            "/roots/{root_id}/directories",
            post(files::create_directory),
        )
        .route("/roots/{root_id}/files", post(files::create_text_file))
        .route(
            "/roots/{root_id}/files/{file_id}/ancestors",
            get(files::ancestors),
        )
        .route(
            "/roots/{root_id}/files/{id}",
            get(files::metadata)
                .patch(files::rename_file)
                .delete(files::delete_file),
        )
        .route(
            "/roots/{root_id}/files/{id}/blob/download",
            get(files::download),
        )
        .route(
            "/roots/{root_id}/files/{id}/blob/chunk",
            get(files::download_chunk),
        )
        .route(
            "/roots/{root_id}/files/{id}/overwrite",
            put(files::overwrite).layer(DefaultBodyLimit::max(upload_max)),
        )
        .route("/roots/{root_id}/files/{id}/preview", get(files::preview))
        .route(
            "/roots/{root_id}/files/share",
            post(files::share_files).delete(files::unshare_files),
        )
        .route("/roots/{root_id}/tree", get(files::tree))
        .route("/roots/{root_id}/trash", get(files::trash))
        .route("/roots/{root_id}/trash/restore", post(files::restore_all))
        .route("/roots/{root_id}/trash/{id}", delete(files::delete_trash))
        .route(
            "/roots/{root_id}/trash/{id}/restore",
            post(files::restore_file),
        )
        .route("/roots/{root_id}/move", post(files::move_files))
        .route("/roots/{root_id}/copy", post(files::copy_files))
        .route("/roots/{root_id}/files", delete(files::delete_files))
        .route_layer(axum_middleware::from_fn_with_state(
            state.clone(),
            middleware::session_check,
        ));

    let admin = Router::new()
        .route("/users", post(users::create_user).get(users::list_users))
        .route(
            "/users/{id}",
            delete(users::delete_user).patch(users::update_user),
        )
        .route("/roots/private", get(folders::list_private))
        .route(
            "/roots/shared",
            get(folders::list_shared).post(folders::create_shared),
        )
        .route(
            "/roots/{root_id}",
            get(folders::get_root)
                .put(folders::update_root)
                .delete(folders::delete_root),
        )
        .route("/roots/{root_id}/quota", put(folders::set_quota))
        .route(
            "/roots/{root_id}/users/{user_id}",
            post(folders::assign).delete(folders::unassign),
        )
        .route_layer(axum_middleware::from_fn(middleware::admin_check))
        .route_layer(axum_middleware::from_fn_with_state(
            state.clone(),
            middleware::session_check,
        ));

    let api = Router::new()
        .merge(public)
        .merge(session)
        .nest("/admin", admin);

    Router::new()
        .route("/_health", get(health))
        .nest("/api", api)
        .fallback(web::static_handler)
        .layer(cors)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &axum::http::Request<_>| {
                    if request.uri().path().starts_with("/api") {
                        tracing::info_span!("api")
                    } else {
                        Span::none()
                    }
                })
                .on_request(|request: &axum::http::Request<_>, span: &Span| {
                    if span.is_disabled() {
                        return;
                    }

                    tracing::info!(
                        method = %request.method(),
                        path = %request.uri().path(),
                        query = %request.uri().query().unwrap_or(""),
                        "request"
                    );
                })
                .on_response(
                    |response: &axum::http::Response<_>, latency: Duration, span: &Span| {
                        if span.is_disabled() {
                            return;
                        }

                        tracing::info!(
                            status = %response.status(),
                            elapsed_ms = latency.as_millis(),
                            "response"
                        );
                    },
                ),
        )
        .with_state(state)
}

async fn health() -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

pub fn sse<T, F>(
    rx: mpsc::Receiver<T>,
    map_fn: F,
) -> Sse<impl Stream<Item = Result<Event, Infallible>> + Send>
where
    T: Send + 'static,
    F: FnMut(T) -> Event + Send + 'static,
{
    let stream = tokio_stream::wrappers::ReceiverStream::new(rx).map(map_fn);

    Sse::new(stream.map(Ok)).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(1))
            .text("keep-alive"),
    )
}

impl DownloadBody {
    /// Build an attachment response. Files include `Content-Length`; zips are chunked.
    pub async fn into_response(self, name: String) -> Result<Response, AppError> {
        match self {
            Self::File { path, mime } => {
                let file = File::open(&path).await?;
                let len = file.metadata().await?.len();

                let essence = mime
                    .split_once(';')
                    .map(|(type_, _)| type_.trim())
                    .unwrap_or(mime);

                let safe_mime = match essence {
                    "text/html"
                    | "application/xhtml+xml"
                    | "text/xml"
                    | "application/xml"
                    | "text/javascript"
                    | "application/javascript"
                    | "text/jscript"
                    | "text/css"
                    | "image/svg+xml" => "text/plain; charset=utf-8",
                    _ => mime,
                };

                Response::builder()
                    .header(header::CONTENT_TYPE, safe_mime)
                    .header(
                        header::CONTENT_DISPOSITION,
                        content_disposition(&name, false),
                    )
                    .header(header::CONTENT_LENGTH, len.to_string())
                    .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
                    .header(
                        header::CONTENT_SECURITY_POLICY,
                        "default-src 'none'; sandbox",
                    )
                    .body(Body::from_stream(ReaderStream::new(file)))
                    .map_err(|error| {
                        tracing::error!(%error, "building download response");
                        AppError::Internal
                    })
            }
            Self::Zip { reader } => Response::builder()
                .header(header::CONTENT_TYPE, "application/zip")
                .header(
                    header::CONTENT_DISPOSITION,
                    content_disposition(&name, false),
                )
                .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
                .header(
                    header::CONTENT_SECURITY_POLICY,
                    "default-src 'none'; sandbox",
                )
                .body(Body::from_stream(ReaderStream::new(reader)))
                .map_err(|error| {
                    tracing::error!(%error, "building zip download response");
                    AppError::Internal
                }),
        }
    }
}

/// Stream a file with optional HTTP Range support (for `<video>` / `<audio>`).
///
/// Without a Range header the whole file is returned with `Accept-Ranges: bytes`.
/// With a Range header the first satisfiable byte range is served as `206`.
///
/// Active document types (`text/html`, scripts, …) are rewritten to plain text
/// and forced to `attachment` so a top-level navigation cannot execute user
/// content on the API origin. Media and PDF stay inline with their real MIME.
pub(crate) async fn chunked_download(
    path: &Path,
    name: &str,
    mime: &'static str,
    range: Option<TypedHeader<Range>>,
) -> Result<Response, AppError> {
    let mut file = File::open(path).await?;
    let file_bytes = file.metadata().await?.len();

    // Neutralize browser-executable types; keep media/PDF inline for players.
    let essence = mime
        .split_once(';')
        .map(|(type_, _)| type_.trim())
        .unwrap_or(mime);
    let (safe_mime, inline) = match essence {
        "text/html"
        | "application/xhtml+xml"
        | "text/xml"
        | "application/xml"
        | "text/javascript"
        | "application/javascript"
        | "text/jscript"
        | "text/css"
        | "image/svg+xml" => ("text/plain; charset=utf-8", false),
        m if m.starts_with("image/")
            || m.starts_with("video/")
            || m.starts_with("audio/")
            || m == "application/pdf" =>
        {
            (mime, true)
        }
        _ => (mime, false),
    };
    let disposition = content_disposition(name, inline);

    let Some(TypedHeader(range)) = range else {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::CONTENT_TYPE, safe_mime)
            .header(header::CONTENT_DISPOSITION, &disposition)
            .header(header::CONTENT_LENGTH, file_bytes.to_string())
            .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
            .header(
                header::CONTENT_SECURITY_POLICY,
                "default-src 'none'; sandbox",
            )
            .body(Body::from_stream(ReaderStream::new(file)))
            .map_err(|error| {
                tracing::error!(%error, "building chunk download response");
                AppError::Internal
            });
    };

    // Browsers request a single range for video seek/buffer; serve the first satisfiable one.
    let Some((start_bound, end_bound)) = range.satisfiable_ranges(file_bytes).next() else {
        return Response::builder()
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(header::CONTENT_RANGE, format!("bytes */{file_bytes}"))
            .body(Body::empty())
            .map_err(|error| {
                tracing::error!(%error, "building range-not-satisfiable response");
                AppError::Internal
            });
    };

    let start = match start_bound {
        Bound::Included(value) => value,
        Bound::Excluded(value) => value.saturating_add(1),
        Bound::Unbounded => 0,
    };

    let end = match end_bound {
        Bound::Included(value) => value,
        Bound::Excluded(value) => value.saturating_sub(1),
        Bound::Unbounded => file_bytes.saturating_sub(1),
    };

    if start > end || start >= file_bytes {
        return Response::builder()
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(header::CONTENT_RANGE, format!("bytes */{file_bytes}"))
            .body(Body::empty())
            .map_err(|error| {
                tracing::error!(%error, "building range-not-satisfiable response");
                AppError::Internal
            });
    }

    let len = end - start + 1;

    file.seek(SeekFrom::Start(start)).await?;

    let chunk = file.take(len);

    Response::builder()
        .status(StatusCode::PARTIAL_CONTENT)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_TYPE, safe_mime)
        .header(header::CONTENT_DISPOSITION, &disposition)
        .header(header::CONTENT_LENGTH, len.to_string())
        .header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{file_bytes}"),
        )
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(
            header::CONTENT_SECURITY_POLICY,
            "default-src 'none'; sandbox",
        )
        .body(Body::from_stream(ReaderStream::new(chunk)))
        .map_err(|error| {
            tracing::error!(%error, "building partial chunk response");
            AppError::Internal
        })
}

pub(crate) fn content_disposition(name: &str, inline: bool) -> String {
    let name: String = name
        .chars()
        .map(|character| {
            if character.is_ascii()
                && character != '"'
                && character != '\\'
                && !character.is_control()
            {
                character
            } else {
                '_'
            }
        })
        .collect();

    let kind = if inline { "inline" } else { "attachment" };

    format!("{kind}; filename=\"{name}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn html_chunk_response_is_plain_attachment_with_security_headers() {
        let path =
            std::env::temp_dir().join(format!("nimbus-safe-mime-{}.html", uuid::Uuid::new_v4()));
        tokio::fs::write(&path, b"<script>alert(1)</script>")
            .await
            .unwrap();

        let response = chunked_download(&path, "evil.html", "text/html", None)
            .await
            .unwrap();

        let _ = tokio::fs::remove_file(&path).await;

        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/plain; charset=utf-8"
        );
        let disposition = response
            .headers()
            .get(header::CONTENT_DISPOSITION)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(
            disposition.starts_with("attachment;"),
            "disposition={disposition}"
        );
        assert_eq!(
            response
                .headers()
                .get(header::X_CONTENT_TYPE_OPTIONS)
                .unwrap(),
            "nosniff"
        );
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_SECURITY_POLICY)
                .unwrap(),
            "default-src 'none'; sandbox"
        );

        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(&body[..], b"<script>alert(1)</script>");
    }

    #[tokio::test]
    async fn png_chunk_response_stays_inline() {
        let path =
            std::env::temp_dir().join(format!("nimbus-safe-mime-{}.png", uuid::Uuid::new_v4()));
        tokio::fs::write(&path, b"\x89PNG").await.unwrap();

        let response = chunked_download(&path, "shot.png", "image/png", None)
            .await
            .unwrap();

        let _ = tokio::fs::remove_file(&path).await;

        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "image/png"
        );
        let disposition = response
            .headers()
            .get(header::CONTENT_DISPOSITION)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(
            disposition.starts_with("inline;"),
            "disposition={disposition}"
        );
    }
}
