#[suitest::suite(auth_otp_setup)]
#[suitest::suite_cfg(sequential = true)]
mod auth_otp_setup {
    use crate::{
        auth::{
            crypto::decode_otp_secret,
            model::{LoginCredentials, LoginResult, Otp},
            session::Session,
            token::{OneShotToken, TokenStore},
        },
        error::{AppError, AuthFailure},
        test::TestState,
        user::model::{CreateUser, Role, UserView},
    };
    use std::time::{SystemTime, UNIX_EPOCH};
    use suitest::{before_all, before_each};
    use uuid::Uuid;

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
    }

    #[before_each]
    async fn user_and_session(state: TestState) -> (UserView, Session) {
        let username = format!("otp-{}", Uuid::new_v4());
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
        let session = crate::test::login(state, &username, "temporary").await;
        (user, session)
    }

    fn totp_now(secret: &[u8]) -> String {
        let step = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            / 30;
        thotp::otp(secret, step).unwrap()
    }

    async fn pending_secret(state: &TestState, token: Uuid) -> Vec<u8> {
        let encoded: String = TokenStore::new(state.cache.clone())
            .get(OneShotToken::OneTimePassword, token)
            .await
            .unwrap();
        decode_otp_secret(&encoded).unwrap()
    }

    /// Start returns a QR SVG and stores a one-shot setup token (no secret on the user yet).
    #[test]
    async fn start_returns_qr_and_stores_token(state: TestState, user: UserView, session: Session) {
        let generated = state
            .services
            .auth
            .start_otp_setup(session.clone())
            .await
            .unwrap();

        assert!(generated.qr.contains("<svg"));
        assert!(
            TokenStore::new(state.cache.clone())
                .get::<String>(OneShotToken::OneTimePassword, generated.token)
                .await
                .is_ok()
        );
        assert!(
            state
                .repository
                .users
                .get_by_id(user.id)
                .await
                .unwrap()
                .otp_secret
                .is_none()
        );
    }

    /// Code length != 6 independently fails validation.
    #[test]
    async fn finish_rejects_invalid_code_length(state: TestState, session: Session) {
        let generated = state
            .services
            .auth
            .start_otp_setup(session.clone())
            .await
            .unwrap();

        let err = state
            .services
            .auth
            .finish_otp_setup(
                session.clone(),
                Otp {
                    token: generated.token,
                    otp_code: "12345".into(),
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    /// Missing setup token yields ResourceMissing.
    #[test]
    async fn finish_rejects_missing_token(state: TestState, session: Session) {
        let err = state
            .services
            .auth
            .finish_otp_setup(
                session.clone(),
                Otp {
                    token: Uuid::new_v4(),
                    otp_code: "123456".into(),
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::ResourceMissing(_)));
    }

    /// Wrong code independently yields InvalidCredentials; secret is not saved.
    #[test]
    async fn finish_rejects_wrong_code(state: TestState, user: UserView, session: Session) {
        let generated = state
            .services
            .auth
            .start_otp_setup(session.clone())
            .await
            .unwrap();

        let err = state
            .services
            .auth
            .finish_otp_setup(
                session.clone(),
                Otp {
                    token: generated.token,
                    otp_code: "000000".into(),
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            AppError::Auth(AuthFailure::InvalidCredentials)
        ));

        assert!(
            state
                .repository
                .users
                .get_by_id(user.id)
                .await
                .unwrap()
                .otp_secret
                .is_none()
        );

        // Wrong code still consumes the token.
        assert!(
            TokenStore::new(state.cache.clone())
                .get::<String>(OneShotToken::OneTimePassword, generated.token)
                .await
                .is_err()
        );
    }

    /// Correct code enables OTP; password login then requires 2FA.
    #[test]
    async fn finish_enables_otp_with_valid_code(
        state: TestState,
        user: UserView,
        session: Session,
    ) {
        let generated = state
            .services
            .auth
            .start_otp_setup(session.clone())
            .await
            .unwrap();

        let secret = pending_secret(state, generated.token).await;
        let code = totp_now(&secret);

        state
            .services
            .auth
            .finish_otp_setup(
                session.clone(),
                Otp {
                    token: generated.token,
                    otp_code: code,
                },
            )
            .await
            .unwrap();

        assert!(
            state
                .repository
                .users
                .get_by_id(user.id)
                .await
                .unwrap()
                .otp_secret
                .is_some()
        );

        let login = state
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
            .unwrap();

        assert!(matches!(login, LoginResult::TwoFactorRequired { .. }));
    }

    /// Setup token is single-use.
    #[test]
    async fn finish_rejects_consumed_token(state: TestState, session: Session) {
        let generated = state
            .services
            .auth
            .start_otp_setup(session.clone())
            .await
            .unwrap();
        let secret = pending_secret(state, generated.token).await;
        let code = totp_now(&secret);

        state
            .services
            .auth
            .finish_otp_setup(
                session.clone(),
                Otp {
                    token: generated.token,
                    otp_code: code.clone(),
                },
            )
            .await
            .unwrap();

        let err = state
            .services
            .auth
            .finish_otp_setup(
                session.clone(),
                Otp {
                    token: generated.token,
                    otp_code: code,
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::ResourceMissing(_)));
    }
}
