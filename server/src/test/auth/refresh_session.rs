#[suitest::suite(auth_refresh_session)]
#[suitest::suite_cfg(sequential = true)]
mod auth_refresh_session {
    use crate::{
        auth::model::{LoginCredentials, LoginResult},
        error::{AppError, AuthFailure},
        test::TestState,
        user::model::{CreateUser, Role},
    };
    use chrono::{Duration, Utc};
    use suitest::before_all;

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

    async fn login(
        state: &TestState,
        username: &str,
        remember: bool,
        permanent: bool,
    ) -> crate::auth::session::Session {
        let LoginResult::Session { session_id } = state
            .services
            .auth
            .login(
                LoginCredentials {
                    username: username.into(),
                    password: "temporary".into(),
                    remember,
                    permanent,
                },
                Some("10.0.0.1".into()),
                Some("old-agent".into()),
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
            .unwrap()
    }

    /// Refresh issues a new id and removes the old row.
    #[test]
    async fn rotates_session_id_and_deletes_old(state: TestState) {
        create_user(state, "refresh-rotate").await;
        let old = login(state, "refresh-rotate", false, true).await;
        let old_id = old.id;

        let refreshed = state
            .services
            .auth
            .refresh_session(
                old,
                Some("10.0.0.2".into()),
                Some("new-agent".into()),
            )
            .await
            .unwrap();

        assert_ne!(refreshed.id, old_id);

        let err = state
            .services
            .auth
            .get_session_by_id(old_id)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            AppError::Auth(AuthFailure::SessionNotFound)
        ));

        let live = state
            .services
            .auth
            .get_session_by_id(refreshed.id)
            .await
            .unwrap();
        assert_eq!(live.id, refreshed.id);
        assert!(live.expires_at.is_none());
    }

    /// Permanent sessions stay permanent after refresh.
    #[test]
    async fn preserves_permanent_expiry(state: TestState) {
        create_user(state, "refresh-permanent").await;
        let session = login(state, "refresh-permanent", false, true).await;
        assert!(session.expires_at.is_none());

        let refreshed = state
            .services
            .auth
            .refresh_session(session, None, None)
            .await
            .unwrap();

        assert!(refreshed.expires_at.is_none());
    }

    /// Timed sessions keep the same absolute expires_at (no lifetime extension).
    #[test]
    async fn preserves_timed_expires_at(state: TestState) {
        create_user(state, "refresh-timed").await;
        let session = login(state, "refresh-timed", true, false).await;
        let expires_at = session.expires_at.expect("remember session has expiry");

        let refreshed = state
            .services
            .auth
            .refresh_session(session, None, None)
            .await
            .unwrap();

        assert_eq!(refreshed.expires_at, Some(expires_at));
    }

    /// Default (12h) sessions also keep the same absolute expiry.
    #[test]
    async fn preserves_default_expires_at(state: TestState) {
        create_user(state, "refresh-default").await;
        let before = Utc::now();
        let session = login(state, "refresh-default", false, false).await;
        let expires_at = session.expires_at.expect("default session has expiry");
        assert!(expires_at > before + Duration::hours(11));

        let refreshed = state
            .services
            .auth
            .refresh_session(session, None, None)
            .await
            .unwrap();

        assert_eq!(refreshed.expires_at, Some(expires_at));
    }

    /// Refresh updates remote address and user agent on the new row.
    #[test]
    async fn updates_client_metadata(state: TestState) {
        create_user(state, "refresh-meta").await;
        let session = login(state, "refresh-meta", false, true).await;

        let refreshed = state
            .services
            .auth
            .refresh_session(
                session,
                Some("203.0.113.9".into()),
                Some("NimbusDesktop/1.0".into()),
            )
            .await
            .unwrap();

        let row = state
            .repository
            .sessions
            .get(refreshed.id)
            .await
            .unwrap()
            .expect("session row");

        assert_eq!(row.remote_address.as_deref(), Some("203.0.113.9"));
        assert_eq!(row.user_agent.as_deref(), Some("NimbusDesktop/1.0"));
    }

    /// Other live sessions for the same user are left alone.
    #[test]
    async fn leaves_other_sessions_intact(state: TestState) {
        create_user(state, "refresh-others").await;
        let keep = login(state, "refresh-others", false, true).await;
        let rotate = login(state, "refresh-others", false, true).await;
        let keep_id = keep.id;

        let refreshed = state
            .services
            .auth
            .refresh_session(rotate, None, None)
            .await
            .unwrap();

        assert!(
            state
                .services
                .auth
                .get_session_by_id(keep_id)
                .await
                .is_ok()
        );
        assert!(
            state
                .services
                .auth
                .get_session_by_id(refreshed.id)
                .await
                .is_ok()
        );

        let live = state
            .repository
            .sessions
            .list_live_for_user(keep.user.id)
            .await
            .unwrap();
        let ids: Vec<_> = live.iter().map(|s| s.id).collect();
        assert!(ids.contains(&keep_id));
        assert!(ids.contains(&refreshed.id));
        assert_eq!(ids.len(), 2);
    }

    /// An already-expired session cannot be refreshed.
    #[test]
    async fn rejects_expired_session(state: TestState) {
        create_user(state, "refresh-expired").await;
        let mut session = login(state, "refresh-expired", false, false).await;
        session.expires_at = Some(Utc::now() - Duration::hours(1));

        let err = state
            .services
            .auth
            .refresh_session(session, None, None)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            AppError::Auth(AuthFailure::SessionNotFound)
        ));
    }
}
