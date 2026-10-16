use axum::{
    Json,
    body::Body,
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
            DisableOtp, FinishNativeLogin, FinishPasskeyLogin, FinishPasskeySetup,
            LoginCredentials, LoginResult, OTP_TOKEN_HEADER, Otp, PasskeyLoginChallenge,
            PasskeySetupChallenge, PasswordChange, PasswordReset, PasswordResetRequest,
            StartPasskeyLogin, UserPasskeyView,
        },
        session::{ActiveSessionView, Session},
    },
    error::AppError,
    router::middleware::RemoteAddress,
    state::{AppState, HttpConfiguration},
    user::model::{UpdateSelf, UserViewFull},
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

    login_response(result, &state).await
}

#[utoipa::path(
    post,
    path = "/auth/login/otp",
    request_body = Otp,
    responses(
        (status = 200, description = "Session created", body = LoginResult),
        (status = 401, description = "Invalid OTP"),
        (status = 404, description = "Missing, used, or expired login token"),
        (status = 422, description = "Validation error")
    )
)]
pub(crate) async fn otp_login(
    State(state): State<AppState>,
    RemoteAddress(remote_address): RemoteAddress,
    user_agent: Option<TypedHeader<UserAgent>>,
    Json(login): Json<Otp>,
) -> Result<Response, AppError> {
    let user_agent = user_agent.map(|TypedHeader(agent)| agent.as_str().to_owned());

    let result = state
        .services
        .auth
        .login_with_otp(login, Some(remote_address), user_agent)
        .await?;

    login_response(result, &state).await
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
    patch,
    path = "/auth/me",
    request_body = UpdateSelf,
    responses((status = 200, description = "Current user updated", body = UserViewFull))
)]
pub(crate) async fn update_me(
    State(state): State<AppState>,
    session: Session,
    Json(input): Json<UpdateSelf>,
) -> Result<Json<UserViewFull>, AppError> {
    Ok(Json(
        state
            .services
            .users
            .update_self(session.user.id, session.id, input)
            .await?,
    ))
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

    Ok(clear_session_cookie(
        StatusCode::NO_CONTENT.into_response(),
        &state.http_config,
    ))
}

#[utoipa::path(
    get,
    path = "/auth/sessions",
    responses(
        (status = 200, description = "Live sessions for the current user", body = Vec<ActiveSessionView>)
    )
)]
pub(crate) async fn list_sessions(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<Vec<ActiveSessionView>>, AppError> {
    Ok(Json(state.services.auth.list_sessions(&session).await?))
}

#[utoipa::path(
    delete,
    path = "/auth/sessions",
    responses((status = 204, description = "All other live sessions expired"))
)]
pub(crate) async fn expire_other_sessions(
    State(state): State<AppState>,
    session: Session,
) -> Result<StatusCode, AppError> {
    state.services.auth.expire_other_sessions(&session).await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/auth/sessions/{id}",
    params(("id" = Uuid, Path, description = "Session id to expire")),
    responses(
        (status = 204, description = "Session expired"),
        (status = 404, description = "Session missing or not owned")
    )
)]
pub(crate) async fn expire_session(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let current = id == session.id;

    state.services.auth.expire_session(&session, id).await?;

    if current {
        return Ok(clear_session_cookie(
            StatusCode::NO_CONTENT.into_response(),
            &state.http_config,
        ));
    }

    Ok(StatusCode::NO_CONTENT.into_response())
}

