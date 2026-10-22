#[suitest::suite(files_overwrite)]
#[suitest::suite_cfg(sequential = true)]
mod overwrite {
    use super::super::{ctx, shared_root, user};
    use crate::{
        auth::session::Session,
        error::AppError,
        file::model::CreateDirectory,
        test::{self, TestState},
        user::model::CreateSharedFolder,
    };
    use suitest::{before_all, before_each};
    use uuid::Uuid;

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
    }

    /// Private workspace: logged-in user and their root.
    #[before_each]
    async fn workspace(state: TestState) -> (Session, Uuid) {
        let username = format!("ow-{}", Uuid::new_v4());
        user(state, &username).await;
        let session = test::login(state, &username, "temporary").await;
        let root = test::private_root(state, &session.user).await;
        (session, root)
    }

    async fn overwrite(
        state: &TestState,
        session: &Session,
        root: Uuid,
        file_id: Uuid,
        bytes: &[u8],
    ) {
        let (existing, mut writer) = state
            .services
            .files
            .start_overwrite(&ctx(state, &session.user, root).await, file_id)
            .await
            .unwrap();
        writer.write(bytes).await.unwrap();
        let temp = writer.finish().await.unwrap();
        state
            .services
            .files
            .finish_overwrite(&ctx(state, &session.user, root).await, existing, temp)
            .await
            .unwrap();
    }

    /// Missing live file independently fails start_overwrite.
    #[test]
    async fn start_missing_file(state: TestState, session: Session, root: Uuid) {
        let err = state
            .services
            .files
            .start_overwrite(&ctx(state, &session.user, *root).await, Uuid::new_v4())
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::ResourceMissing(_)));
    }

    /// Directory independently fails start_overwrite (existence held true).
    #[test]
    async fn start_rejects_directory(state: TestState, session: Session, root: Uuid) {
        let folder = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
                CreateDirectory {
                    parent: None,
                    name: "docs".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let err = state
            .services
            .files
            .start_overwrite(&ctx(state, &session.user, *root).await, folder.id)
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::ResourceMissing(_)));
    }

    /// Non-text file independently fails start_overwrite (kind File held true).
    #[test]
    async fn start_rejects_non_text_file(state: TestState, session: Session, root: Uuid) {
        let file = test::upload(state, &session.user, *root, "pic.png", &[0x89, 0x50, 0x4E, 0x47])
            .await;

        let err = state
            .services
            .files
            .start_overwrite(&ctx(state, &session.user, *root).await, file.id)
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Unprocessable(_)));
    }

    /// Same hash independently skips persist on finish_overwrite.
    #[test]
    async fn finish_same_hash_skips_persist(state: TestState, session: Session, root: Uuid) {
        let bytes = format!("same-{}", Uuid::new_v4()).into_bytes();
        let original = test::upload(state, &session.user, *root, "notes.txt", &bytes).await;
        let before = test::blob_count(state.storage_dir());

        overwrite(state, session, *root, original.id, &bytes).await;

        let updated = state
            .services
            .files
            .get(&ctx(state, &session.user, *root).await, original.id)
            .await
            .unwrap();

        assert_eq!(updated.hash, original.hash);
        assert_eq!(updated.size_bytes, original.size_bytes);
        assert_eq!(test::blob_count(state.storage_dir()), before);
    }

    /// Quota independently blocks finish when new size would exceed it.
    #[test]
    async fn finish_quota_exceeded(state: TestState) {
        let ada = user(state, &format!("ow-quota-{}", Uuid::new_v4())).await;
        let session = test::login(state, &ada.username, "temporary").await;

        let root = state
            .services
            .users
            .create_shared(CreateSharedFolder {
                name: format!("ow-quota-{}", Uuid::new_v4()),
                quota_bytes: 10,
                members: vec![ada.id.into()],
            })
            .await
            .unwrap()
            .id;

        let original = test::upload(state, &session.user, root, "notes.txt", b"hi").await;

        let (existing, mut writer) = state
            .services
            .files
            .start_overwrite(&ctx(state, &session.user, root).await, original.id)
            .await
            .unwrap();
        writer.write(b"hello world!!").await.unwrap();
        let temp = writer.finish().await.unwrap();

        let err = state
            .services
            .files
            .finish_overwrite(&ctx(state, &session.user, root).await, existing, temp)
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::QuotaExceeded));
    }

    /// Overwrite replaces the blob and updates the live row.
    #[test]
    async fn replaces_blob_and_updates_row(state: TestState, session: Session, root: Uuid) {
        // Unique bytes so no earlier suite test still references this hash.
        let original_bytes = format!("ow-old-{}", Uuid::new_v4()).into_bytes();
        let next_bytes = format!("ow-new-{}", Uuid::new_v4()).into_bytes();
        let original =
            test::upload(state, &session.user, *root, "notes.txt", &original_bytes).await;
        let before = test::blob_count(state.storage_dir());

        overwrite(state, session, *root, original.id, &next_bytes).await;

        let updated = state
            .services
            .files
            .get(&ctx(state, &session.user, *root).await, original.id)
            .await
            .unwrap();

        assert_eq!(updated.id, original.id);
        assert_eq!(updated.name, "notes.txt");
        assert_eq!(updated.size_bytes, next_bytes.len() as i64);
        assert_ne!(updated.hash, original.hash);

        let (_name, body) = state
            .services
            .files
            .content_for_download(&ctx(state, &session.user, *root).await, updated.id)
            .await
            .unwrap();

        let crate::file::service::DownloadBody::File { path, mime } = body else {
            panic!("expected file download body");
        };
        assert_eq!(mime, "text/plain; charset=utf-8");
        assert_eq!(tokio::fs::read(&path).await.unwrap(), next_bytes);
        // Old unique blob removed; new one added → count unchanged.
        assert_eq!(test::blob_count(state.storage_dir()), before);
    }

    /// Finish with a file from another root independently fails.
    #[test]
    async fn finish_rejects_wrong_root(state: TestState, session: Session, root: Uuid) {
        let other = shared_root(
            state,
            &format!("ow-other-{}", Uuid::new_v4()),
            &[session.user.id],
        )
        .await;
        let file = test::upload(state, &session.user, *root, "notes.txt", b"hello").await;

        let (existing, mut writer) = state
            .services
            .files
            .start_overwrite(&ctx(state, &session.user, *root).await, file.id)
            .await
            .unwrap();
        writer.write(b"changed").await.unwrap();
        let temp = writer.finish().await.unwrap();

        let err = state
            .services
            .files
            .finish_overwrite(&ctx(state, &session.user, other).await, existing, temp)
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::ResourceMissing(_)));
    }
}
