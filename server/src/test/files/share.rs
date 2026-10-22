#[suitest::suite(files_share)]
#[suitest::suite_cfg(sequential = true)]
mod share {
    use super::super::{ctx, upload_in, user};
    use crate::{
        auth::session::Session,
        error::AppError,
        file::{
            model::{CreateDirectory, FileKind, ShareFiles, SharingMode, UnshareFiles},
            service::DownloadBody,
        },
        test::{self, TestState},
    };
    use chrono::{Duration, Utc};
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
        let username = format!("share-{}", Uuid::new_v4());
        user(state, &username).await;
        let session = test::login(state, &username, "temporary").await;
        let root = test::private_root(state, &session.user).await;
        (session, root)
    }

    /// Empty `ids` independently fails validate before share logic runs.
    #[test]
    async fn share_rejects_empty_ids(state: TestState, session: Session, root: Uuid) {
        let err = state
            .services
            .files
            .share_files(
                &ctx(state, &session.user, *root).await,
                ShareFiles {
                    ids: vec![],
                    sharing_mode: SharingMode::Download,
                    share_expires_at: None,
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Validation(_)));
    }

    /// Past `share_expires_at` independently fails before the share layer updates rows.
    #[test]
    async fn share_rejects_past_expiry(state: TestState, session: Session, root: Uuid) {
        let file = upload_in(state, &session.user, *root, None, "notes.txt", b"hello").await;

        let err = state
            .services
            .files
            .share_files(
                &ctx(state, &session.user, *root).await,
                ShareFiles {
                    ids: vec![file.id],
                    sharing_mode: SharingMode::Download,
                    share_expires_at: Some(Utc::now() - Duration::hours(1)),
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Unprocessable(_)));
    }

    /// Empty `ids` independently fails validate before unshare logic runs.
    #[test]
    async fn unshare_rejects_empty_ids(state: TestState, session: Session, root: Uuid) {
        let err = state
            .services
            .files
            .unshare_files(
                &ctx(state, &session.user, *root).await,
                UnshareFiles { ids: vec![] },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Validation(_)));
    }

    /// Download sharing mode independently enables public metadata and file download.
    #[test]
    async fn download_share_marks_file_and_allows_public_get(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let file = upload_in(state, &session.user, *root, None, "notes.txt", b"hello").await;

        assert!(file.sharing_mode.is_none());

        state
            .services
            .files
            .share_files(
                &ctx(state, &session.user, *root).await,
                ShareFiles {
                    ids: vec![file.id],
                    sharing_mode: SharingMode::Download,
                    share_expires_at: None,
                },
            )
            .await
            .unwrap();

        let shared = state.services.share.get(file.id).await.unwrap();
        assert_eq!(shared.file.sharing_mode, SharingMode::Download);
        assert!(shared.file.share_expires_at.is_none());
        assert_eq!(shared.file.name, "notes.txt");

        let (name, body) = state.services.share.download(file.id).await.unwrap();
        assert_eq!(name, "notes.txt");
        let DownloadBody::File { path, mime } = body else {
            panic!("expected file download body");
        };
        assert!(path.exists());
        assert!(mime.starts_with("text/plain"));
    }

    /// View sharing mode independently allows content reads but denies download.
    #[test]
    async fn view_share_allows_content_but_denies_download(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let file = upload_in(state, &session.user, *root, None, "notes.txt", b"hello").await;

        state
            .services
            .files
            .share_files(
                &ctx(state, &session.user, *root).await,
                ShareFiles {
                    ids: vec![file.id],
                    sharing_mode: SharingMode::View,
                    share_expires_at: None,
                },
            )
            .await
            .unwrap();

        let shared = state.services.share.get(file.id).await.unwrap();
        assert_eq!(shared.file.sharing_mode, SharingMode::View);

        let (entry, path, mime) = state.services.share.content(file.id).await.unwrap();
        assert_eq!(entry.id, file.id);
        assert!(path.exists());
        assert!(mime.starts_with("text/plain"));

        let download_denied = state.services.share.download(file.id).await.unwrap_err();
        assert!(matches!(download_denied, AppError::Forbidden(_)));
    }

    /// Expired share timestamp independently hides the file from public get.
    #[test]
    async fn expired_share_is_not_publicly_readable(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let file = upload_in(state, &session.user, *root, None, "old.txt", b"bye").await;

        state
            .services
            .files
            .share_files(
                &ctx(state, &session.user, *root).await,
                ShareFiles {
                    ids: vec![file.id],
                    sharing_mode: SharingMode::Download,
                    share_expires_at: None,
                },
            )
            .await
            .unwrap();

        assert!(state.services.share.get(file.id).await.is_ok());

        let past = Utc::now() - Duration::hours(1);
        sqlx::query!(
            r#"
            UPDATE files
            SET share_expires_at = $1
            WHERE id = $2
            "#,
            past,
            file.id
        )
        .execute(&state.pool)
        .await
        .unwrap();

        let missing = state.services.share.get(file.id).await.unwrap_err();
        assert!(matches!(missing, AppError::ResourceMissing(_)));
    }

    /// Unshare independently clears public access for a previously shared file.
    #[test]
    async fn unshare_clears_public_access(state: TestState, session: Session, root: Uuid) {
        let file = upload_in(state, &session.user, *root, None, "secret.txt", b"nope").await;

        let missing = state.services.share.get(file.id).await.unwrap_err();
        assert!(matches!(missing, AppError::ResourceMissing(_)));

        state
            .services
            .files
            .share_files(
                &ctx(state, &session.user, *root).await,
                ShareFiles {
                    ids: vec![file.id],
                    sharing_mode: SharingMode::Download,
                    share_expires_at: None,
                },
            )
            .await
            .unwrap();

        state
            .services
            .files
            .unshare_files(
                &ctx(state, &session.user, *root).await,
                UnshareFiles { ids: vec![file.id] },
            )
            .await
            .unwrap();

        let missing = state.services.share.get(file.id).await.unwrap_err();
        assert!(matches!(missing, AppError::ResourceMissing(_)));
    }

    /// Sharing a directory independently cascades mode and expiry to descendants.
    #[test]
    async fn sharing_directory_cascades_to_descendants(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let dir = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
                CreateDirectory {
                    name: "docs".into(),
                    parent: None,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let nested = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
                CreateDirectory {
                    name: "nested".into(),
                    parent: Some(dir.id),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let leaf = upload_in(
            state,
            &session.user,
            *root,
            Some(nested.id),
            "leaf.txt",
            b"leaf",
        )
        .await;

        let expires_at = Utc::now() + Duration::hours(24);

        state
            .services
            .files
            .share_files(
                &ctx(state, &session.user, *root).await,
                ShareFiles {
                    ids: vec![dir.id],
                    sharing_mode: SharingMode::View,
                    share_expires_at: Some(expires_at),
                },
            )
            .await
            .unwrap();

        let shared_dir = state.services.share.get(dir.id).await.unwrap();
        assert_eq!(shared_dir.file.sharing_mode, SharingMode::View);
        assert!(shared_dir.file.share_expires_at.is_some());

        assert_eq!(
            state
                .services
                .share
                .get(nested.id)
                .await
                .unwrap()
                .file
                .sharing_mode,
            SharingMode::View
        );
        assert_eq!(
            state
                .services
                .share
                .get(leaf.id)
                .await
                .unwrap()
                .file
                .sharing_mode,
            SharingMode::View
        );

        let children = state.services.share.list(dir.id).await.unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].file.id, nested.id);
        assert_eq!(children[0].file.kind, FileKind::Directory);

        let nested_children = state.services.share.list(nested.id).await.unwrap();
        assert_eq!(nested_children.len(), 1);
        assert_eq!(nested_children[0].file.id, leaf.id);

        state
            .services
            .files
            .unshare_files(
                &ctx(state, &session.user, *root).await,
                UnshareFiles { ids: vec![dir.id] },
            )
            .await
            .unwrap();

        assert!(matches!(
            state.services.share.get(dir.id).await.unwrap_err(),
            AppError::ResourceMissing(_)
        ));
        assert!(matches!(
            state.services.share.get(leaf.id).await.unwrap_err(),
            AppError::ResourceMissing(_)
        ));
    }
}