fn clear_session_cookie(mut response: Response, config: &HttpConfiguration) -> Response {
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

    match HeaderValue::from_str(&cookie.to_string()) {
        Ok(value) => {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
        Err(error) => tracing::error!(%error, "unable to create session cookie header"),
    }

    response
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
    responses(
        (
            status = 200,
            description = "TOTP QR code as SVG. Confirmation token is in the X-Otp-Token response header.",
            content_type = "image/svg+xml",
            body = String
        )
    )
)]
pub(crate) async fn start_otp_setup(
    State(state): State<AppState>,
    session: Session,
) -> Result<Response, AppError> {
    let generated = state.services.auth.start_otp_setup(session).await?;

    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/svg+xml; charset=utf-8")
        .body(Body::from(generated.qr))
        .map_err(|error| {
            tracing::error!(%error, "unable to build OTP QR response");
            AppError::Internal
        })?;

    match HeaderValue::from_str(&generated.token.to_string()) {
        Ok(value) => {
            response.headers_mut().insert(OTP_TOKEN_HEADER, value);
        }
        Err(error) => {
            tracing::error!(%error, "unable to create OTP token header");
            return Err(AppError::Internal);
        }
    }

    Ok(response)
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
    Json(data): Json<Otp>,
) -> Result<StatusCode, AppError> {
    state.services.auth.finish_otp_setup(session, data).await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/auth/otp/disable",
    request_body = DisableOtp,
    responses(
        (status = 204, description = "TOTP disabled"),
        (status = 401, description = "Invalid OTP"),
        (status = 422, description = "2FA is not enabled")
    )
)]
pub(crate) async fn disable_otp(
    State(state): State<AppState>,
    session: Session,
    Json(data): Json<DisableOtp>,
) -> Result<StatusCode, AppError> {
    state.services.auth.disable_otp(session, data).await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/auth/passkeys",
    responses((status = 200, description = "Registered passkeys", body = Vec<UserPasskeyView>))
)]
pub(crate) async fn list_passkeys(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<Vec<UserPasskeyView>>, AppError> {
    Ok(Json(state.services.auth.list_passkeys(&session).await?))
}

pub(crate) async fn start_passkey_setup(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<PasskeySetupChallenge>, AppError> {
    Ok(Json(
        state.services.auth.start_passkey_setup(session).await?,
    ))
}

pub(crate) async fn finish_passkey_setup(
    State(state): State<AppState>,
    session: Session,
    Json(data): Json<FinishPasskeySetup>,
) -> Result<StatusCode, AppError> {
    state
        .services
        .auth
        .finish_passkey_setup(session, data)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/auth/login/passkey",
    request_body = StartPasskeyLogin,
    responses(
        (status = 200, description = "WebAuthn authentication challenge"),
        (status = 404, description = "Missing, used, or expired login token"),
        (status = 422, description = "No passkeys registered")
    )
)]
pub(crate) async fn start_passkey_login(
    State(state): State<AppState>,
    Json(data): Json<StartPasskeyLogin>,
) -> Result<Json<PasskeyLoginChallenge>, AppError> {
    Ok(Json(state.services.auth.start_passkey_login(data).await?))
}

pub(crate) async fn finish_passkey_login(
    State(state): State<AppState>,
    RemoteAddress(remote_address): RemoteAddress,
    user_agent: Option<TypedHeader<UserAgent>>,
    Json(data): Json<FinishPasskeyLogin>,
) -> Result<Response, AppError> {
    let user_agent = user_agent.map(|TypedHeader(agent)| agent.as_str().to_owned());

    let result = state
        .services
        .auth
        .finish_passkey_login(data, Some(remote_address), user_agent)
        .await?;

    login_response(result, &state).await
}

/// Browser half of native passkey 2FA: verify the assertion, do not create a session.
pub(crate) async fn start_native_passkey_login(
    State(state): State<AppState>,
    Json(data): Json<FinishPasskeyLogin>,
) -> Result<Json<Uuid>, AppError> {
    Ok(Json(state.services.auth.start_native_login(data).await?))
}

/// Desktop half of native passkey 2FA: exchange the pickup token for a session cookie.
#[utoipa::path(
    post,
    path = "/auth/login/native",
    request_body = FinishNativeLogin,
    responses(
        (status = 200, description = "Session created", body = LoginResult),
        (status = 404, description = "Pickup not ready, used, or expired"),
    )
)]
pub(crate) async fn finish_native_login(
    State(state): State<AppState>,
    RemoteAddress(remote_address): RemoteAddress,
    user_agent: Option<TypedHeader<UserAgent>>,
    Json(data): Json<FinishNativeLogin>,
) -> Result<Response, AppError> {
    let user_agent = user_agent.map(|TypedHeader(agent)| agent.as_str().to_owned());

    let result = state
        .services
        .auth
        .finish_native_login(data.token, Some(remote_address), user_agent)
        .await?;

    login_response(result, &state).await
}

async fn login_response(result: LoginResult, state: &AppState) -> Result<Response, AppError> {
    let config = &state.http_config;

    let cookie = match result {
        LoginResult::Session { session_id } => {
            let session = state.services.auth.get_session_by_id(session_id).await?;

            let mut cookie =
                Cookie::build((config.cookie_name.to_string(), session.id.to_string()))
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
                    cookie.set_expires(
                        OffsetDateTime::from_unix_timestamp(expires_at.timestamp()).ok(),
                    );
                }
                None => cookie.make_permanent(),
            }

            cookie
        }
        LoginResult::TwoFactorRequired { .. } => {
            return Ok(Json(result).into_response());
        }
    };

    let mut response = Json(result).into_response();

    match HeaderValue::from_str(&cookie.to_string()) {
        Ok(value) => {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
        Err(error) => tracing::error!(%error, "unable to create session cookie header"),
    }

    Ok(response)
}
