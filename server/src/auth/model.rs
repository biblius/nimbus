use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;
use validify::{ValidationError, Validify};
use webauthn_rs::prelude::{
    COSEAlgorithm, CreationChallengeResponse, Passkey, PublicKeyCredential,
    RegisterPublicKeyCredential, RequestChallengeResponse,
};

/// Successful password check: either a real session, or OTP still required.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LoginResult {
    /// A successful session has been established.
    Session { session_id: Uuid },

    /// TOTP verification required after a successful password check.
    TwoFactorRequired {
        /// One-shot token to present when finishing OTP login.
        token: String,
    },
}

#[derive(Debug, Clone, Deserialize, Validify)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
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
#[derive(Debug, Clone, Deserialize, Validify)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
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
#[derive(Debug, Deserialize, Validify)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DisableOtp {
    /// The OTP 6 digit code from the currently registered authenticator.
    #[modify(trim, lowercase)]
    #[validate(length(equal = 6, message = "Invalid code"))]
    pub otp_code: String,
}

#[derive(Debug, Deserialize, Validify)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct PasswordResetRequest {
    #[modify(lowercase, trim)]
    #[validate(email(message = "Not a valid email"))]
    pub email: String,
}

#[derive(Debug, Deserialize, Validify)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct PasswordReset {
    #[validate(length(min = 1, message = "Password cannot be empty"))]
    #[validate(custom(function = strong_password))]
    pub new_password: String,
}

#[derive(Debug, Deserialize, Validify)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
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

/// Client-facing passkey row (no public-key / attestation material).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct UserPasskeyView {
    pub id: i64,
    pub created_at: DateTime<Utc>,
    pub algorithm: String,
}

impl From<UserPasskey> for UserPasskeyView {
    fn from(value: UserPasskey) -> Self {
        let Json(passkey) = value.data;

        let algorithm = match passkey.cred_algorithm() {
            COSEAlgorithm::ES256 => "ES256",
            COSEAlgorithm::ES384 => "ES384",
            COSEAlgorithm::ES512 => "ES512",
            COSEAlgorithm::RS256 => "RS256",
            COSEAlgorithm::RS384 => "RS384",
            COSEAlgorithm::RS512 => "RS512",
            COSEAlgorithm::PS256 => "PS256",
            COSEAlgorithm::PS384 => "PS384",
            COSEAlgorithm::PS512 => "PS512",
            COSEAlgorithm::EDDSA => "EdDSA",
            COSEAlgorithm::INSECURE_RS1 => "",
            COSEAlgorithm::PinUvProtocol => "",
        }
        .to_owned();

        Self {
            id: value.id,
            created_at: value.created_at,
            algorithm,
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

/// Start a discoverable (usernameless) passkey login ceremony.
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct StartPasskeyLogin {
    /// When set, restart the ceremony for an existing token (native browser bridge).
    #[serde(default)]
    pub token: Option<Uuid>,

    /// Extend the session to 30 days (web "remember me"). Ignored when `permanent` is set.
    #[serde(default)]
    pub remember: bool,

    /// Never expire until logout. For native clients that store a Bearer token.
    #[serde(default)]
    pub permanent: bool,
}

/// Challenge returned when starting discoverable passkey authentication.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PasskeyLoginChallenge {
    /// One-shot token; present it again when finishing.
    pub token: Uuid,

    /// WebAuthn `PublicKeyCredentialRequestOptions` for the browser.
    pub challenge: RequestChallengeResponse,

    /// Web app origin for native passkey browser handoff.
    pub frontend_url: String,
}

/// Finish discoverable passkey login: ceremony token plus the authenticator assertion.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinishPasskeyLogin {
    pub token: Uuid,
    pub credential: PublicKeyCredential,
}

/// Exchange a native passkey pickup token for a real session.
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct FinishNativeLogin {
    pub token: Uuid,
}
