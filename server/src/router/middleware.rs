use crate::{
    auth::session::Session,
    error::{AppError, AuthFailure},
    state::AppState,
    user::model::Role,
};
use axum::{
    extract::{FromRequestParts, Request},
    http::header,
    middleware::Next,
    response::{IntoResponse, Response},
};
use axum_extra::extract::CookieJar;
use uuid::Uuid;

/// Load the session from the session cookie or an `Authorization: Bearer` token.
///
/// Cookie requests require the CSRF header outside of debug builds. Bearer tokens are how
/// the desktop and mobile clients authenticate across origins.
pub async fn session_check(
    axum::extract::State(state): axum::extract::State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let session_id: Uuid = if let Some(header) = request.headers().get(header::AUTHORIZATION) {
        let value = match header.to_str() {
            Ok(value) => value,
            Err(e) => {
                tracing::error!("not a valid HTTP header value '{:?}': {}", header, e);
                return AppError::Auth(AuthFailure::InvalidCredentials).into_response();
            }
        };

        let Some(token) = value.strip_prefix("Bearer ") else {
            return AppError::Auth(AuthFailure::InvalidCredentials).into_response();
        };

        match token.parse() {
            Ok(id) => id,
            Err(e) => {
                tracing::error!("unable to parse session ID: {e}");
                return AppError::Auth(AuthFailure::InvalidCredentials).into_response();
            }
        }
    } else {
        if !request
            .headers()
            .contains_key(&*state.http_config.csrf_header)
        {
            return AppError::Auth(AuthFailure::CsrfHeaderNotFound).into_response();
        }

        match CookieJar::from_headers(request.headers()).get(state.http_config.cookie_name.as_ref())
        {
            Some(session_id) => match session_id.value().parse() {
                Ok(id) => id,
                Err(e) => {
                    tracing::error!("unable to parse session ID: {e}");
                    return AppError::Auth(AuthFailure::InvalidCredentials).into_response();
                }
            },
            None => return AppError::Auth(AuthFailure::SessionCookieNotFound).into_response(),
        }
    };

    let session = match state.services.auth.get_session_by_id(session_id).await {
        Ok(session) => session,
        Err(error) => {
            tracing::debug!(%error, "session not found");
            return AppError::Auth(AuthFailure::SessionNotFound).into_response();
        }
    };

    if session.is_expired() {
        return AppError::Auth(AuthFailure::SessionNotFound).into_response();
    }

    request.extensions_mut().insert(session);

    next.run(request).await
}

pub struct RemoteAddress(pub String);

impl<S> FromRequestParts<S> for RemoteAddress
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let address = parts
            .extensions
            .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
            .map(|info| info.0.ip().to_string())
            .unwrap_or_else(|| "127.0.0.1".to_string());
        Ok(Self(address))
    }
}

pub struct AdminSession(#[allow(dead_code)] pub Session);

impl FromRequestParts<AppState> for Session {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Session>()
            .cloned()
            .ok_or(AppError::Auth(AuthFailure::SessionCookieNotFound))
    }
}

impl FromRequestParts<AppState> for AdminSession {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, state).await?;
        if session.user.role != Role::Admin {
            return Err(AppError::Forbidden("admin role required".into()));
        }
        Ok(Self(session))
    }
}
