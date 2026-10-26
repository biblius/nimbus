#[suitest::suite(configuration_service)]
#[suitest::suite_cfg(sequential = true)]
mod configuration_service {
    use crate::{config::ConfigUpdate, test::TestState};
    use suitest::before_all;

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
    }

    #[test]
    async fn get_returns_current_runtime_settings(state: TestState) {
        let config = state.services.config.get().await;
        assert_eq!(config.file_retention_days, 30);
        assert_eq!(config.nimbus_origin, "http://localhost:5173");
        assert_eq!(config.upload_max_bytes, 1024 * 1024);
        assert_eq!(config.email_from, "Nimbus");
        assert!(config.smtp_password.is_empty());
    }

    #[test]
    async fn update_swaps_values_immediately(state: TestState) {
        let updated = state
            .services
            .config
            .update(ConfigUpdate {
                file_retention_days: Some(12),
                email_from: Some("Ops".into()),
                nimbus_origin: Some("https://files.example.com/".into()),
                upload_max_bytes: Some(2048),
                ..Default::default()
            })
            .await
            .unwrap();

        assert_eq!(updated.file_retention_days, 12);
        assert_eq!(updated.email_from, "Ops");
        assert_eq!(updated.nimbus_origin, "https://files.example.com");
        assert_eq!(updated.upload_max_bytes, 2048);

        assert_eq!(
            state.services.config.inner().file_retention_days().await,
            12
        );
        assert_eq!(
            state.services.config.inner().nimbus_origin().await,
            "https://files.example.com"
        );
        assert_eq!(state.services.config.inner().upload_max_bytes().await, 2048);
    }

    #[test]
    async fn update_rejects_invalid_origin(state: TestState) {
        let err = state
            .services
            .config
            .update(ConfigUpdate {
                nimbus_origin: Some("not a url".into()),
                ..Default::default()
            })
            .await
            .unwrap_err();

        assert!(matches!(err, crate::error::AppError::Unprocessable(_)));
    }

    #[test]
    async fn partial_update_leaves_other_fields_alone(state: TestState) {
        state
            .services
            .config
            .update(ConfigUpdate {
                file_retention_days: Some(21),
                smtp_host: Some("smtp.example.com".into()),
                smtp_port: Some(465),
                smtp_username: Some("nimbus@example.com".into()),
                smtp_password: Some("secret".into()),
                ..Default::default()
            })
            .await
            .unwrap();

        let before = state.services.config.inner().get().await;

        state
            .services
            .config
            .update(ConfigUpdate {
                email_from: Some("Support".into()),
                ..Default::default()
            })
            .await
            .unwrap();

        let after = state.services.config.inner().get().await;
        assert_eq!(after.email_from, "Support");
        assert_eq!(after.file_retention_days, before.file_retention_days);
        assert_eq!(after.smtp_host, before.smtp_host);
        assert_eq!(after.smtp_password, "secret");

        let view = state.services.config.get().await;
        assert_eq!(view.smtp_password, "<redacted>");
    }

    #[test]
    async fn auth_reads_nimbus_origin_from_shared_config(state: TestState) {
        state
            .services
            .config
            .update(ConfigUpdate {
                nimbus_origin: Some("https://passkeys.example.com".into()),
                ..Default::default()
            })
            .await
            .unwrap();

        let challenge = state
            .services
            .auth
            .start_passkey_login(crate::auth::model::StartPasskeyLogin {
                token: None,
                remember: false,
                permanent: false,
            })
            .await
            .unwrap();

        assert_eq!(&*challenge.nimbus_origin, "https://passkeys.example.com");
    }
}
