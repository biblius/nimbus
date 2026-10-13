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
            .start_overwrite(&session.user, root, original.id)
            .await
            .unwrap();

        writer.write(b"hello world").await.unwrap();
        let temp = writer.finish().await.unwrap();

        let updated = state
            .services
            .files
            .finish_overwrite(&session.user, existing, temp)
            .await
            .unwrap();

        assert_eq!(updated.id, original.id);
        assert_eq!(updated.name, "notes.txt");
        assert_eq!(updated.size_bytes, 11);
        assert_ne!(updated.hash, original.hash);

        let (_entry, path, mime) = state
            .services
            .files
            .content(&session.user, updated.id)
            .await
            .unwrap();

        assert_eq!(mime, "text/plain; charset=utf-8");
        assert_eq!(tokio::fs::read(&path).await.unwrap(), b"hello world");
        assert_eq!(test::blob_count(state.storage_dir()), before);
    }
}
