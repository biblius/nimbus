use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use axum_extra::{
    extract::{
        TypedHeader,
        cookie::{Cookie, SameSite},
    },
    headers::UserAgent,
};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    auth::{
        model::{
            GeneratedOtp, LoginCredentials, LoginResult, Otp, PasswordChange, PasswordReset,
            PasswordResetRequest,
        },
        session::{Session, SessionView},
    },
    error::AppError,
    router::middleware::RemoteAddress,
    state::{AppState, HttpConfiguration},
    user::model::UserViewFull,
};

#[utoipa::path(
    post,
    path = "/auth/login",
    request_body = LoginCredentials,
    responses(
        (status = 200, description = "Session created or two-factor authentication required", body = LoginResult),
        (status = 401, description = "Invalid credentials"),
        (status = 422, description = "Validation error")
    )
)]
pub(crate) async fn credentials_login(
    State(state): State<AppState>,
    RemoteAddress(remote_address): RemoteAddress,
    user_agent: Option<TypedHeader<UserAgent>>,
    Json(credentials): Json<LoginCredentials>,
) -> Result<Response, AppError> {
    let user_agent = user_agent.map(|TypedHeader(agent)| agent.as_str().to_owned());
    let result = state
        .services
        .auth
        .login(credentials, Some(remote_address), user_agent)
        .await?;
    Ok(login_response(result, &state.http_config))
}

#[utoipa::path(
    post,
    path = "/auth/login/2fa",
    request_body = Otp,
    responses(
        (status = 200, description = "Session created", body = LoginResult),
        (status = 401, description = "Invalid OTP"),
        (status = 404, description = "Missing, used, or expired login token")
    )
)]
pub(crate) async fn otp_login(
    State(state): State<AppState>,
    RemoteAddress(remote_address): RemoteAddress,
    user_agent: Option<TypedHeader<UserAgent>>,
    Json(otp): Json<Otp>,
) -> Result<Response, AppError> {
    let user_agent = user_agent.map(|TypedHeader(agent)| agent.as_str().to_owned());
    let result = state
        .services
        .auth
        .login_with_otp(otp, Some(remote_address), user_agent)
        .await?;
    Ok(login_response(result, &state.http_config))
}

#[utoipa::path(
    post,
    path = "/auth/reset",
    request_body = PasswordResetRequest,
    responses((status = 204, description = "Reset email queued when the account exists"))
)]
pub(crate) async fn start_password_reset(
    State(state): State<AppState>,
    Json(data): Json<PasswordResetRequest>,
) -> Result<StatusCode, AppError> {
    state.services.auth.start_password_reset(data).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/auth/reset/{token}",
    params(("token" = Uuid, Path, description = "Password-reset token")),
    request_body = PasswordReset,
    responses((status = 204, description = "Password reset"))
)]
pub(crate) async fn finish_password_reset(
    State(state): State<AppState>,
    Path(token): Path<Uuid>,
    Json(data): Json<PasswordReset>,
) -> Result<StatusCode, AppError> {
    state
        .services
        .auth
        .finish_password_reset(token, data)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/auth/me",
    responses((status = 200, description = "Current user", body = UserViewFull))
)]
pub(crate) async fn me(session: Session) -> Json<UserViewFull> {
    Json(session.user.into())
}

#[utoipa::path(
    post,
    path = "/auth/logout",
    responses((status = 204, description = "Session invalidated"))
)]
pub(crate) async fn logout(
    State(state): State<AppState>,
    session: Session,
) -> Result<Response, AppError> {
    state.services.auth.logout(session.id).await?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    append_cookie(&mut response, expired_session_cookie(&state.http_config));
    Ok(response)
}

#[utoipa::path(
    post,
    path = "/auth/password",
    request_body = PasswordChange,
    responses((status = 204, description = "Password changed"))
)]
pub(crate) async fn change_password(
    State(state): State<AppState>,
    session: Session,
    Json(change): Json<PasswordChange>,
) -> Result<StatusCode, AppError> {
    state.services.auth.change_password(session, change).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/auth/otp/setup",
    responses((status = 200, description = "TOTP setup data", body = GeneratedOtp))
)]
pub(crate) async fn start_otp_setup(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<GeneratedOtp>, AppError> {
    Ok(Json(state.services.auth.start_otp_setup(session).await?))
}

#[utoipa::path(
    post,
    path = "/auth/otp/setup/confirm",
    request_body = Otp,
    responses((status = 204, description = "TOTP enabled"))
)]
pub(crate) async fn finish_otp_setup(
    State(state): State<AppState>,
    session: Session,
    Json(otp): Json<Otp>,
) -> Result<StatusCode, AppError> {
    state.services.auth.finish_otp_setup(session, otp).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn login_response(result: LoginResult, config: &HttpConfiguration) -> Response {
    let cookie = match &result {
        LoginResult::Session { session } => Some(session_cookie(session, config)),
        LoginResult::TwoFactorRequired { .. } => None,
    };
    let mut response = Json(result).into_response();
    if let Some(cookie) = cookie {
        append_cookie(&mut response, cookie);
    }
    response
}

fn append_cookie(response: &mut Response, cookie: Cookie<'_>) {
    match HeaderValue::from_str(&cookie.to_string()) {
        Ok(value) => {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
        Err(error) => tracing::error!(%error, "unable to create session cookie header"),
    }
}

fn session_cookie(session: &SessionView, config: &HttpConfiguration) -> Cookie<'static> {
    let mut cookie = Cookie::build((config.cookie_name.to_string(), session.id.to_string()))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(config.cookie_secure)
        .build();

    if !config.cookie_domain.is_empty() {
        cookie.set_domain(config.cookie_domain.to_string());
    }

    match session.expires_at {
        Some(expires_at) => {
            cookie.set_expires(OffsetDateTime::from_unix_timestamp(expires_at.timestamp()).ok());
        }
        None => cookie.make_permanent(),
    }

    cookie
}

fn expired_session_cookie(config: &HttpConfiguration) -> Cookie<'static> {
    let mut cookie = Cookie::build((config.cookie_name.to_string(), String::new()))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(config.cookie_secure)
        .max_age(time::Duration::ZERO)
        .build();

    if !config.cookie_domain.is_empty() {
        cookie.set_domain(config.cookie_domain.to_string());
    }

    cookie
}
