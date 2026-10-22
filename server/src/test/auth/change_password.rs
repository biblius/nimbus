#[suitest::suite(auth_change_password)]
#[suitest::suite_cfg(sequential = true)]
mod auth_change_password {
    use crate::{
        auth::model::{LoginCredentials, LoginResult, PasswordChange},
        error::{AppError, AuthFailure},
        test::TestState,
        user::model::{CreateUser, Role},
    };
    use suitest::before_all;

    const STRONG: &str = "correct-horse-battery-staple-99";

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
    }

    async fn create_user(state: &TestState, username: &str, email: Option<&str>) {
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
            .unwrap();
    }

    async fn session(state: &TestState, username: &str) -> crate::auth::session::Session {
        crate::test::login(state, username, "temporary").await
    }

    /// Empty current password independently fails validation (new held strong).
    #[test]
    async fn rejects_empty_current_password(state: TestState) {
        create_user(state, "pw-empty-cur", None).await;
        let session = session(state, "pw-empty-cur").await;

        let err = state
            .services
            .auth
            .change_password(
                session,
                PasswordChange {
                    current_password: "".into(),
                    new_password: STRONG.into(),
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Validation(_)));
    }

    /// Empty new password independently fails validation (current held true).
    #[test]
    async fn rejects_empty_new_password(state: TestState) {
        create_user(state, "pw-empty-new", None).await;
        let session = session(state, "pw-empty-new").await;

        let err = state
            .services
            .auth
            .change_password(
                session,
                PasswordChange {
                    current_password: "temporary".into(),
                    new_password: "".into(),
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Validation(_)));
    }

    /// Weak non-empty new password independently fails validation.
    #[test]
    async fn rejects_weak_new_password(state: TestState) {
        create_user(state, "pw-weak", None).await;
        let session = session(state, "pw-weak").await;

        let err = state
            .services
            .auth
            .change_password(
                session,
                PasswordChange {
                    current_password: "temporary".into(),
                    new_password: "admin".into(),
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Validation(_)));
    }

    /// Missing user for the session's user id yields InvalidCredentials.
    #[test]
    async fn missing_user_is_invalid_credentials(state: TestState) {
        create_user(state, "pw-gone", None).await;
        let mut session = session(state, "pw-gone").await;
        session.user.id = uuid::Uuid::new_v4();

        let err = state
            .services
            .auth
            .change_password(
                session,
                PasswordChange {
                    current_password: "temporary".into(),
                    new_password: STRONG.into(),
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            AppError::Auth(AuthFailure::InvalidCredentials)
        ));
    }

    /// Wrong current password independently yields InvalidCredentials.
    #[test]
    async fn wrong_current_password_is_invalid_credentials(state: TestState) {
        create_user(state, "pw-wrong", None).await;
        let session = session(state, "pw-wrong").await;

        let err = state
            .services
            .auth
            .change_password(
                session,
                PasswordChange {
                    current_password: "not-temporary".into(),
                    new_password: STRONG.into(),
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            AppError::Auth(AuthFailure::InvalidCredentials)
        ));

        // Password unchanged.
        let still_old = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "pw-wrong".into(),
                    password: "temporary".into(),
                    remember: false,
                    permanent: false,
                },
                None,
                None,
            )
            .await
            .unwrap();

        assert!(matches!(still_old, LoginResult::Session { .. }));
    }

    /// Successful change updates the password and keeps the calling session.
    #[test]
    async fn changes_password_and_keeps_current_session(state: TestState) {
        create_user(state, "pw-ok", None).await;

        let session = session(state, "pw-ok").await;
        let session_id = session.id;

        state
            .services
            .auth
            .change_password(
                session,
                PasswordChange {
                    current_password: "temporary".into(),
                    new_password: STRONG.into(),
                },
            )
            .await
            .unwrap();

        let old = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "pw-ok".into(),
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
            old,
            AppError::Auth(AuthFailure::InvalidCredentials)
        ));

        let new = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: "pw-ok".into(),
                    password: STRONG.into(),
                    remember: false,
                    permanent: false,
                },
                None,
                None,
            )
            .await
            .unwrap();

        assert!(matches!(new, LoginResult::Session { .. }));

        state
            .services
            .auth
            .get_session_by_id(session_id)
            .await
            .unwrap();
    }

    /// Other live sessions are expired; the current one remains.
    #[test]
    async fn expires_other_sessions(state: TestState) {
        create_user(state, "pw-sessions", None).await;

        let keep = session(state, "pw-sessions").await;
        let other = session(state, "pw-sessions").await;

        assert_ne!(keep.id, other.id);

        state
            .services
            .auth
            .change_password(
                keep.clone(),
                PasswordChange {
                    current_password: "temporary".into(),
                    new_password: STRONG.into(),
                },
            )
            .await
            .unwrap();

        let live = state
            .repository
            .sessions
            .list_live_for_user(keep.user.id)
            .await
            .unwrap();

        assert_eq!(live.len(), 1);
        assert_eq!(live[0].id, keep.id);

        let other_row = state
            .repository
            .sessions
            .get(other.id)
            .await
            .unwrap()
            .expect("session row still exists");

        assert!(
            other_row
                .expires_at
                .is_some_and(|expires_at| expires_at <= chrono::Utc::now())
        );
    }

    /// Email present: change still succeeds (notify branch taken).
    #[test]
    async fn succeeds_when_user_has_email(state: TestState) {
        create_user(state, "pw-email", Some("pw-email@example.com")).await;
        let session = session(state, "pw-email").await;

        state
            .services
            .auth
            .change_password(
                session,
                PasswordChange {
                    current_password: "temporary".into(),
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
                    username: "pw-email".into(),
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
