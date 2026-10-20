#[suitest::suite(files_overwrite)]
#[suitest::suite_cfg(sequential = true)]
mod overwrite {
    use super::super::user;
    use crate::test::{self, TestState};
    use suitest::before_all;

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
    }

    #[test]
    async fn overwrite_replaces_blob_and_updates_row(state: TestState) {
        let _ada = user(state, "ada-overwrite").await;
        let session = test::login(state, "ada-overwrite", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let original = test::upload(state, &session.user, root, "notes.txt", b"hello").await;
        let before = test::blob_count(state.storage_dir());

        let (existing, mut writer) = state
            .services
            .files
            .start_overwrite(
                &super::super::ctx(state, &session.user, root).await,
                original.id,
            )
            .await
            .unwrap();

        writer.write(b"hello world").await.unwrap();
        let temp = writer.finish().await.unwrap();

        state
            .services
            .files
            .finish_overwrite(
                &super::super::ctx(state, &session.user, root).await,
                existing,
                temp,
            )
            .await
            .unwrap();

        let updated = state
            .services
            .files
            .metadata(
                &super::super::ctx(state, &session.user, root).await,
                original.id,
            )
            .await
            .unwrap();

        assert_eq!(updated.id, original.id);
        assert_eq!(updated.name, "notes.txt");
        assert_eq!(updated.size_bytes, 11);
        assert_ne!(updated.hash, original.hash);

        let (_name, body) = state
            .services
            .files
            .content_for_download(
                &super::super::ctx(state, &session.user, root).await,
                updated.id,
            )
            .await
            .unwrap();

        let crate::file::service::DownloadBody::File { path, mime } = body else {
            panic!("expected file download body");
        };
        assert_eq!(mime, "text/plain; charset=utf-8");
        assert_eq!(tokio::fs::read(&path).await.unwrap(), b"hello world");
        assert_eq!(test::blob_count(state.storage_dir()), before);
    }
}
