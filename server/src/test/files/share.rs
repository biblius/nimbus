#[suitest::suite(files_share)]
#[suitest::suite_cfg(sequential = true)]
mod sharing {
    use super::super::{upload_in, user};
    use crate::{
        error::AppError,
        file::model::{CreateDirectory, FileKind, ShareFiles, SharingMode, UnshareFiles},
        test::{self, TestState},
    };
    use chrono::{Duration, Utc};
    use suitest::before_all;
    use uuid::Uuid;

    fn share_download(id: Uuid) -> ShareFiles {
        ShareFiles {
            ids: vec![id],
            sharing_mode: SharingMode::Download,
            share_expires_at: None,
        }
    }

    fn share_view(id: Uuid) -> ShareFiles {
        ShareFiles {
            ids: vec![id],
            sharing_mode: SharingMode::View,
            share_expires_at: None,
        }
    }

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
    }

    #[test]
    async fn share_marks_file_and_allows_public_get(state: TestState) {
        let _ada = user(state, "ada-share-file").await;
        let session = test::login(state, "ada-share-file", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let file = upload_in(state, &session.user, root, None, "notes.txt", b"hello").await;

        assert!(file.sharing_mode.is_none());

        state
            .services
            .files
            .share_files(&session.user, root, share_download(file.id))
            .await
            .unwrap();

        let shared = state.services.share.get(file.id).await.unwrap();
        assert_eq!(shared.file.sharing_mode, SharingMode::Download);
        assert!(shared.file.share_expires_at.is_none());
        assert_eq!(shared.file.name, "notes.txt");

        let (name, body) = state.services.share.download(file.id).await.unwrap();
        assert_eq!(name, "notes.txt");
        let crate::file::service::DownloadBody::File { path, mime } = body else {
            panic!("expected file download body");
        };
        assert!(path.exists());
        assert!(mime.starts_with("text/plain"));
    }

    #[test]
    async fn view_share_allows_content_but_denies_download(state: TestState) {
        let _ada = user(state, "ada-share-view").await;
        let session = test::login(state, "ada-share-view", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let file = upload_in(state, &session.user, root, None, "notes.txt", b"hello").await;

        state
            .services
            .files
            .share_files(&session.user, root, share_view(file.id))
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

    #[test]
    async fn expired_share_is_not_publicly_readable(state: TestState) {
        let _ada = user(state, "ada-share-expired").await;
        let session = test::login(state, "ada-share-expired", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let file = upload_in(state, &session.user, root, None, "old.txt", b"bye").await;

        state
            .services
            .files
            .share_files(&session.user, root, share_download(file.id))
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

    #[test]
    async fn unshared_file_is_not_publicly_readable(state: TestState) {
        let _ada = user(state, "ada-unshared").await;
        let session = test::login(state, "ada-unshared", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let file = upload_in(state, &session.user, root, None, "secret.txt", b"nope").await;

        let missing = state.services.share.get(file.id).await.unwrap_err();
        assert!(matches!(missing, AppError::ResourceMissing(_)));

        state
            .services
            .files
            .share_files(&session.user, root, share_download(file.id))
            .await
            .unwrap();

        state
            .services
            .files
            .unshare_files(
                &session.user,
                root,
                UnshareFiles {
                    ids: vec![file.id],
                },
            )
            .await
            .unwrap();

        let missing = state.services.share.get(file.id).await.unwrap_err();
        assert!(matches!(missing, AppError::ResourceMissing(_)));
    }

    #[test]
    async fn sharing_directory_cascades_to_descendants(state: TestState) {
        let _ada = user(state, "ada-share-dir").await;
        let session = test::login(state, "ada-share-dir", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let dir = state
            .services
            .files
            .create_directory(
                &session.user,
                root,
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
                &session.user,
                root,
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
            root,
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
                &session.user,
                root,
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
                &session.user,
                root,
                UnshareFiles {
                    ids: vec![dir.id],
                },
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

    #[test]
    async fn upload_into_shared_parent_inherits_shared(state: TestState) {
        let _ada = user(state, "ada-share-inherit").await;
        let session = test::login(state, "ada-share-inherit", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let dir = state
            .services
            .files
            .create_directory(
                &session.user,
                root,
                CreateDirectory {
                    name: "public".into(),
                    parent: None,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let expires_at = Utc::now() + Duration::days(7);

        state
            .services
            .files
            .share_files(
                &session.user,
                root,
                ShareFiles {
                    ids: vec![dir.id],
                    sharing_mode: SharingMode::Download,
                    share_expires_at: Some(expires_at),
                },
            )
            .await
            .unwrap();

        let nested = state
            .services
            .files
            .create_directory(
                &session.user,
                root,
                CreateDirectory {
                    name: "nested".into(),
                    parent: Some(dir.id),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(nested.sharing_mode, Some(SharingMode::Download));
        assert!(nested.share_expires_at.is_some());

        let file = upload_in(
            state,
            &session.user,
            root,
            Some(dir.id),
            "new.txt",
            b"fresh",
        )
        .await;
        assert_eq!(file.sharing_mode, Some(SharingMode::Download));
        assert!(file.share_expires_at.is_some());

        let shared = state.services.share.get(file.id).await.unwrap();
        assert_eq!(shared.file.name, "new.txt");

        let nested_shared = state.services.share.get(nested.id).await.unwrap();
        assert_eq!(nested_shared.file.kind, FileKind::Directory);
    }

    #[test]
    async fn foreign_user_cannot_share_private_file(state: TestState) {
        let _ada = user(state, "ada-owner-share").await;
        let owner = test::login(state, "ada-owner-share", "temporary").await;
        let root = test::private_root(state, &owner.user).await;

        let file = upload_in(state, &owner.user, root, None, "mine.txt", b"mine").await;

        let _bob = user(state, "bob-share-denied").await;
        let bob = test::login(state, "bob-share-denied", "temporary").await;

        let denied = state
            .services
            .files
            .share_files(&bob.user, root, share_download(file.id))
            .await
            .unwrap_err();

        assert!(matches!(denied, AppError::Forbidden(_)));
        assert!(matches!(
            state.services.share.get(file.id).await.unwrap_err(),
            AppError::ResourceMissing(_)
        ));
    }
}
