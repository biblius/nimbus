#[suitest::suite(auth_service)]
#[suitest::suite_cfg(sequential = true)]
mod auth_service {
    use crate::{
        error::AppError,
        test::TestState,
        user::model::{CreateUser, Role, UpdateUser},
    };
    use suitest::before_all;

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
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
}
