use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use utoipa::ToSchema;
use uuid::Uuid;
use validify::{ValidationError, Validify};
use webauthn_rs::prelude::{
    CreationChallengeResponse, Passkey, PublicKeyCredential, RegisterPublicKeyCredential,
    RequestChallengeResponse,
};

/// Successful password check: either a real session, or 2FA still required.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "status", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum LoginResult {
    /// A successful session has been established.
    Session { session_id: Uuid },

    /// Additional authentication step required.
    TwoFactorRequired {
        /// One-shot token to present when finishing 2FA login.
        token: String,

        /// Represents which 2FA methods are available.
        available: Vec<TFAType>,

        /// Web app origin for native passkey browser handoff.
        frontend_url: String,
    },
}

/// Sent as part of [LoginResult] if the user has 2FA enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum TFAType {
    Otp,
    Passkey,
}

#[derive(Debug, Deserialize, Validify, ToSchema)]
pub struct LoginCredentials {
    #[modify(trim, lowercase)]
    #[validate(length(min = 1, message = "Username cannot be empty"))]
    pub username: String,

    #[validate(length(min = 1, message = "Password cannot be empty"))]
    pub password: String,

    /// Extend the session to 30 days (web "remember me"). Ignored when `permanent` is set.
    #[serde(default)]
    pub remember: bool,

    /// Never expire until logout. For native (mobile/desktop) clients that store a Bearer token.
    #[serde(default)]
    pub permanent: bool,
}

/// Generated TOTP data used to set up 2FA.
/// The HTTP handler returns the QR as an SVG body and the token in the `X-Otp-Token` header.
#[derive(Debug)]
pub struct GeneratedOtp {
    /// SVG QR code that contains the secret.
    pub qr: String,

    /// Token to present when confirming the OTP.
    pub token: Uuid,
}

/// Header carrying the one-shot token from `POST /auth/otp/setup`.
pub const OTP_TOKEN_HEADER: &str = "x-otp-token";

/// Finish a TOTP login using the pending-login token from [`LoginResult::TwoFactorRequired`].
#[derive(Debug, Deserialize, Validify, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Otp {
    /// One-shot token from the password login step.
    pub token: Uuid,

    /// The OTP 6 digit code.
    #[modify(trim, lowercase)]
    #[validate(length(equal = 6, message = "Invalid code"))]
    pub otp_code: String,
}

/// Disable 2FA by proving possession of the current authenticator.
#[derive(Debug, Deserialize, Validify, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DisableOtp {
    /// The OTP 6 digit code from the currently registered authenticator.
    #[modify(trim, lowercase)]
    #[validate(length(equal = 6, message = "Invalid code"))]
    pub otp_code: String,
}

#[derive(Debug, Deserialize, Validify, ToSchema)]
pub struct PasswordResetRequest {
    #[modify(lowercase, trim)]
    #[validate(email(message = "Not a valid email"))]
    pub email: String,
}

#[derive(Debug, Deserialize, Validify, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PasswordReset {
    #[validate(length(min = 1, message = "Password cannot be empty"))]
    #[validate(custom(function = strong_password))]
    pub new_password: String,
}

#[derive(Debug, Deserialize, Validify, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PasswordChange {
    #[validate(length(min = 1, message = "Password cannot be empty"))]
    pub current_password: String,
    #[validate(length(min = 1, message = "Password cannot be empty"))]
    #[validate(custom(function = strong_password))]
    pub new_password: String,
}

fn strong_password(password: &str) -> Result<(), validify::ValidationError> {
    let score = zxcvbn::zxcvbn(password, &[]).score();
    if score < zxcvbn::Score::Three {
        return Err(
            ValidationError::new_field("weak_password").with_message(format!(
                "The password is too weak (score {}/{})",
                score,
                zxcvbn::Score::Three
            )),
        );
    }
    Ok(())
}

/// Row in `user_passkeys`.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct UserPasskey {
    pub id: i64,
    pub user_id: Uuid,
    pub data: Json<Passkey>,
    pub created_at: DateTime<Utc>,
}

/// Client-facing passkey row (no credential material).
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserPasskeyView {
    pub id: i64,
    pub created_at: DateTime<Utc>,
}

impl From<UserPasskey> for UserPasskeyView {
    fn from(value: UserPasskey) -> Self {
        Self {
            id: value.id,
            created_at: value.created_at,
        }
    }
}

/// Challenge returned when starting passkey registration, plus the one-shot token
/// needed to finish it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PasskeySetupChallenge {
    /// One-shot token to present when finishing registration.
    pub token: Uuid,

    /// WebAuthn `PublicKeyCredentialCreationOptions` for the browser.
    pub challenge: CreationChallengeResponse,
}

/// Finish passkey registration: one-shot token plus the authenticator response.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinishPasskeySetup {
    pub token: Uuid,
    pub credential: RegisterPublicKeyCredential,
}

/// Start passkey login using the pending-login token from [`LoginResult::TwoFactorRequired`].
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StartPasskeyLogin {
    pub token: Uuid,
}

/// Challenge returned when starting passkey authentication.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PasskeyLoginChallenge {
    /// Same pending-login token; present it again when finishing.
    pub token: Uuid,

    /// WebAuthn `PublicKeyCredentialRequestOptions` for the browser.
    pub challenge: RequestChallengeResponse,
}

/// Finish passkey login: pending-login token plus the authenticator assertion.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinishPasskeyLogin {
    pub token: Uuid,
    pub credential: PublicKeyCredential,
}

/// Exchange a native passkey pickup token for a real session.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FinishNativeLogin {
    pub token: Uuid,
}
