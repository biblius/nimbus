#[suitest::suite(auth_login_with_otp)]
#[suitest::suite_cfg(sequential = true)]
mod auth_login_with_otp {
    use crate::{
        auth::{
            crypto::decode_otp_secret,
            model::{LoginCredentials, LoginResult, Otp},
            token::{OneShotToken, TokenStore},
        },
        error::{AppError, AuthFailure},
        test::TestState,
        user::model::{CreateUser, Role, UserView},
    };
    use chrono::{Duration, Utc};
    use serde::{Deserialize, Serialize};
    use std::time::{SystemTime, UNIX_EPOCH};
    use suitest::{before_all, before_each};
    use uuid::Uuid;

    /// Well-known TOTP secret used across these fixtures.
    const SECRET: &str = "JBSWY3DPEHPK3PXP";
    const SESSION_HOURS: i64 = 12;
    const REMEMBER_SESSION_DAYS: i64 = 30;

    #[derive(Serialize, Deserialize)]
    struct PendingLogin {
        user_id: Uuid,
        remember: bool,
        permanent: bool,
    }

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
    }

    /// User with OTP enabled and a fresh pending-login token from password login.
    #[before_each]
    async fn user_and_pending(state: TestState) -> (UserView, Uuid) {
        let username = format!("otp-login-{}", Uuid::new_v4());
        let user = state
            .services
            .users
            .create_user(CreateUser {
                username: username.clone(),
                password: "temporary".into(),
                email: None,
                role: Role::User,
            })
            .await
            .unwrap();

        state
            .repository
            .users
            .update_otp_secret(user.id, SECRET)
            .await
            .unwrap();

        let login = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username,
                    password: "temporary".into(),
                    remember: false,
                    permanent: false,
                },
                None,
                None,
            )
            .await
            .unwrap();

        let LoginResult::TwoFactorRequired { token } = login else {
            panic!("expected TwoFactorRequired");
        };
        (user, token.parse().unwrap())
    }

    fn totp_now() -> String {
        let secret = decode_otp_secret(SECRET).unwrap();
        let step = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            / 30;
        thotp::otp(&secret, step).unwrap()
    }

    async fn set_backoff(state: &TestState, username: &str, unlock_at: chrono::DateTime<Utc>) {
        let ttl = unlock_at
            .signed_duration_since(Utc::now())
            .num_seconds()
            .max(1) as u64;
        state
            .cache
            .set_ex(
                &format!("login:backoff:{username}"),
                &unlock_at.to_rfc3339(),
                ttl,
            )
            .await
            .unwrap();
    }

    /// Code length != 6 independently fails validation.
    #[test]
    async fn rejects_invalid_code_length(state: TestState, token: Uuid) {
        let err = state
            .services
            .auth
            .login_with_otp(
                Otp {
                    token: *token,
                    otp_code: "12345".into(),
                },
                None,
                None,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    /// Missing pending-login token yields ResourceMissing.
    #[test]
    async fn rejects_missing_pending_token(state: TestState) {
        let err = state
            .services
            .auth
            .login_with_otp(
                Otp {
                    token: Uuid::new_v4(),
                    otp_code: "123456".into(),
                },
                None,
                None,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::ResourceMissing(_)));
    }

    /// Pending user id with no row yields InvalidCredentials.
    #[test]
    async fn rejects_missing_user(state: TestState) {
        let token = Uuid::new_v4();
        TokenStore::new(state.cache.clone())
            .store(
                OneShotToken::PendingLogin,
                token,
                &PendingLogin {
                    user_id: Uuid::new_v4(),
                    remember: false,
                    permanent: false,
                },
            )
            .await
            .unwrap();

        let err = state
            .services
            .auth
            .login_with_otp(
                Otp {
                    token,
                    otp_code: totp_now(),
                },
                None,
                None,
            )
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            AppError::Auth(AuthFailure::InvalidCredentials)
        ));
    }

    /// Active backoff independently blocks a correct OTP.
    #[test]
    async fn active_backoff_blocks_correct_otp(state: TestState, user: UserView, token: Uuid) {
        set_backoff(state, &user.username, Utc::now() + Duration::minutes(10)).await;

        let err = state
            .services
            .auth
            .login_with_otp(
                Otp {
                    token: *token,
                    otp_code: totp_now(),
                },
                None,
                None,
            )
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            AppError::Auth(AuthFailure::LoginBackoff { .. })
        ));
    }

    /// Expired unlock_at independently allows a correct OTP.
    #[test]
    async fn expired_backoff_allows_otp(state: TestState, user: UserView, token: Uuid) {
        state
            .cache
            .set_ex(
                &format!("login:backoff:{}", user.username),
                &(Utc::now() - Duration::minutes(1)).to_rfc3339(),
                60,
            )
            .await
            .unwrap();

        let result = state
            .services
            .auth
            .login_with_otp(
                Otp {
                    token: *token,
                    otp_code: totp_now(),
                },
                None,
                None,
            )
            .await
            .unwrap();
        assert!(matches!(result, LoginResult::Session { .. }));
    }

    /// None otp_secret independently yields InvalidCredentials.
    #[test]
    async fn rejects_when_otp_secret_absent(state: TestState, user: UserView) {
        state
            .repository
            .users
            .clear_otp_secret(user.id)
            .await
            .unwrap();

        let token = Uuid::new_v4();
        TokenStore::new(state.cache.clone())
            .store(
                OneShotToken::PendingLogin,
                token,
                &PendingLogin {
                    user_id: user.id,
                    remember: false,
                    permanent: false,
                },
            )
            .await
            .unwrap();

        let err = state
            .services
            .auth
            .login_with_otp(
                Otp {
                    token,
                    otp_code: "123456".into(),
                },
                None,
                None,
            )
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            AppError::Auth(AuthFailure::InvalidCredentials)
        ));
    }

    /// Empty otp_secret independently yields InvalidCredentials.
    #[test]
    async fn rejects_when_otp_secret_empty(state: TestState, user: UserView) {
        state
            .repository
            .users
            .update_otp_secret(user.id, "")
            .await
            .unwrap();

        let token = Uuid::new_v4();
        TokenStore::new(state.cache.clone())
            .store(
                OneShotToken::PendingLogin,
                token,
                &PendingLogin {
                    user_id: user.id,
                    remember: false,
                    permanent: false,
                },
            )
            .await
            .unwrap();

        let err = state
            .services
            .auth
            .login_with_otp(
                Otp {
                    token,
                    otp_code: "123456".into(),
                },
                None,
                None,
            )
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            AppError::Auth(AuthFailure::InvalidCredentials)
        ));
    }

    /// Wrong code independently fails; pending token is kept for retries.
    #[test]
    async fn wrong_code_is_invalid_credentials(state: TestState, user: UserView, token: Uuid) {
        let err = state
            .services
            .auth
            .login_with_otp(
                Otp {
                    token: *token,
                    otp_code: "000000".into(),
                },
                None,
                None,
            )
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            AppError::Auth(AuthFailure::InvalidCredentials)
        ));
        assert_eq!(
            state
                .cache
                .get(&format!("login:attempts:{}", user.username))
                .await
                .unwrap()
                .and_then(|raw| raw.parse().ok()),
            Some(1)
        );
        assert!(
            TokenStore::new(state.cache.clone())
                .get::<PendingLogin>(OneShotToken::PendingLogin, *token)
                .await
                .is_ok()
        );
    }

    /// Correct code creates a session, clears attempts, and consumes the pending token.
    #[test]
    async fn correct_code_creates_session(state: TestState, user: UserView, token: Uuid) {
        state
            .services
            .auth
            .login_with_otp(
                Otp {
                    token: *token,
                    otp_code: "000000".into(),
                },
                None,
                None,
            )
            .await
            .unwrap_err();

        let LoginResult::Session { session_id } = state
            .services
            .auth
            .login_with_otp(
                Otp {
                    token: *token,
                    otp_code: totp_now(),
                },
                None,
                None,
            )
            .await
            .unwrap()
        else {
            panic!("expected Session");
        };

        state
            .services
            .auth
            .get_session_by_id(session_id)
            .await
            .unwrap();
        assert!(
            state
                .cache
                .get(&format!("login:attempts:{}", user.username))
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            TokenStore::new(state.cache.clone())
                .get::<PendingLogin>(OneShotToken::PendingLogin, *token)
                .await
                .is_err()
        );
    }

    /// Leading/trailing whitespace is trimmed before verification.
    #[test]
    async fn trims_otp_code(state: TestState, token: Uuid) {
        let code = totp_now();
        let result = state
            .services
            .auth
            .login_with_otp(
                Otp {
                    token: *token,
                    otp_code: format!("  {code}  "),
                },
                None,
                None,
            )
            .await
            .unwrap();
        assert!(matches!(result, LoginResult::Session { .. }));
    }

    /// Pending remember independently extends session expiry.
    #[test]
    async fn remember_session_expires_in_thirty_days(state: TestState, user: UserView) {
        let token = Uuid::new_v4();
        TokenStore::new(state.cache.clone())
            .store(
                OneShotToken::PendingLogin,
                token,
                &PendingLogin {
                    user_id: user.id,
                    remember: true,
                    permanent: false,
                },
            )
            .await
            .unwrap();

        let before = Utc::now();
        let LoginResult::Session { session_id } = state
            .services
            .auth
            .login_with_otp(
                Otp {
                    token,
                    otp_code: totp_now(),
                },
                None,
                None,
            )
            .await
            .unwrap()
        else {
            panic!("expected Session");
        };
        let after = Utc::now();
        let session = state
            .services
            .auth
            .get_session_by_id(session_id)
            .await
            .unwrap();
        let expires = session.expires_at.expect("remember session has expiry");
        assert!(expires >= before + Duration::days(REMEMBER_SESSION_DAYS) - Duration::seconds(5));
        assert!(expires <= after + Duration::days(REMEMBER_SESSION_DAYS) + Duration::seconds(5));
    }

    /// Pending permanent independently clears expiry.
    #[test]
    async fn permanent_session_has_no_expiry(state: TestState, user: UserView) {
        let token = Uuid::new_v4();
        TokenStore::new(state.cache.clone())
            .store(
                OneShotToken::PendingLogin,
                token,
                &PendingLogin {
                    user_id: user.id,
                    remember: false,
                    permanent: true,
                },
            )
            .await
            .unwrap();

        let LoginResult::Session { session_id } = state
            .services
            .auth
            .login_with_otp(
                Otp {
                    token,
                    otp_code: totp_now(),
                },
                None,
                None,
            )
            .await
            .unwrap()
        else {
            panic!("expected Session");
        };
        let session = state
            .services
            .auth
            .get_session_by_id(session_id)
            .await
            .unwrap();
        assert!(session.expires_at.is_none());
    }

    /// Default pending flags yield the short-lived session.
    #[test]
    async fn default_session_expires_in_twelve_hours(state: TestState, token: Uuid) {
        let before = Utc::now();
        let LoginResult::Session { session_id } = state
            .services
            .auth
            .login_with_otp(
                Otp {
                    token: *token,
                    otp_code: totp_now(),
                },
                None,
                None,
            )
            .await
            .unwrap()
        else {
            panic!("expected Session");
        };
        let after = Utc::now();
        let session = state
            .services
            .auth
            .get_session_by_id(session_id)
            .await
            .unwrap();
        let expires = session.expires_at.expect("default session has expiry");
        assert!(expires >= before + Duration::hours(SESSION_HOURS) - Duration::seconds(5));
        assert!(expires <= after + Duration::hours(SESSION_HOURS) + Duration::seconds(5));
    }

    /// Every 5th OTP failure sets backoff; password login shares the same lock.
    #[test]
    async fn five_failures_engage_backoff(state: TestState, user: UserView, token: Uuid) {
        let wrong = Otp {
            token: *token,
            otp_code: "000000".into(),
        };

        for _ in 0..5 {
            let err = state
                .services
                .auth
                .login_with_otp(wrong.clone(), None, None)
                .await
                .unwrap_err();
            assert!(matches!(
                err,
                AppError::Auth(AuthFailure::InvalidCredentials)
            ));
        }

        let backoff = state
            .services
            .auth
            .login_with_otp(wrong, None, None)
            .await
            .unwrap_err();
        assert!(matches!(
            backoff,
            AppError::Auth(AuthFailure::LoginBackoff { .. })
        ));

        let password_locked = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: user.username.clone(),
                    password: "temporary".into(),
                    remember: false,
                    permanent: false,
                },
                None,
                None,
            )
            .await
            .unwrap_err();
        assert!(matches!(
            password_locked,
            AppError::Auth(AuthFailure::LoginBackoff { .. })
        ));
    }

    /// Session path records remote address and user agent when provided.
    #[test]
    async fn session_stores_remote_address_and_user_agent(
        state: TestState,
        user: UserView,
        token: Uuid,
    ) {
        let LoginResult::Session { session_id } = state
            .services
            .auth
            .login_with_otp(
                Otp {
                    token: *token,
                    otp_code: totp_now(),
                },
                Some("198.51.100.10".into()),
                Some("nimbus-otp/1.0".into()),
            )
            .await
            .unwrap()
        else {
            panic!("expected Session");
        };

        let live = state
            .repository
            .sessions
            .list_live_for_user(user.id)
            .await
            .unwrap();
        let row = live
            .iter()
            .find(|s| s.id == session_id)
            .expect("session row");
        assert_eq!(row.remote_address.as_deref(), Some("198.51.100.10"));
        assert_eq!(row.user_agent.as_deref(), Some("nimbus-otp/1.0"));
    }
}
