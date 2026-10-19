#[suitest::suite(auth_service)]
#[suitest::suite_cfg(sequential = true)]
mod auth_service {
    use crate::{
        auth::model::{LoginCredentials, LoginResult, PasswordChange},
        db::DbPool,
        error::{AppError, AuthFailure},
        test::TestState,
        user::model::{CreateUser, Role, UpdateUser},
    };
    use suitest::before_all;
    use uuid::Uuid;

    /// Whether the user has at least one registered passkey.
    async fn is_passkey_enabled(pool: &DbPool, user_id: Uuid) -> bool {
        sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM user_passkeys WHERE user_id = $1
            ) AS "exists!: bool"
            "#,
            user_id
        )
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
    }

    #[test]
    async fn seeded_admin_can_log_in(state: TestState) {
        let login = state
            .services
            .auth
            .login(
                crate::auth::model::LoginCredentials {
                    username: "admin".into(),
                    password: "admin".into(),
                    remember: false,
                    permanent: false,
                },
                Some("127.0.0.1".into()),
                None,
            )
            .await
            .unwrap();

        let LoginResult::Session { session_id } = login else {
            panic!("expected a session");
        };

        let admin = state
            .services
            .auth
            .get_session_by_id(session_id)
            .await
            .unwrap();
        assert_eq!(admin.user.role, Role::Admin);
        assert!(!admin.user.password.is_empty());
    }

    #[test]
    async fn login_backoff_after_ten_failures_clears_on_success(state: TestState) {
        state
            .services
            .users
            .create_user(CreateUser {
                username: "ada-backoff".into(),
                password: "temporary".into(),
                email: None,
                role: Role::User,
            })
            .await
            .unwrap();

        let wrong = LoginCredentials {
            username: "ada-backoff".into(),
            password: "wrong-password".into(),
            remember: false,
            permanent: false,
        };

        // 5 failures: attempts % 5 == 0 sets backoff of (5/10)*10 = 0 minutes (no lock).
        for _ in 0..5 {
            let err = state
                .services
                .auth
                .login(wrong.clone(), None, None)
                .await
                .unwrap_err();
            assert!(matches!(
                err,
                AppError::Auth(AuthFailure::InvalidCredentials)
            ));
        }

        let still_open = state
            .services
            .auth
            .login(wrong.clone(), None, None)
            .await
            .unwrap_err();
        assert!(matches!(
            still_open,
            AppError::Auth(AuthFailure::InvalidCredentials)
        ));

        // Failures 7..10 (4 more) → 10 total; (10/10)*10 = 10 minute backoff.
        for _ in 0..3 {
            let err = state
                .services
                .auth
                .login(wrong.clone(), None, None)
                .await
                .unwrap_err();
            assert!(matches!(
                err,
                AppError::Auth(AuthFailure::InvalidCredentials)
            ));
        }

        let locked = state
            .services
            .auth
            .login(wrong.clone(), None, None)
            .await
            .unwrap_err();
        // 10th failure records then returns InvalidCredentials; next attempt is locked.
        assert!(matches!(
            locked,
            AppError::Auth(AuthFailure::InvalidCredentials)
        ));

        let backoff = state
            .services
            .auth
            .login(wrong.clone(), None, None)
            .await
            .unwrap_err();
        assert!(matches!(
            backoff,
            AppError::Auth(AuthFailure::LoginBackoff { .. })
        ));

        // Correct password is also blocked while locked.
        let blocked_correct = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "ada-backoff".into(),
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
            blocked_correct,
            AppError::Auth(AuthFailure::LoginBackoff { .. })
        ));

        // Expire the backoff entry so we can verify a successful login clears throttle.
        state.cache.del("login:backoff:ada-backoff").await.unwrap();

        let ok = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "ada-backoff".into(),
                    password: "temporary".into(),
                    remember: false,
                    permanent: false,
                },
                None,
                None,
            )
            .await
            .unwrap();
        assert!(matches!(ok, LoginResult::Session { .. }));

        // After success, failures start from zero again (no immediate lock).
        for _ in 0..5 {
            let err = state
                .services
                .auth
                .login(wrong.clone(), None, None)
                .await
                .unwrap_err();
            assert!(matches!(
                err,
                AppError::Auth(AuthFailure::InvalidCredentials)
            ));
        }
    }

    #[test]
    async fn otp_failures_share_login_backoff(state: TestState) {
        use crate::auth::model::Otp;
        use crate::db::UserRepository;

        let user = state
            .services
            .users
            .create_user(CreateUser {
                username: "ada-otp-backoff".into(),
                password: "temporary".into(),
                email: None,
                role: Role::User,
            })
            .await
            .unwrap();

        UserRepository::new(state.pool.clone())
            .update_otp_secret(user.id, "JBSWY3DPEHPK3PXP")
            .await
            .unwrap();

        let login = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "ada-otp-backoff".into(),
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
            panic!("expected two-factor challenge");
        };
        let token: Uuid = token.parse().unwrap();

        let wrong = Otp {
            token,
            otp_code: "000000".into(),
        };

        for _ in 0..10 {
            let err = state
                .services
                .auth
                .login_with_otp(wrong.clone(), None, None)
                .await
                .unwrap_err();
            assert!(matches!(
                err,
                AppError::Auth(AuthFailure::InvalidCredentials)
                    | AppError::Auth(AuthFailure::LoginBackoff { .. })
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

        // Password login is locked on the same counter.
        let password_locked = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "ada-otp-backoff".into(),
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

    #[test]
    async fn weak_password_is_rejected(state: TestState) {
        let ada = state
            .services
            .users
            .create_user(CreateUser {
                username: "ada".into(),
                password: "temporary".into(),
                email: None,
                role: Role::User,
            })
            .await
            .unwrap();
        assert_eq!(ada.role, Role::User);

        let session = crate::test::login(state, "ada", "temporary").await;
        let weak = state
            .services
            .auth
            .change_password(
                session,
                PasswordChange {
                    current_password: "temporary".into(),
                    new_password: "admin".into(),
                },
            )
            .await;

        assert!(matches!(weak, Err(AppError::Validation(_))));
    }

    #[test]
    async fn admin_can_edit_a_user(state: TestState) {
        let ada = state
            .services
            .users
            .create_user(CreateUser {
                username: "ada-edit".into(),
                password: "temporary".into(),
                email: None,
                role: Role::User,
            })
            .await
            .unwrap();

        let updated = state
            .services
            .users
            .update_user(
                ada.id,
                UpdateUser {
                    username: Some("ada-renamed".into()),
                    email: Some("ada@example.com".into()),
                    role: Role::User,
                    password: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(updated.id, ada.id);
        assert_eq!(updated.username, "ada-renamed");
        assert_eq!(updated.email.as_deref(), Some("ada@example.com"));
        assert_eq!(updated.role, Role::User);
    }

    #[test]
    async fn user_can_edit_their_own_profile(state: TestState) {
        let ada = state
            .services
            .users
            .create_user(CreateUser {
                username: "ada-self".into(),
                password: "temporary".into(),
                email: None,
                role: Role::User,
            })
            .await
            .unwrap();

        let session = crate::test::login(state, "ada-self", "temporary").await;

        let updated = state
            .services
            .users
            .update_self(
                ada.id,
                session.id,
                crate::user::model::UpdateSelf {
                    username: Some("ada-self-renamed".into()),
                    email: Some("ada-self@example.com".into()),
                    current_password: None,
                    password: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(updated.id, ada.id);
        assert_eq!(updated.username, "ada-self-renamed");
        assert_eq!(updated.email.as_deref(), Some("ada-self@example.com"));
        assert_eq!(updated.role, Role::User);
        assert!(!updated.otp_enabled);
    }

    #[test]
    async fn update_self_password_requires_correct_current(state: TestState) {
        use crate::error::{AppError, AuthFailure};

        let ada = state
            .services
            .users
            .create_user(CreateUser {
                username: "ada-pw-self".into(),
                password: "temporary".into(),
                email: None,
                role: Role::User,
            })
            .await
            .unwrap();

        let session = crate::test::login(state, "ada-pw-self", "temporary").await;

        let missing = state
            .services
            .users
            .update_self(
                ada.id,
                session.id,
                crate::user::model::UpdateSelf {
                    username: None,
                    email: None,
                    current_password: None,
                    password: Some("correct-horse-battery-staple-99".into()),
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(missing, AppError::Unprocessable(_)));

        let wrong = state
            .services
            .users
            .update_self(
                ada.id,
                session.id,
                crate::user::model::UpdateSelf {
                    username: None,
                    email: None,
                    current_password: Some("nope".into()),
                    password: Some("correct-horse-battery-staple-99".into()),
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(
            wrong,
            AppError::Auth(AuthFailure::InvalidCredentials)
        ));

        state
            .services
            .users
            .update_self(
                ada.id,
                session.id,
                crate::user::model::UpdateSelf {
                    username: None,
                    email: None,
                    current_password: Some("temporary".into()),
                    password: Some("correct-horse-battery-staple-99".into()),
                },
            )
            .await
            .unwrap();

        let login = state
            .services
            .auth
            .login(
                crate::auth::model::LoginCredentials {
                    username: "ada-pw-self".into(),
                    password: "correct-horse-battery-staple-99".into(),
                    remember: false,
                    permanent: false,
                },
                None,
                None,
            )
            .await
            .unwrap();
        assert!(matches!(
            login,
            crate::auth::model::LoginResult::Session { .. }
        ));
    }

    #[test]
    async fn disable_otp_rejects_when_not_enabled(state: TestState) {
        state
            .services
            .users
            .create_user(CreateUser {
                username: "ada-otp".into(),
                password: "temporary".into(),
                email: None,
                role: Role::User,
            })
            .await
            .unwrap();

        let session = crate::test::login(state, "ada-otp", "temporary").await;

        let result = state
            .services
            .auth
            .disable_otp(
                session,
                crate::auth::model::DisableOtp {
                    otp_code: "123456".into(),
                },
            )
            .await;

        assert!(matches!(result, Err(AppError::Unprocessable(_))));
    }

    #[test]
    async fn password_reset_clears_2fa_and_sessions(state: TestState) {
        use crate::{
            auth::{
                model::{LoginCredentials, LoginResult, PasswordReset},
                token::{OneShotToken, TokenStore},
            },
            db::{SessionRepository, UserRepository},
        };
        use chrono::Utc;
        use uuid::Uuid;

        let user = state
            .services
            .users
            .create_user(CreateUser {
                username: "ada-reset".into(),
                password: "temporary".into(),
                email: Some("ada-reset@example.com".into()),
                role: Role::User,
            })
            .await
            .unwrap();

        let session = crate::test::login(state, "ada-reset", "temporary").await;

        let users = UserRepository::new(state.pool.clone());
        let sessions = SessionRepository::new(state.pool.clone());

        users
            .update_otp_secret(user.id, "JBSWY3DPEHPK3PXP")
            .await
            .unwrap();

        // Placeholder JSON is enough for EXISTS / DELETE; we never deserialize it here.
        sqlx::query(
            r#"
            INSERT INTO user_passkeys (user_id, data, created_at)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(user.id)
        .bind(serde_json::json!({ "test": true }))
        .bind(Utc::now())
        .execute(&state.pool)
        .await
        .unwrap();

        assert!(is_passkey_enabled(&state.pool, user.id).await);
        assert!(users.get_by_id(user.id).await.unwrap().otp_secret.is_some());
        assert_eq!(sessions.list_live_for_user(user.id).await.unwrap().len(), 1);

        let token = Uuid::new_v4();
        TokenStore::new(state.cache.clone())
            .store(OneShotToken::PasswordReset, token, &user.id)
            .await
            .unwrap();

        state
            .services
            .auth
            .finish_password_reset(
                token,
                PasswordReset {
                    new_password: "correct-horse-battery-staple-99".into(),
                },
            )
            .await
            .unwrap();

        let reset_user = users.get_by_id(user.id).await.unwrap();
        assert!(reset_user.otp_secret.is_none());
        assert!(!is_passkey_enabled(&state.pool, user.id).await);
        assert!(
            sessions
                .list_live_for_user(user.id)
                .await
                .unwrap()
                .is_empty()
        );

        let old_login = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "ada-reset".into(),
                    password: "temporary".into(),
                    remember: false,
                    permanent: false,
                },
                None,
                None,
            )
            .await;
        assert!(matches!(old_login, Err(AppError::Auth(_))));

        let login = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "ada-reset".into(),
                    password: "correct-horse-battery-staple-99".into(),
                    remember: false,
                    permanent: false,
                },
                None,
                None,
            )
            .await
            .unwrap();

        let LoginResult::Session { .. } = login else {
            panic!("expected a session without 2FA after password reset");
        };

        assert!(
            sessions
                .list_live_for_user(user.id)
                .await
                .unwrap()
                .iter()
                .all(|live| live.id != session.id)
        );
    }
}
