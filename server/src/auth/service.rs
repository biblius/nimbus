use super::{
    crypto::{decode_otp_secret, hash_password, verify_otp, verify_password},
    model::{
        DisableOtp, FinishPasskeyLogin, FinishPasskeySetup, GeneratedOtp, LoginCredentials,
        LoginResult, Otp, PasskeyLoginChallenge, PasskeySetupChallenge, PasswordChange,
        PasswordReset, PasswordResetRequest, StartPasskeyLogin, UserPasskey, UserPasskeyView,
    },
    session::{ActiveSessionView, Session},
    token::{OneShotToken, TokenStore},
};
use crate::{
    db::{Repository, sessions::NewSession},
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
use webauthn_rs::prelude::{
    DiscoverableAuthentication, DiscoverableKey, PasskeyRegistration, Webauthn,
};

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

/// In-progress discoverable passkey ceremony, plus session prefs for when it finishes.
#[derive(Serialize, Deserialize)]
struct PasskeyLoginCeremony {
    auth: DiscoverableAuthentication,
    remember: bool,
    permanent: bool,
}

#[derive(Clone)]
pub struct AuthenticationService {
    repo: Repository,
    tokens: TokenStore,
    email: EmailSender,
    webauthn: Webauthn,
    frontend_url: String,
}

impl AuthenticationService {
    pub fn new(
        repo: Repository,
        tokens: TokenStore,
        email: EmailSender,
        webauthn: Webauthn,
        frontend_url: String,
    ) -> Self {
        Self {
            repo,
            tokens,
            email,
            webauthn,
            frontend_url: frontend_url.trim_end_matches('/').to_string(),
        }
    }

    /// Authenticate with username and password.
    ///
    /// No session is created until login actually succeeds.
    /// Users with 2FA enabled get [`LoginResult::TwoFactorRequired`].
    #[instrument(skip(self, credentials), fields(username = %credentials.username), err(Debug))]
    pub async fn login(
        &self,
        mut credentials: LoginCredentials,
        remote_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<LoginResult, AppError> {
        credentials.validify()?;

        let user = self
            .repo
            .users
            .get_by_username(&credentials.username)
            .await?
            .ok_or(AppError::Auth(AuthFailure::InvalidCredentials))?;

        self.validate_password(&user, &credentials.password)?;

        if user
            .otp_secret
            .as_deref()
            .is_some_and(|secret| !secret.is_empty())
        {
            let token = Uuid::new_v4();

            self.tokens
                .store(
                    OneShotToken::PendingLogin,
                    token,
                    &PendingLogin {
                        user_id: user.id,
                        remember: credentials.remember,
                        permanent: credentials.permanent,
                    },
                )
                .await?;

            return Ok(LoginResult::TwoFactorRequired {
                token: token.to_string(),
            });
        }

        let session = self
            .create_session(
                user,
                remote_address,
                user_agent,
                credentials.remember,
                credentials.permanent,
            )
            .await?;

        Ok(LoginResult::Session {
            session_id: session.id,
        })
    }

    /// Expire the session under the given ID.
    #[instrument(skip_all, err(Debug))]
    pub async fn logout(&self, session_id: Uuid) -> Result<(), AppError> {
        self.repo.sessions.expire(session_id).await
    }

    /// Finish a TOTP login using the pending-login token from [`Self::login`].
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
            .consume(OneShotToken::PendingLogin, otp.token)
            .await?;

        let user = self
            .repo
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
            session_id: session.id,
        })
    }

    /// Get a session by its ID.
    #[instrument(skip_all, err(Debug))]
    pub async fn get_session_by_id(&self, id: Uuid) -> Result<Session, AppError> {
        let record = self
            .repo
            .sessions
            .get(id)
            .await?
            .ok_or(AppError::Auth(AuthFailure::SessionNotFound))?;

        let user = self.repo.users.get_by_id(record.user_id).await?;

        Ok(Session::from_record(record, user))
    }

    /// Live sessions for the signed-in user.
    #[instrument(skip(self, session), fields(user_id = %session.user.id), err(Debug))]
    pub async fn list_sessions(
        &self,
        session: &Session,
    ) -> Result<Vec<ActiveSessionView>, AppError> {
        let rows = self
            .repo
            .sessions
            .list_live_for_user(session.user.id)
            .await?;
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
            .repo
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

        self.repo.sessions.expire(session_id).await
    }

    /// Expire every live session for the user except the current one.
    #[instrument(skip(self, session), fields(user_id = %session.user.id), err(Debug))]
    pub async fn expire_other_sessions(&self, session: &Session) -> Result<(), AppError> {
        self.repo
            .sessions
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
            .repo
            .users
            .get_by_id(session.user.id)
            .await
            .map_err(|_| AppError::Auth(AuthFailure::InvalidCredentials))?;

        self.validate_password(&user, &change.current_password)?;

        let password = hash_password(&change.new_password)?;

        self.repo.users.update_password(user.id, &password).await?;

        self.repo
            .sessions
            .expire_others(user.id, session.id)
            .await?;

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

        let user = match self.repo.users.get_by_email(&data.email).await {
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

        let user = self.repo.users.get_by_id(user_id).await?;

        let password = hash_password(&data.new_password)?;

        let mut tx = self.repo.start_tx().await?;

        self.repo
            .users
            .update_password_with(&mut *tx, user_id, &password)
            .await?;

        self.repo
            .users
            .clear_otp_secret_with(&mut *tx, user_id)
            .await?;

        self.repo
            .users
            .clear_passkeys_with(&mut *tx, user_id)
            .await?;

        self.repo
            .sessions
            .expire_all_with(&mut *tx, user_id)
            .await?;

        tx.commit().await?;

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

        let qr = thotp::qr::generate_code_svg(&uri, None, None, thotp::qr::EcLevel::L).map_err(
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

        self.repo
            .users
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

        let user = self.repo.users.get_by_id(session.user.id).await?;

        let encoded = user.otp_secret.as_deref().ok_or_else(|| {
            AppError::Unprocessable("Two-factor authentication is not enabled".into())
        })?;

        verify_otp(&decode_otp_secret(encoded)?, &data.otp_code)?;

        self.repo.users.clear_otp_secret(session.user.id).await
    }

    /// List the signed-in user's registered passkeys.
    #[instrument(skip(self, session), fields(user_id = %session.user.id), err(Debug))]
    pub async fn list_passkeys(&self, session: &Session) -> Result<Vec<UserPasskeyView>, AppError> {
        let passkeys = self.repo.users.list_passkeys(session.user.id).await?;
        Ok(passkeys.into_iter().map(UserPasskeyView::from).collect())
    }

    /// Remove one of the signed-in user's registered passkeys.
    #[instrument(skip(self, session), fields(user_id = %session.user.id, passkey_id = %id), err(Debug))]
    pub async fn delete_passkey(&self, session: &Session, id: i64) -> Result<(), AppError> {
        self.repo.users.delete_passkey(session.user.id, id).await
    }

    /// Start WebAuthn passkey registration and cache the in-progress state.
    #[instrument(skip(self, session), fields(user_id = %session.user.id), err(Debug))]
    pub async fn start_passkey_setup(
        &self,
        session: Session,
    ) -> Result<PasskeySetupChallenge, AppError> {
        let existing = self.repo.users.list_passkeys(session.user.id).await?;

        let exclude = existing
            .iter()
            .map(|passkey| passkey.data.cred_id().clone())
            .collect::<Vec<_>>();

        let exclude = (!exclude.is_empty()).then_some(exclude);

        let (challenge, regdata) = self.webauthn.start_passkey_registration(
            session.user.id,
            session
                .user
                .email
                .as_ref()
                .unwrap_or(&session.user.username),
            &session.user.username,
            exclude,
        )?;

        let token = Uuid::new_v4();

        self.tokens
            .store(OneShotToken::Passkey, token, &regdata)
            .await?;

        Ok(PasskeySetupChallenge { token, challenge })
    }

    /// Finish WebAuthn passkey registration and persist the credential.
    #[instrument(skip(self, session, data), fields(user_id = %session.user.id), err(Debug))]
    pub async fn finish_passkey_setup(
        &self,
        session: Session,
        data: FinishPasskeySetup,
    ) -> Result<UserPasskey, AppError> {
        let regdata: PasskeyRegistration = self
            .tokens
            .consume(OneShotToken::Passkey, data.token)
            .await?;

        let passkey = self
            .webauthn
            .finish_passkey_registration(&data.credential, &regdata)?;

        if self
            .repo
            .users
            .passkey_cred_id_taken(passkey.cred_id())
            .await?
        {
            return Err(AppError::ResourceConflict(
                "This passkey is already registered".into(),
            ));
        }

        self.repo
            .users
            .insert_passkey(session.user.id, passkey)
            .await
    }

    /// Start a discoverable (usernameless) WebAuthn passkey login.
    ///
    /// When [`StartPasskeyLogin::token`] is set, restarts the ceremony for that
    /// token while keeping the session prefs from the first start (native bridge).
    #[instrument(skip(self, data), err(Debug))]
    pub async fn start_passkey_login(
        &self,
        data: StartPasskeyLogin,
    ) -> Result<PasskeyLoginChallenge, AppError> {
        let (token, remember, permanent) = if let Some(token) = data.token {
            // Bridge restart: prove the ceremony exists, keep the original prefs.
            // Do NOT type-infer this get as Uuid - the payload is PasskeyLoginCeremony.
            let existing: PasskeyLoginCeremony =
                self.tokens.get(OneShotToken::Passkey, token).await?;

            (token, existing.remember, existing.permanent)
        } else {
            (Uuid::new_v4(), data.remember, data.permanent)
        };

        let (challenge, auth) = self.webauthn.start_discoverable_authentication()?;

        self.tokens
            .store(
                OneShotToken::Passkey,
                token,
                &PasskeyLoginCeremony {
                    auth,
                    remember,
                    permanent,
                },
            )
            .await?;

        Ok(PasskeyLoginChallenge {
            token,
            challenge,
            frontend_url: self.frontend_url.clone(),
        })
    }

    /// Finish discoverable passkey authentication and create a session.
    #[instrument(skip(self, data), err(Debug))]
    pub async fn finish_passkey_login(
        &self,
        data: FinishPasskeyLogin,
        remote_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<LoginResult, AppError> {
        let pending = self.verify_passkey_login(data).await?;

        let user = self.repo.users.get_by_id(pending.user_id).await?;

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
            session_id: session.id,
        })
    }

    /// Verify the WebAuthn assertion in the browser without creating a session.
    ///
    /// Stores the verified login under [`OneShotToken::NativeSession`] using the
    /// same token id so the native app can poll [`Self::finish_native_login`].
    #[instrument(skip(self, data), err(Debug))]
    pub async fn start_native_passkey_login(
        &self,
        data: FinishPasskeyLogin,
    ) -> Result<Uuid, AppError> {
        let token = data.token;
        let pending = self.verify_passkey_login(data).await?;

        self.tokens
            .store(OneShotToken::NativeSession, token, &pending)
            .await?;

        Ok(token)
    }

    /// Exchange the token from [`Self::start_native_login`] for a real session.
    #[instrument(skip(self), err(Debug))]
    pub async fn finish_native_passkey_login(
        &self,
        token: Uuid,
        remote_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<LoginResult, AppError> {
        let pending: PendingLogin = self
            .tokens
            .consume(OneShotToken::NativeSession, token)
            .await?;

        let user = self.repo.users.get_by_id(pending.user_id).await?;

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
            session_id: session.id,
        })
    }

    /// Verify a discoverable assertion, update credential counters, and consume
    /// the ceremony token. Does not create a session.
    async fn verify_passkey_login(
        &self,
        data: FinishPasskeyLogin,
    ) -> Result<PendingLogin, AppError> {
        let ceremony: PasskeyLoginCeremony = self
            .tokens
            .consume(OneShotToken::Passkey, data.token)
            .await?;

        let (user_id, _cred_id) = self
            .webauthn
            .identify_discoverable_authentication(&data.credential)?;

        let passkeys = self.repo.users.list_passkeys(user_id).await?;

        if passkeys.is_empty() {
            return Err(AppError::Auth(AuthFailure::InvalidCredentials));
        }

        let disc_keys = passkeys
            .iter()
            .map(|row| DiscoverableKey::from(&row.data.0))
            .collect::<Vec<_>>();

        let auth_result = self.webauthn.finish_discoverable_authentication(
            &data.credential,
            ceremony.auth,
            &disc_keys,
        )?;

        for mut passkey in passkeys {
            if !passkey
                .data
                .0
                .update_credential(&auth_result)
                .unwrap_or(false)
            {
                continue;
            }

            tracing::debug!(
                user = %user_id,
                cred_id = ?passkey.data.0.cred_id(),
                "updating passkey"
            );

            self.repo
                .users
                .apply_passkey_authentication(passkey.id, passkey.data.0)
                .await?;
        }

        // Ensure the user still exists before handing back a pending login.
        let _user = self.repo.users.get_by_id(user_id).await?;

        Ok(PendingLogin {
            user_id,
            remember: ceremony.remember,
            permanent: ceremony.permanent,
        })
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
        let now = Utc::now();

        let expires_at = if permanent {
            None
        } else if remember {
            Some(now + Duration::days(REMEMBER_SESSION_DAYS))
        } else {
            Some(now + Duration::hours(SESSION_HOURS))
        };

        let record = self
            .repo
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

        Ok(Session::from_record(record, user))
    }

    #[instrument(skip_all, fields(user_id = %user.id), err(Debug))]
    fn validate_password(&self, user: &User, password: &str) -> Result<(), AppError> {
        if !verify_password(password, &user.password) {
            return Err(AppError::Auth(AuthFailure::InvalidCredentials));
        }

        Ok(())
    }
}
