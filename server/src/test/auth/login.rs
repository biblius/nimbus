#[suitest::suite(auth_login)]
#[suitest::suite_cfg(sequential = true)]
mod auth_login {
    use crate::{
        auth::model::{LoginCredentials, LoginResult},
        error::{AppError, AuthFailure},
        test::TestState,
        user::model::{CreateUser, Role},
    };
    use chrono::{Duration, Utc};
    use suitest::before_all;
    use uuid::Uuid;

    /// Default session lifetime when neither remember nor permanent is set.
    const SESSION_HOURS: i64 = 12;
    /// Remember-me session lifetime.
    const REMEMBER_SESSION_DAYS: i64 = 30;

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
    }

    async fn create_user(state: &TestState, username: &str) {
        state
            .services
            .users
            .create_user(CreateUser {
                username: username.into(),
                password: "temporary".into(),
                email: None,
                role: Role::User,
            })
            .await
            .unwrap();
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

    /// Empty username independently fails validation (password held true).
    #[test]
    async fn rejects_empty_username(state: TestState) {
        let err = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "".into(),
                    password: "temporary".into(),
                    remember: false,
                    permanent: false,
                },
                None,
                None,
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Validation(_)));
    }

    /// Empty password independently fails validation (username held true).
    #[test]
    async fn rejects_empty_password(state: TestState) {
        create_user(state, "login-empty-pw").await;

        let err = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "login-empty-pw".into(),
                    password: "".into(),
                    remember: false,
                    permanent: false,
                },
                None,
                None,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    /// Username is not lowercased: case must match the stored value.
    #[test]
    async fn username_has_to_be_exact(state: TestState) {
        create_user(state, "login-Case").await;

        let err = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "login-case".into(),
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
            err,
            AppError::Auth(AuthFailure::InvalidCredentials)
        ));

        let result = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "login-Case".into(),
                    password: "temporary".into(),
                    remember: false,
                    permanent: false,
                },
                None,
                None,
            )
            .await
            .unwrap();

        assert!(matches!(result, LoginResult::Session { .. }));
    }

    /// Active backoff independently blocks (correct password held true).
    #[test]
    async fn active_backoff_blocks_correct_password(state: TestState) {
        create_user(state, "login-backoff-active").await;

        set_backoff(
            state,
            "login-backoff-active",
            Utc::now() + Duration::minutes(10),
        )
        .await;

        let err = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "login-backoff-active".into(),
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
            err,
            AppError::Auth(AuthFailure::LoginBackoff { .. })
        ));
    }

    /// Expired unlock_at allows login
    #[test]
    async fn expired_backoff_allows_login(state: TestState) {
        create_user(state, "login-backoff-expired").await;

        // TTL must outlive the call; unlock timestamp is in the past.
        state
            .cache
            .set_ex(
                "login:backoff:login-backoff-expired",
                &(Utc::now() - Duration::minutes(1)).to_rfc3339(),
                60,
            )
            .await
            .unwrap();

        let result = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "login-backoff-expired".into(),
                    password: "temporary".into(),
                    remember: false,
                    permanent: false,
                },
                None,
                None,
            )
            .await
            .unwrap();

        assert!(matches!(result, LoginResult::Session { .. }));
    }

    /// User lookup runs before backoff: unknown usernames never hit the throttle.
    #[test]
    async fn unknown_user_skips_backoff(state: TestState) {
        set_backoff(
            state,
            "login-unknown-locked",
            Utc::now() + Duration::minutes(5),
        )
        .await;

        let err = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "login-unknown-locked".into(),
                    password: "anything".into(),
                    remember: false,
                    permanent: false,
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

    /// Missing user independently yields InvalidCredentials without recording attempts.
    #[test]
    async fn unknown_username_is_invalid_credentials(state: TestState) {
        let err = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "login-ghost".into(),
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
            err,
            AppError::Auth(AuthFailure::InvalidCredentials)
        ));

        assert!(
            state
                .cache
                .get("login:attempts:login-ghost")
                .await
                .unwrap()
                .is_none()
        );
    }

    /// Wrong password independently yields InvalidCredentials (user held true).
    #[test]
    async fn wrong_password_is_invalid_credentials(state: TestState) {
        create_user(state, "login-bad-pw").await;

        let err = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "login-bad-pw".into(),
                    password: "not-temporary".into(),
                    remember: false,
                    permanent: false,
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
                .get("login:attempts:login-bad-pw")
                .await
                .unwrap()
                .and_then(|raw| raw.parse().ok()),
            Some(1)
        );
    }

    /// Password checked before 2FA: wrong password with OTP enabled yields credentials, not 2FA.
    #[test]
    async fn wrong_password_with_otp_does_not_challenge(state: TestState) {
        let user = state
            .services
            .users
            .create_user(CreateUser {
                username: "login-otp-bad-pw".into(),
                password: "temporary".into(),
                email: None,
                role: Role::User,
            })
            .await
            .unwrap();

        state
            .repository
            .users
            .update_otp_secret(user.id, "JBSWY3DPEHPK3PXP")
            .await
            .unwrap();

        let err = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "login-otp-bad-pw".into(),
                    password: "wrong".into(),
                    remember: false,
                    permanent: false,
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

    /// `otp_secret.is_some()` yields TwoFactorRequired; attempts are not cleared.
    #[test]
    async fn otp_secret_requires_two_factor(state: TestState) {
        let user = state
            .services
            .users
            .create_user(CreateUser {
                username: "login-otp-on".into(),
                password: "temporary".into(),
                email: None,
                role: Role::User,
            })
            .await
            .unwrap();

        state
            .repository
            .users
            .update_otp_secret(user.id, "JBSWY3DPEHPK3PXP")
            .await
            .unwrap();

        // Prior failure must remain until a real session is created.
        state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "login-otp-on".into(),
                    password: "wrong".into(),
                    remember: false,
                    permanent: false,
                },
                None,
                None,
            )
            .await
            .unwrap_err();

        assert_eq!(
            state
                .cache
                .get("login:attempts:login-otp-on")
                .await
                .unwrap()
                .and_then(|raw| raw.parse().ok()),
            Some(1)
        );

        let result = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "login-otp-on".into(),
                    password: "temporary".into(),
                    remember: false,
                    permanent: false,
                },
                None,
                None,
            )
            .await
            .unwrap();

        let LoginResult::TwoFactorRequired { token } = result else {
            panic!("expected TwoFactorRequired, got {result:?}");
        };

        let token: Uuid = token.parse().unwrap();

        // Attempt counter not cleared on 2FA challenge.
        assert_eq!(
            state
                .cache
                .get("login:attempts:login-otp-on")
                .await
                .unwrap()
                .and_then(|raw| raw.parse().ok()),
            Some(1)
        );

        let pending = state
            .cache
            .get(&format!("token:login:{token}"))
            .await
            .unwrap()
            .expect("pending login token stored");

        let body: serde_json::Value = serde_json::from_str(&pending).unwrap();

        assert_eq!(body["user_id"], user.id.to_string());
        assert_eq!(body["remember"], false);
        assert_eq!(body["permanent"], false);
    }

    /// None otp_secret independently skips 2FA and creates a session.
    #[test]
    async fn absent_otp_secret_creates_session(state: TestState) {
        create_user(state, "login-otp-none").await;

        let result = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "login-otp-none".into(),
                    password: "temporary".into(),
                    remember: false,
                    permanent: false,
                },
                None,
                None,
            )
            .await
            .unwrap();

        assert!(matches!(result, LoginResult::Session { .. }));
    }

    /// Pending login carries remember/permanent for the OTP finish step.
    #[test]
    async fn two_factor_pending_preserves_session_flags(state: TestState) {
        let user = state
            .services
            .users
            .create_user(CreateUser {
                username: "login-otp-flags".into(),
                password: "temporary".into(),
                email: None,
                role: Role::User,
            })
            .await
            .unwrap();

        state
            .repository
            .users
            .update_otp_secret(user.id, "JBSWY3DPEHPK3PXP")
            .await
            .unwrap();

        let result = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "login-otp-flags".into(),
                    password: "temporary".into(),
                    remember: true,
                    permanent: true,
                },
                None,
                None,
            )
            .await
            .unwrap();

        let LoginResult::TwoFactorRequired { token } = result else {
            panic!("expected TwoFactorRequired");
        };

        let token: Uuid = token.parse().unwrap();

        let pending = state
            .cache
            .get(&format!("token:login:{token}"))
            .await
            .unwrap()
            .unwrap();

        let body: serde_json::Value = serde_json::from_str(&pending).unwrap();

        assert_eq!(body["remember"], true);
        assert_eq!(body["permanent"], true);
    }

    /// Not permanent and not remember yields about 12h expiry.
    #[test]
    async fn default_session_expires_in_twelve_hours(state: TestState) {
        create_user(state, "login-exp-default").await;

        let before = Utc::now();

        let LoginResult::Session { session_id } = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "login-exp-default".into(),
                    password: "temporary".into(),
                    remember: false,
                    permanent: false,
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

        let expires = session.expires_at.expect("default session has expiry");
        let after = Utc::now();

        assert!(expires >= before + Duration::hours(SESSION_HOURS) - Duration::seconds(5));
        assert!(expires <= after + Duration::hours(SESSION_HOURS) + Duration::seconds(5));
    }

    /// Not permanent and remember independently extends to about 30 days.
    #[test]
    async fn remember_session_expires_in_thirty_days(state: TestState) {
        create_user(state, "login-exp-remember").await;

        let before = Utc::now();

        let LoginResult::Session { session_id } = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "login-exp-remember".into(),
                    password: "temporary".into(),
                    remember: true,
                    permanent: false,
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
        let expires = session.expires_at.expect("remember session has expiry");

        let after = Utc::now();

        assert!(expires >= before + Duration::days(REMEMBER_SESSION_DAYS) - Duration::seconds(5));
        assert!(expires <= after + Duration::days(REMEMBER_SESSION_DAYS) + Duration::seconds(5));
    }

    /// Permanent independently clears expiry (remember held false).
    #[test]
    async fn permanent_session_has_no_expiry(state: TestState) {
        create_user(state, "login-exp-perm").await;

        let LoginResult::Session { session_id } = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "login-exp-perm".into(),
                    password: "temporary".into(),
                    remember: false,
                    permanent: true,
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

    /// permanent overrides remember; both true still means no expiry.
    #[test]
    async fn permanent_overrides_remember(state: TestState) {
        create_user(state, "login-exp-both").await;

        let LoginResult::Session { session_id } = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "login-exp-both".into(),
                    password: "temporary".into(),
                    remember: true,
                    permanent: true,
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

    /// Successful session clears attempt counter (exit: Session after prior failure).
    #[test]
    async fn successful_login_clears_attempts(state: TestState) {
        create_user(state, "login-clear").await;

        state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "login-clear".into(),
                    password: "wrong".into(),
                    remember: false,
                    permanent: false,
                },
                None,
                None,
            )
            .await
            .unwrap_err();

        assert_eq!(
            state
                .cache
                .get("login:attempts:login-clear")
                .await
                .unwrap()
                .and_then(|raw| raw.parse().ok()),
            Some(1)
        );

        let result = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "login-clear".into(),
                    password: "temporary".into(),
                    remember: false,
                    permanent: false,
                },
                None,
                None,
            )
            .await
            .unwrap();

        assert!(matches!(result, LoginResult::Session { .. }));
        assert_eq!(
            state
                .cache
                .get("login:attempts:login-clear")
                .await
                .unwrap()
                .and_then(|raw| raw.parse().ok()),
            None::<u64>
        );
        assert!(
            state
                .cache
                .get("login:backoff:login-clear")
                .await
                .unwrap()
                .is_none()
        );
    }

    /// Every 5th failure sets backoff; a later success clears it.
    #[test]
    async fn five_failures_engage_backoff_then_success_clears(state: TestState) {
        create_user(state, "login-five").await;

        let wrong = LoginCredentials {
            username: "login-five".into(),
            password: "wrong".into(),
            remember: false,
            permanent: false,
        };

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

        let blocked = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "login-five".into(),
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
            blocked,
            AppError::Auth(AuthFailure::LoginBackoff { .. })
        ));

        state.cache.del("login:backoff:login-five").await.unwrap();

        let ok = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "login-five".into(),
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

        assert_eq!(
            state
                .cache
                .get("login:attempts:login-five")
                .await
                .unwrap()
                .and_then(|raw| raw.parse().ok()),
            None::<u64>
        );
    }

    /// Session path records remote address and user agent when provided.
    #[test]
    async fn session_stores_remote_address_and_user_agent(state: TestState) {
        let user = state
            .services
            .users
            .create_user(CreateUser {
                username: "login-meta".into(),
                password: "temporary".into(),
                email: None,
                role: Role::User,
            })
            .await
            .unwrap();

        let LoginResult::Session { session_id } = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "login-meta".into(),
                    password: "temporary".into(),
                    remember: false,
                    permanent: false,
                },
                Some("203.0.113.10".into()),
                Some("nimbus-test/1.0".into()),
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

        assert_eq!(row.remote_address.as_deref(), Some("203.0.113.10"));
        assert_eq!(row.user_agent.as_deref(), Some("nimbus-test/1.0"));
    }

    /// Seeded admin entry path
    #[test]
    async fn seeded_admin_can_log_in(state: TestState) {
        let result = state
            .services
            .auth
            .login(
                LoginCredentials {
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

        let LoginResult::Session { session_id } = result else {
            panic!("expected Session");
        };

        let admin = state
            .services
            .auth
            .get_session_by_id(session_id)
            .await
            .unwrap();

        assert_eq!(admin.user.role, Role::Admin);
    }

    /// Logout expires the session so it is no longer live.
    #[test]
    async fn logout_expires_session(state: TestState) {
        create_user(state, "login-logout").await;

        let LoginResult::Session { session_id } = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "login-logout".into(),
                    password: "temporary".into(),
                    remember: false,
                    permanent: false,
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
        assert!(!session.is_expired());

        state.services.auth.logout(session_id).await.unwrap();

        let session = state
            .services
            .auth
            .get_session_by_id(session_id)
            .await
            .unwrap();
        assert!(session.is_expired());
        assert!(
            state
                .repository
                .sessions
                .list_live_for_user(session.user.id)
                .await
                .unwrap()
                .is_empty()
        );
    }
}
