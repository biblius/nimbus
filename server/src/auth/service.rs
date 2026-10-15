use super::{
    crypto::{decode_otp_secret, hash_password, verify_otp, verify_password},
    model::{
        DisableOtp, GeneratedOtp, LoginCredentials, LoginResult, Otp, PasswordChange,
        PasswordReset, PasswordResetRequest,
    },
    session::{ActiveSessionView, Session},
    token::{OneShotToken, TokenStore},
};
use crate::{
    db::{SessionRepository, UserRepository, sessions::NewSession},
    email::EmailSender,
    error::{AppError, AuthFailure},
    user::model::User,
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use thotp::encoding::data_encoding::BASE32;
use tracing::instrument;
use uuid::Uuid;
use validify::Validify;

/// Sessions without "remember" expire after this long.
const SESSION_HOURS: i64 = 12;

/// Web "remember me" sessions expire after this many days.
const REMEMBER_SESSION_DAYS: i64 = 30;

#[derive(Serialize, Deserialize)]
struct PendingLogin {
    user_id: Uuid,
    remember: bool,
    permanent: bool,
}

#[derive(Clone)]
pub struct AuthenticationService {
    users: UserRepository,
    sessions: SessionRepository,
    tokens: TokenStore,
    email: EmailSender,
}

impl AuthenticationService {
    pub fn new(
        users: UserRepository,
        sessions: SessionRepository,
        tokens: TokenStore,
        email: EmailSender,
    ) -> Self {
        Self {
            users,
            sessions,
            tokens,
            email,
        }
    }

    /// Authenticate with username and password.
    ///
    /// No session is created until login actually succeeds. Users with 2FA enabled
    /// get [`LoginResult::TwoFactorRequired`].
    #[instrument(skip(self, credentials), fields(username = %credentials.username), err(Debug))]
    pub async fn login(
        &self,
        mut credentials: LoginCredentials,
        remote_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<LoginResult, AppError> {
        credentials.validify()?;

        let user = self
            .users
            .get_by_username(&credentials.username)
            .await?
            .ok_or(AppError::Auth(AuthFailure::InvalidCredentials))?;

        self.validate_password(&user, &credentials.password)?;

        if user.otp_secret.is_none() {
            let session = self
                .create_session(
                    user,
                    remote_address,
                    user_agent,
                    credentials.remember,
                    credentials.permanent,
                )
                .await?;
            return Ok(LoginResult::Session {
                session: session.into(),
            });
        }

        let token = Uuid::new_v4();

        self.tokens
            .store(
                OneShotToken::OneTimePassword,
                token,
                &PendingLogin {
                    user_id: user.id,
                    remember: credentials.remember,
                    permanent: credentials.permanent,
                },
            )
            .await?;

        Ok(LoginResult::TwoFactorRequired {
            token: token.to_string(),
        })
    }

    /// Finish a 2FA login.
    #[instrument(skip(self, otp), err(Debug))]
    pub async fn login_with_otp(
        &self,
        mut otp: Otp,
        remote_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<LoginResult, AppError> {
        otp.validify()?;

        let pending: PendingLogin = self
            .tokens
            .consume(OneShotToken::OneTimePassword, otp.token)
            .await?;

        let user = self
            .users
            .get_by_id(pending.user_id)
            .await
            .map_err(|_| AppError::Auth(AuthFailure::InvalidCredentials))?;

        let encoded = user
            .otp_secret
            .as_deref()
            .filter(|secret| !secret.is_empty())
            .ok_or(AppError::Auth(AuthFailure::InvalidCredentials))?;

        verify_otp(&decode_otp_secret(encoded)?, &otp.otp_code)?;

        let session = self
            .create_session(
                user,
                remote_address,
                user_agent,
                pending.remember,
                pending.permanent,
            )
            .await?;

        Ok(LoginResult::Session {
            session: session.into(),
        })
    }

    /// Get a session by its ID.
    #[instrument(skip_all, err(Debug))]
    pub async fn get_session_by_id(&self, id: Uuid) -> Result<Session, AppError> {
        let record = self
            .sessions
            .get(id)
            .await?
            .ok_or(AppError::Auth(AuthFailure::SessionNotFound))?;

        let user = self.users.get_by_id(record.user_id).await?;

        Ok(Session::from_record(record, user, false))
    }

    /// Expire the session under the given ID.
    #[instrument(skip_all, err(Debug))]
    pub async fn logout(&self, session_id: Uuid) -> Result<(), AppError> {
        self.sessions.expire(session_id).await
    }

    /// Live sessions for the signed-in user.
    #[instrument(skip(self, session), fields(user_id = %session.user.id), err(Debug))]
    pub async fn list_sessions(
        &self,
        session: &Session,
    ) -> Result<Vec<ActiveSessionView>, AppError> {
        let rows = self.sessions.list_live_for_user(session.user.id).await?;
        Ok(rows
            .into_iter()
            .map(|row| ActiveSessionView::from_record(row, session.id))
            .collect())
    }

    /// Expire one of the caller's sessions. Returns not found if missing or not owned.
    #[instrument(
        skip(self, session),
        fields(user_id = %session.user.id, target = %session_id),
        err(Debug)
    )]
    pub async fn expire_session(
        &self,
        session: &Session,
        session_id: Uuid,
    ) -> Result<(), AppError> {
        let row = self
            .sessions
            .get(session_id)
            .await?
            .ok_or_else(|| AppError::ResourceMissing("Session does not exist".into()))?;

        if row.user_id != session.user.id {
            return Err(AppError::ResourceMissing("Session does not exist".into()));
        }

        if row
            .expires_at
            .is_some_and(|expires_at| expires_at < Utc::now())
        {
            return Err(AppError::ResourceMissing("Session does not exist".into()));
        }

        self.sessions.expire(session_id).await
    }

    /// Expire every live session for the user except the current one.
    #[instrument(skip(self, session), fields(user_id = %session.user.id), err(Debug))]
    pub async fn expire_other_sessions(&self, session: &Session) -> Result<(), AppError> {
        self.sessions
            .expire_others(session.user.id, session.id)
            .await
    }

    /// Update the user's password.
    #[instrument(
        skip(self, session, change),
        fields(user_id = %session.user.id),
        err(Debug)
    )]
    pub async fn change_password(
        &self,
        session: Session,
        mut change: PasswordChange,
    ) -> Result<(), AppError> {
        change.validify()?;

        let user = self
            .users
            .get_by_id(session.user.id)
            .await
            .map_err(|_| AppError::Auth(AuthFailure::InvalidCredentials))?;

        self.validate_password(&user, &change.current_password)?;

        let password = hash_password(&change.new_password)?;

        self.users.update_password(user.id, &password).await?;

        self.sessions.expire_others(user.id, session.id).await?;

        if let Some(email) = user.email.as_deref()
            && let Err(error) = self
                .email
                .send_password_changed(email, &user.username)
                .await
        {
            tracing::error!(%error, "password-changed email was not sent");
        }
        Ok(())
    }

    /// Send a password reset token to the user found by the email.
    /// If no user is found, returns OK to prevent exposing emails.
    #[instrument(skip(self), err(Debug))]
    pub async fn start_password_reset(
        &self,
        mut data: PasswordResetRequest,
    ) -> Result<(), AppError> {
        data.validify()?;

        let user = match self.users.get_by_email(&data.email).await {
            Ok(Some(user)) => user,
            Ok(None) => return Ok(()),
            Err(error) => {
                tracing::error!(%error, "error while fetching user for password reset");
                return Ok(());
            }
        };

        let token = Uuid::new_v4();

        self.tokens
            .store(OneShotToken::PasswordReset, token, &user.id)
            .await?;

        self.email
            .send_password_reset(&data.email, &user.username, token)
            .await?;

        Ok(())
    }

    /// Consume the token sent by [Self::start_password_reset] and reset the user's password to
    /// the given one.
    #[instrument(skip(self, token, data), err(Debug))]
    pub async fn finish_password_reset(
        &self,
        token: Uuid,
        mut data: PasswordReset,
    ) -> Result<(), AppError> {
        data.validify()?;

        let user_id: Uuid = self
            .tokens
            .consume(OneShotToken::PasswordReset, token)
            .await?;

        let user = self.users.get_by_id(user_id).await?;

        let password = hash_password(&data.new_password)?;

        self.users.update_password(user_id, &password).await?;

        self.sessions.expire_all(user_id).await?;

        if let Some(email) = user.email.as_deref()
            && let Err(error) = self
                .email
                .send_password_changed(email, &user.username)
                .await
        {
            tracing::error!(%error, "password-changed email was not sent");
        }
        Ok(())
    }

    /// Cache a TOTP secret and return the QR code that registers it.
    #[instrument(skip(self, session), fields(user_id = %session.user.id), err(Debug))]
    pub async fn start_otp_setup(&self, session: Session) -> Result<GeneratedOtp, AppError> {
        let secret = thotp::generate_secret(160);

        let encoded = thotp::encoding::encode(&secret, BASE32);

        let label = format!("nimbus:{}", session.user.username);

        let uri =
            thotp::qr::otp_uri("totp", &encoded, &label, "Nimbus", None).map_err(|error| {
                tracing::error!(%error, "error generating TOTP uri");
                AppError::Internal
            })?;

        let token = Uuid::new_v4();

        self.tokens
            .store(OneShotToken::OneTimePassword, token, &encoded)
            .await?;

        let qr = thotp::qr::generate_code_svg(&uri, None, None, thotp::qr::EcLevel::M).map_err(
            |error| {
                tracing::error!(%error, "error generating TOTP QR code");
                AppError::Internal
            },
        )?;

        Ok(GeneratedOtp { qr, token })
    }

    /// Consume the OTP token created by [Self::start_otp_setup] and update the user's OTP secret.
    #[instrument(skip(self, session, otp), fields(user_id = %session.user.id), err(Debug))]
    pub async fn finish_otp_setup(&self, session: Session, mut otp: Otp) -> Result<(), AppError> {
        otp.validify()?;

        let encoded: String = self
            .tokens
            .consume(OneShotToken::OneTimePassword, otp.token)
            .await?;

        let secret = decode_otp_secret(&encoded).map_err(|error| {
            tracing::error!(%error, "error decoding OTP secret");
            AppError::Internal
        })?;

        verify_otp(&secret, &otp.otp_code)?;

        self.users
            .update_otp_secret(session.user.id, &encoded)
            .await
    }

    /// Verify the current authenticator code and clear the user's OTP secret.
    #[instrument(skip(self, session, data), fields(user_id = %session.user.id), err(Debug))]
    pub async fn disable_otp(
        &self,
        session: Session,
        mut data: DisableOtp,
    ) -> Result<(), AppError> {
        data.validify()?;

        let user = self.users.get_by_id(session.user.id).await?;

        let encoded = user.otp_secret.as_deref().ok_or_else(|| {
            AppError::Unprocessable("Two-factor authentication is not enabled".into())
        })?;

        verify_otp(&decode_otp_secret(encoded)?, &data.otp_code)?;

        self.users.clear_otp_secret(session.user.id).await
    }

    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    async fn create_session(
        &self,
        user: User,
        remote_address: Option<String>,
        user_agent: Option<String>,
        remember: bool,
        permanent: bool,
    ) -> Result<Session, AppError> {
        let first_login = self.sessions.count_for_user(user.id).await? == 0;

        let now = Utc::now();

        let expires_at = if permanent {
            None
        } else if remember {
            Some(now + Duration::days(REMEMBER_SESSION_DAYS))
        } else {
            Some(now + Duration::hours(SESSION_HOURS))
        };

        let record = self
            .sessions
            .insert(NewSession {
                id: Uuid::new_v4(),
                user_id: user.id,
                remote_address,
                user_agent,
                now,
                expires_at,
            })
            .await?;

        Ok(Session::from_record(record, user, first_login))
    }

    #[instrument(skip_all, fields(user_id = %user.id), err(Debug))]
    fn validate_password(&self, user: &User, password: &str) -> Result<(), AppError> {
        if !verify_password(password, &user.password) {
            return Err(AppError::Auth(AuthFailure::InvalidCredentials));
        }

        Ok(())
    }
}
