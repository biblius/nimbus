use super::session::SessionView;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validify::{ValidationError, Validify};

/// Successful password check: either a real session, or 2FA still required.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum LoginResult {
    /// A successful session has been established.
    Session { session: SessionView },

    /// Contains a 1-shot token that must be provided when finishing the login with 2FA.
    TwoFactorRequired { token: String },
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
#[derive(Debug, Serialize, ToSchema)]
pub struct GeneratedOtp {
    /// SVG QR code that contains the secret.
    pub qr: String,

    /// Token to present when confirming the OTP.
    pub token: Uuid,
}

/// OTP verification input. Used both for initial setup and later verification.
#[derive(Debug, Deserialize, Validify, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Otp {
    /// 1-shot token provided on initial login with credentials.
    pub token: Uuid,

    /// The OTP 6 digit code.
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
