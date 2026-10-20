#[suitest::suite(files_access)]
#[suitest::suite_cfg(sequential = true)]
mod access {
    use super::super::{assert_forbidden, ctx, grant, shared_root, upload_in, user};
    use crate::{
        error::AppError,
        file::{
            AccessMode,
            model::{
                CopyFiles, CreateDirectory, CreateTextFile, DownloadFiles, MoveFiles, PreviewSize,
                RenameFile, RestoreFile, ShareFiles, SharingMode, TrashFiles, UnshareFiles,
            },
        },
        test::{self, TestState},
        user::model::{CreateUser, Role, User},
    };
    use suitest::before_all;
    use uuid::Uuid;

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
    }

    async fn admin(state: &TestState, username: &str) -> crate::user::model::UserView {
        state
            .services
            .users
            .create_user(CreateUser {
                username: username.into(),
                password: "temporary".into(),
                email: None,
                role: Role::Admin,
            })
            .await
            .unwrap()
    }

    /// Writer (DEFAULT) seeds content; subject gets `mode` on the same shared root.
    async fn fixture(
        state: &TestState,
        label: &str,
        mode: AccessMode,
    ) -> (
        User,
        User,
        Uuid,
        crate::file::model::FileEntry,
        crate::file::model::FileEntry,
    ) {
        let writer = user(state, &format!("{label}-writer")).await;
        let subject = user(state, &format!("{label}-subject")).await;
        let writer_session = test::login(state, &format!("{label}-writer"), "temporary").await;
        let subject_session = test::login(state, &format!("{label}-subject"), "temporary").await;

        let root = shared_root(state, &format!("{label}-root"), &[writer.id]).await;
        grant(state, root, subject.id, mode).await;

        let dir = state
            .services
            .files
            .create_directory(
                &ctx(state, &writer_session.user, root).await,
                CreateDirectory {
                    parent: None,
                    name: "docs".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let file = upload_in(
            state,
            &writer_session.user,
            root,
            Some(dir.id),
            "notes.txt",
            b"hello",
        )
        .await;

        (writer_session.user, subject_session.user, root, file, dir)
    }

    #[test]
    async fn non_member_cannot_resolve_context(state: TestState) {
        let ada = user(state, "ada-access-none").await;
        let _bob = user(state, "bob-access-none").await;
        let root = shared_root(state, "locked", &[ada.id]).await;

        let bob = test::login(state, "bob-access-none", "temporary").await;
        assert_forbidden(state.services.files.accessible_root(&bob.user, root).await);
    }

    #[test]
    async fn private_owner_has_full_access(state: TestState) {
        let _ada = user(state, "ada-access-private").await;
        let session = test::login(state, "ada-access-private", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let ctx = ctx(state, &session.user, root).await;
        assert_eq!(ctx.access, AccessMode::FULL);
        ctx.require(AccessMode::WRITE).unwrap();
        ctx.require(AccessMode::SHARE).unwrap();
    }

    #[test]
    async fn admin_has_full_access_without_membership(state: TestState) {
        let ada = user(state, "ada-access-admin-share").await;
        let _admin = admin(state, "admin-access-full").await;
        let root = shared_root(state, "admin-visible", &[ada.id]).await;

        let admin_session = test::login(state, "admin-access-full", "temporary").await;
        let ctx = ctx(state, &admin_session.user, root).await;
        assert_eq!(ctx.access, AccessMode::FULL);
    }

    #[test]
    async fn view_only_can_read_but_not_write_or_share(state: TestState) {
        let (_writer, subject, root, file, dir) = fixture(state, "view", AccessMode::empty()).await;
        let ctx = ctx(state, &subject, root).await;
        assert_eq!(ctx.access, AccessMode::empty());

        // View (implicit)
        state.services.files.usage(&ctx).await.unwrap();
        state.services.files.list(&ctx, None).await.unwrap();
        state.services.files.list(&ctx, Some(dir.id)).await.unwrap();
        state.services.files.ancestors(&ctx, file.id).await.unwrap();
        state.services.files.metadata(&ctx, file.id).await.unwrap();
        state
            .services
            .files
            .content_for_stream(&ctx, file.id)
            .await
            .unwrap();
        state
            .services
            .files
            .content_for_download(&ctx, file.id)
            .await
            .unwrap();
        state
            .services
            .files
            .content_for_root_download(&ctx)
            .await
            .unwrap();
        state
            .services
            .files
            .download_dir(&ctx, DownloadFiles { ids: vec![file.id] })
            .await
            .unwrap();
        state.services.files.get_tree(&ctx, None).await.unwrap();
        state.services.trash.get_trash_tree(&ctx).await.unwrap();
        // No preview blob yet — must not be Forbidden.
        assert!(matches!(
            state
                .services
                .files
                .preview(&ctx, file.id, PreviewSize::Icon)
                .await,
            Err(AppError::ResourceMissing(_))
        ));

        // Write
        assert_forbidden(
            state
                .services
                .files
                .create_directory(
                    &ctx,
                    CreateDirectory {
                        parent: None,
                        name: "nope".into(),
                        ..Default::default()
                    },
                )
                .await,
        );
        assert_forbidden(
            state
                .services
                .files
                .create_text_file(
                    &ctx,
                    CreateTextFile {
                        parent: None,
                        name: "nope.txt".into(),
                    },
                )
                .await,
        );
        assert_forbidden(
            state
                .services
                .files
                .finish_upload(&ctx, None, vec![], vec![])
                .await,
        );
        assert_forbidden(state.services.files.start_overwrite(&ctx, file.id).await);
        assert_forbidden(state.services.files.delete(&ctx, file.id).await);
        assert_forbidden(
            state
                .services
                .files
                .delete_many(&ctx, TrashFiles { ids: vec![file.id] })
                .await,
        );
        assert_forbidden(
            state
                .services
                .files
                .rename(
                    &ctx,
                    file.id,
                    RenameFile {
                        name: "renamed.txt".into(),
                    },
                )
                .await,
        );
        assert_forbidden(
            state
                .services
                .files
                .move_files(
                    &ctx,
                    MoveFiles {
                        ids: vec![file.id],
                        parent: None,
                    },
                )
                .await,
        );
        assert_forbidden(
            state
                .services
                .trash
                .restore(&ctx, file.id, RestoreFile { parent: None })
                .await,
        );
        assert_forbidden(
            state
                .services
                .trash
                .restore_all(&ctx, RestoreFile { parent: None })
                .await,
        );
        assert_forbidden(
            state
                .services
                .trash
                .delete_trash(&ctx, file.id, state.queue.clone())
                .await,
        );

        // Share
        assert_forbidden(
            state
                .services
                .files
                .share_files(
                    &ctx,
                    ShareFiles {
                        ids: vec![file.id],
                        sharing_mode: SharingMode::Download,
                        share_expires_at: None,
                    },
                )
                .await,
        );
        assert_forbidden(
            state
                .services
                .files
                .unshare_files(&ctx, UnshareFiles { ids: vec![file.id] })
                .await,
        );
    }

    #[test]
    async fn write_only_can_mutate_but_not_share(state: TestState) {
        let (_writer, subject, root, file, dir) = fixture(state, "write", AccessMode::WRITE).await;
        let ctx = ctx(state, &subject, root).await;
        assert_eq!(ctx.access, AccessMode::WRITE);

        // Read still works
        state.services.files.metadata(&ctx, file.id).await.unwrap();

        let created = state
            .services
            .files
            .create_directory(
                &ctx,
                CreateDirectory {
                    parent: Some(dir.id),
                    name: "inbox".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(created.parent, Some(dir.id));

        let text = state
            .services
            .files
            .create_text_file(
                &ctx,
                CreateTextFile {
                    parent: Some(dir.id),
                    name: "draft.txt".into(),
                },
            )
            .await
            .unwrap();
        assert_eq!(text.name, "draft.txt");

        let uploaded = upload_in(state, &subject, root, Some(dir.id), "up.txt", b"data").await;
        assert_eq!(uploaded.name, "up.txt");

        let (existing, mut writer) = state
            .services
            .files
            .start_overwrite(&ctx, text.id)
            .await
            .unwrap();
        writer.write(b"updated").await.unwrap();
        let temp = writer.finish().await.unwrap();
        state
            .services
            .files
            .finish_overwrite(&ctx, existing, temp)
            .await
            .unwrap();

        state
            .services
            .files
            .rename(
                &ctx,
                uploaded.id,
                RenameFile {
                    name: "uploaded.txt".into(),
                },
            )
            .await
            .unwrap();

        state
            .services
            .files
            .move_files(
                &ctx,
                MoveFiles {
                    ids: vec![uploaded.id],
                    parent: Some(created.id),
                },
            )
            .await
            .unwrap();

        state
            .services
            .files
            .delete_many(
                &ctx,
                TrashFiles {
                    ids: vec![uploaded.id],
                },
            )
            .await
            .unwrap();

        state
            .services
            .trash
            .restore(
                &ctx,
                uploaded.id,
                RestoreFile {
                    parent: Some(dir.id),
                },
            )
            .await
            .unwrap();

        state
            .services
            .files
            .delete(&ctx, uploaded.id)
            .await
            .unwrap();

        let rx = state
            .services
            .trash
            .delete_trash(&ctx, uploaded.id, state.queue.clone())
            .await
            .unwrap();
        state.await_deletion(rx).await;

        // Share denied
        assert_forbidden(
            state
                .services
                .files
                .share_files(
                    &ctx,
                    ShareFiles {
                        ids: vec![file.id],
                        sharing_mode: SharingMode::View,
                        share_expires_at: None,
                    },
                )
                .await,
        );
        assert_forbidden(
            state
                .services
                .files
                .unshare_files(&ctx, UnshareFiles { ids: vec![file.id] })
                .await,
        );
    }

    #[test]
    async fn write_only_can_restore_all(state: TestState) {
        let (writer, subject, root, file, _dir) =
            fixture(state, "restore-all", AccessMode::WRITE).await;

        state
            .services
            .files
            .delete(&ctx(state, &writer, root).await, file.id)
            .await
            .unwrap();

        let subject_ctx = ctx(state, &subject, root).await;
        state
            .services
            .trash
            .restore_all(&subject_ctx, RestoreFile { parent: None })
            .await
            .unwrap();

        state
            .services
            .files
            .metadata(&subject_ctx, file.id)
            .await
            .unwrap();
    }

    #[test]
    async fn share_only_can_share_but_not_write(state: TestState) {
        let (_writer, subject, root, file, _dir) = fixture(state, "share", AccessMode::SHARE).await;
        let ctx = ctx(state, &subject, root).await;
        assert_eq!(ctx.access, AccessMode::SHARE);

        state.services.files.metadata(&ctx, file.id).await.unwrap();

        state
            .services
            .files
            .share_files(
                &ctx,
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

        state
            .services
            .files
            .unshare_files(&ctx, UnshareFiles { ids: vec![file.id] })
            .await
            .unwrap();

        assert!(matches!(
            state.services.share.get(file.id).await,
            Err(AppError::ResourceMissing(_))
        ));

        assert_forbidden(
            state
                .services
                .files
                .create_directory(
                    &ctx,
                    CreateDirectory {
                        parent: None,
                        name: "blocked".into(),
                        ..Default::default()
                    },
                )
                .await,
        );
        assert_forbidden(state.services.files.delete(&ctx, file.id).await);
        assert_forbidden(
            state
                .services
                .files
                .rename(
                    &ctx,
                    file.id,
                    RenameFile {
                        name: "x.txt".into(),
                    },
                )
                .await,
        );
    }

    #[test]
    async fn full_member_can_write_and_share(state: TestState) {
        let mode = AccessMode::WRITE | AccessMode::SHARE;
        let (_writer, subject, root, file, _dir) = fixture(state, "full", mode).await;
        let ctx = ctx(state, &subject, root).await;
        assert_eq!(ctx.access, mode);

        state
            .services
            .files
            .create_directory(
                &ctx,
                CreateDirectory {
                    parent: None,
                    name: "ok".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        state
            .services
            .files
            .share_files(
                &ctx,
                ShareFiles {
                    ids: vec![file.id],
                    sharing_mode: SharingMode::View,
                    share_expires_at: None,
                },
            )
            .await
            .unwrap();

        state
            .services
            .files
            .unshare_files(&ctx, UnshareFiles { ids: vec![file.id] })
            .await
            .unwrap();
    }

    #[test]
    async fn copy_requires_write_on_target_only(state: TestState) {
        let writer = user(state, "ada-copy-access-w").await;
        let viewer = user(state, "bob-copy-access-v").await;
        let writer_session = test::login(state, "ada-copy-access-w", "temporary").await;
        let viewer_session = test::login(state, "bob-copy-access-v", "temporary").await;

        let source = shared_root(state, "copy-src", &[writer.id, viewer.id]).await;
        grant(state, source, viewer.id, AccessMode::empty()).await;

        let target_ok = shared_root(state, "copy-dst-ok", &[viewer.id]).await;
        grant(state, target_ok, viewer.id, AccessMode::WRITE).await;

        let target_ro = shared_root(state, "copy-dst-ro", &[viewer.id]).await;
        grant(state, target_ro, viewer.id, AccessMode::empty()).await;

        let file = test::upload(state, &writer_session.user, source, "memo.txt", b"copy-me").await;

        // View on source + write on target → allowed
        state
            .services
            .files
            .copy_files(
                &ctx(state, &viewer_session.user, source).await,
                CopyFiles {
                    ids: vec![file.id],
                    target_root_id: target_ok,
                    parent: None,
                },
            )
            .await
            .unwrap();

        let listed = state
            .services
            .files
            .list(&ctx(state, &viewer_session.user, target_ok).await, None)
            .await
            .unwrap();
        assert!(listed.iter().any(|entry| entry.file.name == "memo.txt"));

        // View on source + view on target → Forbidden
        assert_forbidden(
            state
                .services
                .files
                .copy_files(
                    &ctx(state, &viewer_session.user, source).await,
                    CopyFiles {
                        ids: vec![file.id],
                        target_root_id: target_ro,
                        parent: None,
                    },
                )
                .await,
        );
    }

    #[test]
    async fn missing_root_is_not_forbidden(state: TestState) {
        let _ada = user(state, "ada-access-missing").await;
        let session = test::login(state, "ada-access-missing", "temporary").await;
        let missing = state
            .services
            .files
            .accessible_root(&session.user, Uuid::new_v4())
            .await;
        assert!(matches!(missing, Err(AppError::ResourceMissing(_))));
    }
}
