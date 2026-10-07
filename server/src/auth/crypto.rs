use thotp::encoding::data_encoding::BASE32;

use crate::error::{AppError, AuthFailure};

pub fn verify_otp(secret: &[u8], code: &str) -> Result<(), AppError> {
    let (result, drift) = match thotp::verify_totp(code, secret, 0) {
        Ok((result, drift)) => (result, drift),
        Err(error) => {
            tracing::error!(%error, "error verifying TOTP");
            return Err(AppError::Auth(AuthFailure::InvalidCredentials));
        }
    };

    #[cfg(debug_assertions)]
    tracing::debug!(%result, %drift, "validated OTP");

    if !result {
        return Err(AppError::Auth(AuthFailure::InvalidCredentials));
    }

    Ok(())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    use argon2::{
        Argon2,
        password_hash::{PasswordHash, PasswordVerifier},
    };

    let Ok(parsed) = PasswordHash::new(hash) else {
        tracing::error!("stored password hash is not a valid argon2 hash");
        return false;
    };

    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub fn hash_password(password: &str) -> Result<String, AppError> {
    use argon2::{
        Argon2,
        password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
    };

    let salt = SaltString::generate(&mut OsRng);

    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| {
            tracing::error!(%error, "hashing password");
            AppError::Internal
        })
}

pub fn decode_otp_secret(encoded: &str) -> Result<Vec<u8>, AppError> {
    thotp::encoding::decode(encoded, BASE32)
        .map_err(|_| AppError::Auth(AuthFailure::InvalidCredentials))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argon2_roundtrip() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &hash));
        assert!(!verify_password("nope", &hash));
    }
}
