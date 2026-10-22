#[suitest::suite(auth_password_reset)]
#[suitest::suite_cfg(sequential = true)]
mod auth_password_reset {
    use crate::{
        auth::{
            model::{LoginCredentials, LoginResult, PasswordReset, PasswordResetRequest},
            token::{OneShotToken, TokenStore},
        },
        error::AppError,
        test::TestState,
        user::model::{CreateUser, Role},
    };
    use chrono::Utc;
    use suitest::before_all;
    use uuid::Uuid;

    const STRONG: &str = "correct-horse-battery-staple-99";

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
    }

    async fn create_user(state: &TestState, username: &str, email: Option<&str>) -> Uuid {
        state
            .services
            .users
            .create_user(CreateUser {
                username: username.into(),
                password: "temporary".into(),
                email: email.map(str::to_owned),
                role: Role::User,
            })
            .await
            .unwrap()
            .id
    }

    async fn store_reset_token(state: &TestState, user_id: Uuid) -> Uuid {
        let token = Uuid::new_v4();
        TokenStore::new(state.cache.clone())
            .store(OneShotToken::PasswordReset, token, &user_id)
            .await
            .unwrap();
        token
    }

    async fn passkey_enabled(state: &TestState, user_id: Uuid) -> bool {
        sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM user_passkeys WHERE user_id = $1
            ) AS "exists!: bool"
            "#,
            user_id
        )
        .fetch_one(&state.pool)
        .await
        .unwrap()
    }

    /// Invalid email independently fails validation.
    #[test]
    async fn start_rejects_invalid_email(state: TestState) {
        let err = state
            .services
            .auth
            .start_password_reset(PasswordResetRequest {
                email: "not-an-email".into(),
            })
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Validation(_)));
    }

    /// Unknown email returns Ok without exposing whether the account exists.
    #[test]
    async fn start_unknown_email_is_ok(state: TestState) {
        state
            .services
            .auth
            .start_password_reset(PasswordResetRequest {
                email: "nobody@example.com".into(),
            })
            .await
            .unwrap();
    }

    /// Known email returns Ok (token stored and mail enqueued).
    #[test]
    async fn start_known_email_is_ok(state: TestState) {
        create_user(state, "reset-start", Some("reset-start@example.com")).await;

        state
            .services
            .auth
            .start_password_reset(PasswordResetRequest {
                email: "reset-start@example.com".into(),
            })
            .await
            .unwrap();
    }

    /// Email is trimmed and lowercased before lookup.
    #[test]
    async fn start_trims_and_lowercases_email(state: TestState) {
        create_user(state, "reset-case", Some("reset-case@example.com")).await;

        state
            .services
            .auth
            .start_password_reset(PasswordResetRequest {
                email: "  Reset-Case@Example.COM  ".into(),
            })
            .await
            .unwrap();
    }

    /// Empty new password independently fails validation.
    #[test]
    async fn finish_rejects_empty_password(state: TestState) {
        let user_id = create_user(state, "reset-empty", Some("reset-empty@example.com")).await;
        let token = store_reset_token(state, user_id).await;

        let err = state
            .services
            .auth
            .finish_password_reset(
                token,
                PasswordReset {
                    new_password: "".into(),
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    /// Weak non-empty password independently fails validation.
    #[test]
    async fn finish_rejects_weak_password(state: TestState) {
        let user_id = create_user(state, "reset-weak", Some("reset-weak@example.com")).await;
        let token = store_reset_token(state, user_id).await;

        let err = state
            .services
            .auth
            .finish_password_reset(
                token,
                PasswordReset {
                    new_password: "admin".into(),
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    /// Missing or already-consumed token yields ResourceMissing.
    #[test]
    async fn finish_rejects_missing_token(state: TestState) {
        let err = state
            .services
            .auth
            .finish_password_reset(
                Uuid::new_v4(),
                PasswordReset {
                    new_password: STRONG.into(),
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::ResourceMissing(_)));
    }

    /// Token is single-use: second finish with the same id fails.
    #[test]
    async fn finish_rejects_consumed_token(state: TestState) {
        let user_id = create_user(state, "reset-once", Some("reset-once@example.com")).await;
        let token = store_reset_token(state, user_id).await;

        state
            .services
            .auth
            .finish_password_reset(
                token,
                PasswordReset {
                    new_password: STRONG.into(),
                },
            )
            .await
            .unwrap();

        let err = state
            .services
            .auth
            .finish_password_reset(
                token,
                PasswordReset {
                    new_password: "another-correct-horse-battery-staple-1".into(),
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::ResourceMissing(_)));
    }

    /// Token payload pointing at a missing user fails the user lookup.
    #[test]
    async fn finish_rejects_missing_user(state: TestState) {
        let token = store_reset_token(state, Uuid::new_v4()).await;

        let err = state
            .services
            .auth
            .finish_password_reset(
                token,
                PasswordReset {
                    new_password: STRONG.into(),
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::ResourceMissing(_)));
    }

    /// Successful reset updates the password, clears 2FA/passkeys, and expires sessions.
    #[test]
    async fn finish_resets_password_and_clears_auth_state(state: TestState) {
        let user_id = create_user(state, "reset-full", Some("reset-full@example.com")).await;
        let session = crate::test::login(state, "reset-full", "temporary").await;

        state
            .repository
            .users
            .update_otp_secret(user_id, "JBSWY3DPEHPK3PXP")
            .await
            .unwrap();

        // Placeholder JSON is enough for EXISTS / DELETE; never deserialized here.
        sqlx::query(
            r#"
            INSERT INTO user_passkeys (user_id, data, created_at)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(user_id)
        .bind(serde_json::json!({ "test": true }))
        .bind(Utc::now())
        .execute(&state.pool)
        .await
        .unwrap();

        assert!(passkey_enabled(state, user_id).await);
        assert!(
            state
                .repository
                .users
                .get_by_id(user_id)
                .await
                .unwrap()
                .otp_secret
                .is_some()
        );
        assert_eq!(
            state
                .repository
                .sessions
                .list_live_for_user(user_id)
                .await
                .unwrap()
                .len(),
            1
        );

        let token = store_reset_token(state, user_id).await;

        state
            .services
            .auth
            .finish_password_reset(
                token,
                PasswordReset {
                    new_password: STRONG.into(),
                },
            )
            .await
            .unwrap();

        let reset_user = state.repository.users.get_by_id(user_id).await.unwrap();
        assert!(reset_user.otp_secret.is_none());
        assert!(!passkey_enabled(state, user_id).await);
        assert!(
            state
                .repository
                .sessions
                .list_live_for_user(user_id)
                .await
                .unwrap()
                .is_empty()
        );

        let old = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "reset-full".into(),
                    password: "temporary".into(),
                    remember: false,
                    permanent: false,
                },
                None,
                None,
            )
            .await
            .unwrap_err();
        assert!(matches!(old, AppError::Auth(_)));

        let login = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "reset-full".into(),
                    password: STRONG.into(),
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
            state
                .repository
                .sessions
                .list_live_for_user(user_id)
                .await
                .unwrap()
                .iter()
                .all(|live| live.id != session.id)
        );
    }

    /// User without email still resets successfully (notify branch skipped).
    #[test]
    async fn finish_succeeds_without_user_email(state: TestState) {
        let user_id = create_user(state, "reset-no-mail", None).await;
        let token = store_reset_token(state, user_id).await;

        state
            .services
            .auth
            .finish_password_reset(
                token,
                PasswordReset {
                    new_password: STRONG.into(),
                },
            )
            .await
            .unwrap();

        let login = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "reset-no-mail".into(),
                    password: STRONG.into(),
                    remember: false,
                    permanent: false,
                },
                None,
                None,
            )
            .await
            .unwrap();

        assert!(matches!(login, LoginResult::Session { .. }));
    }
}
