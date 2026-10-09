pub mod auth;
pub mod files;
pub mod folders;
pub mod middleware;
pub mod openapi;
pub mod users;

use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{HeaderName, HeaderValue, Method, StatusCode},
    middleware as axum_middleware,
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use std::{str::FromStr, time::Duration};
use tower_http::{
    cors::{AllowCredentials, CorsLayer},
    trace::TraceLayer,
};
use tracing::Span;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use self::openapi::ApiDoc;
use crate::state::AppState;

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
        .route("/auth/login/2fa", post(auth::otp_login))
        .route("/auth/reset", post(auth::start_password_reset))
        .route("/auth/reset/{token}", post(auth::finish_password_reset));

    let session = Router::new()
        .route("/auth/me", get(auth::me))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/password", post(auth::change_password))
        .route("/auth/otp/setup", post(auth::start_otp_setup))
        .route("/auth/otp/setup/confirm", post(auth::finish_otp_setup))
        .route("/roots", get(files::roots))
        .route("/roots/{root_id}/users", get(folders::list_members))
        .route("/roots/{root_id}/usage", get(folders::usage))
        .route("/roots/{root_id}", get(files::list_files))
        .route(
            "/roots/{root_id}",
            post(files::upload).layer(DefaultBodyLimit::max(upload_max)),
        )
        .route(
            "/roots/{root_id}/directories",
            post(files::create_directory),
        )
        .route("/roots/{root_id}/files/{id}", get(files::metadata))
        .route("/roots/{root_id}/files/{id}/download", get(files::download))
        .route("/roots/{root_id}/files/{id}", delete(files::delete_file))
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
        .route("/admin/roots", get(folders::list_all))
        .route(
            "/roots/{root_id}",
            put(folders::update_root).delete(folders::delete_root),
        )
        .route("/roots/shared", post(folders::create_shared))
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

    Router::new()
        .merge(public)
        .merge(session)
        .merge(admin)
        .route("/_health", get(health))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(cors)
        .layer(
            TraceLayer::new_for_http()
                .on_request(|request: &axum::http::Request<_>, _span: &Span| {
                    tracing::info!(method = %request.method(), path = %request.uri().path(), "request");
                })
                .on_response(
                    |response: &axum::http::Response<_>, latency: Duration, _span: &Span| {
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
