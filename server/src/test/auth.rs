#[suitest::suite(auth_service)]
#[suitest::suite_cfg(sequential = true)]
mod auth_service {
    use crate::{
        auth::model::{LoginResult, PasswordChange},
        error::AppError,
        test::TestState,
        user::model::{CreateUser, Role},
    };
    use suitest::before_all;

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
                },
                Some("127.0.0.1".into()),
                None,
            )
            .await
            .unwrap();

        let LoginResult::Session { session } = login else {
            panic!("expected a session");
        };
        assert_eq!(session.user.role, Role::Admin);
        assert!(session.first_login);

        let admin = state
            .services
            .auth
            .get_session_by_id(session.id)
            .await
            .unwrap();
        assert_eq!(admin.user.role, Role::Admin);
        assert!(!admin.user.password.is_empty());
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

        let session = crate::test::login(&state, "ada", "temporary").await;
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
}
