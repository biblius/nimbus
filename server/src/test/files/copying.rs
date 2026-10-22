#[suitest::suite(files_copy)]
#[suitest::suite_cfg(sequential = true)]
mod copying {
    use super::super::{ctx, shared_root, upload_in, user};
    use crate::{
        auth::session::Session,
        error::AppError,
        file::model::{CopyFiles, CreateDirectory, FileKind},
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
        let username = format!("copy-{}", Uuid::new_v4());
        user(state, &username).await;
        let session = test::login(state, &username, "temporary").await;
        let root = test::private_root(state, &session.user).await;
        (session, root)
    }

    /// Empty `ids` independently fails validate before any copy logic runs.
    #[test]
    async fn copy_rejects_empty_ids(state: TestState, session: Session, root: Uuid) {
        let err = state
            .services
            .files
            .copy_files(
                &ctx(state, &session.user, *root).await,
                CopyFiles {
                    ids: vec![],
                    target_root_id: *root,
                    parent: None,
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Validation(_)));
    }

    /// Missing target parent independently yields Unprocessable on the destination root.
    #[test]
    async fn copy_target_parent_missing(state: TestState, session: Session, root: Uuid) {
        let file = test::upload(state, &session.user, *root, "notes.txt", b"notes").await;

        let err = state
            .services
            .files
            .copy_files(
                &ctx(state, &session.user, *root).await,
                CopyFiles {
                    ids: vec![file.id],
                    target_root_id: *root,
                    parent: Some(Uuid::new_v4()),
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Unprocessable(_)));
    }

    /// Target parent that is not a directory independently yields Unprocessable.
    #[test]
    async fn copy_target_parent_not_directory(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let not_a_dir = test::upload(state, &session.user, *root, "leaf.txt", b"leaf").await;
        let file = test::upload(state, &session.user, *root, "notes.txt", b"notes").await;

        let err = state
            .services
            .files
            .copy_files(
                &ctx(state, &session.user, *root).await,
                CopyFiles {
                    ids: vec![file.id],
                    target_root_id: *root,
                    parent: Some(not_a_dir.id),
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Unprocessable(_)));
    }

    /// Copy into another root assigns a new id, root, and created_at; source row is unchanged.
    #[test]
    async fn copies_file_into_another_root_with_new_id_and_created_at(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let shared = shared_root(
            state,
            &format!("copy-xroot-{}", Uuid::new_v4()),
            &[session.user.id],
        )
        .await;
        let source = test::upload(state, &session.user, shared, "notes.txt", b"notes").await;

        state
            .services
            .files
            .copy_files(
                &ctx(state, &session.user, shared).await,
                CopyFiles {
                    ids: vec![source.id],
                    target_root_id: *root,
                    parent: None,
                },
            )
            .await
            .unwrap();

        let listed = state
            .services
            .files
            .list(&ctx(state, &session.user, *root).await, None)
            .await
            .unwrap();
        let copied = listed
            .into_iter()
            .find(|entry| entry.file.name == "notes.txt")
            .expect("copied file");

        assert_ne!(copied.file.id, source.id);
        assert_eq!(copied.file.root_id, *root);
        assert_eq!(copied.file.parent, None);
        assert_eq!(copied.file.hash, source.hash);
        assert_eq!(copied.file.size_bytes, source.size_bytes);
        assert_eq!(copied.file.file_type, source.file_type);
        assert_eq!(copied.file.created_by, source.created_by);
        assert!(copied.metadata.is_none());
        assert!(copied.file.created_at >= source.created_at);

        let original = state
            .services
            .files
            .get(&ctx(state, &session.user, shared).await, source.id)
            .await
            .unwrap();
        assert_eq!(original.root_id, shared);
    }

    /// Multiple ids in one request each produce a distinct copy in the target root.
    #[test]
    async fn copies_multiple_files_in_one_request(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let shared = shared_root(
            state,
            &format!("copy-many-{}", Uuid::new_v4()),
            &[session.user.id],
        )
        .await;
        let first = test::upload(state, &session.user, shared, "a.txt", b"a").await;
        let second = test::upload(state, &session.user, shared, "b.txt", b"b").await;

        state
            .services
            .files
            .copy_files(
                &ctx(state, &session.user, shared).await,
                CopyFiles {
                    ids: vec![first.id, second.id],
                    target_root_id: *root,
                    parent: None,
                },
            )
            .await
            .unwrap();

        let listed = state
            .services
            .files
            .list(&ctx(state, &session.user, *root).await, None)
            .await
            .unwrap();
        let names: Vec<_> = listed
            .iter()
            .map(|entry| entry.file.name.as_str())
            .collect();
        assert!(names.contains(&"a.txt"));
        assert!(names.contains(&"b.txt"));
        assert_eq!(listed.len(), 2);
        assert!(
            listed
                .iter()
                .all(|entry| entry.file.id != first.id && entry.file.id != second.id)
        );
    }

    /// Copying a directory includes descendant files under remapped ids.
    #[test]
    async fn copies_directory_with_descendants(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let shared = shared_root(
            state,
            &format!("copy-tree-{}", Uuid::new_v4()),
            &[session.user.id],
        )
        .await;
        let folder = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, shared).await,
                CreateDirectory {
                    parent: None,
                    name: "photos".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let nested = upload_in(
            state,
            &session.user,
            shared,
            Some(folder.id),
            "inside.txt",
            b"inside",
        )
        .await;

        state
            .services
            .files
            .copy_files(
                &ctx(state, &session.user, shared).await,
                CopyFiles {
                    ids: vec![folder.id],
                    target_root_id: *root,
                    parent: None,
                },
            )
            .await
            .unwrap();

        let listed = state
            .services
            .files
            .list(&ctx(state, &session.user, *root).await, None)
            .await
            .unwrap();
        let copied_folder = listed
            .into_iter()
            .find(|entry| entry.file.name == "photos")
            .expect("copied folder");
        assert_eq!(copied_folder.file.kind, FileKind::Directory);
        assert_ne!(copied_folder.file.id, folder.id);

        let children = state
            .services
            .files
            .list(
                &ctx(state, &session.user, *root).await,
                Some(copied_folder.file.id),
            )
            .await
            .unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].file.name, "inside.txt");
        assert_ne!(children[0].file.id, nested.id);
        assert_eq!(children[0].file.hash, nested.hash);
    }

    /// Overlapping selection (folder + child) nests the child under the copied folder once.
    #[test]
    async fn overlapping_selection_nests_child_under_copied_folder(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let shared = shared_root(
            state,
            &format!("copy-overlap-{}", Uuid::new_v4()),
            &[session.user.id],
        )
        .await;
        let folder = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, shared).await,
                CreateDirectory {
                    parent: None,
                    name: "docs".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let nested = upload_in(
            state,
            &session.user,
            shared,
            Some(folder.id),
            "note.txt",
            b"note",
        )
        .await;

        state
            .services
            .files
            .copy_files(
                &ctx(state, &session.user, shared).await,
                CopyFiles {
                    ids: vec![folder.id, nested.id],
                    target_root_id: *root,
                    parent: None,
                },
            )
            .await
            .unwrap();

        let listed = state
            .services
            .files
            .list(&ctx(state, &session.user, *root).await, None)
            .await
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].file.name, "docs");

        let children = state
            .services
            .files
            .list(
                &ctx(state, &session.user, *root).await,
                Some(listed[0].file.id),
            )
            .await
            .unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].file.name, "note.txt");
    }

    /// `parent` on the target root places copies under that directory.
    #[test]
    async fn copies_into_a_target_directory(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let destination = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
                CreateDirectory {
                    parent: None,
                    name: "archive".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let file = test::upload(state, &session.user, *root, "memo.txt", b"memo").await;

        state
            .services
            .files
            .copy_files(
                &ctx(state, &session.user, *root).await,
                CopyFiles {
                    ids: vec![file.id],
                    target_root_id: *root,
                    parent: Some(destination.id),
                },
            )
            .await
            .unwrap();

        let children = state
            .services
            .files
            .list(
                &ctx(state, &session.user, *root).await,
                Some(destination.id),
            )
            .await
            .unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].file.name, "memo.txt");
        assert_ne!(children[0].file.id, file.id);
    }

    /// Target root quota independently blocks copy when size would exceed limit.
    #[test]
    async fn enforces_quota_on_the_target_root(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let tiny = state
            .services
            .users
            .create_shared(CreateSharedFolder {
                name: format!("tiny-{}", Uuid::new_v4()),
                quota_bytes: 4,
                members: vec![session.user.id.into()],
            })
            .await
            .unwrap()
            .id;

        let file = test::upload(state, &session.user, *root, "big.txt", b"12345").await;

        let err = state
            .services
            .files
            .copy_files(
                &ctx(state, &session.user, *root).await,
                CopyFiles {
                    ids: vec![file.id],
                    target_root_id: tiny,
                    parent: None,
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::QuotaExceeded));
    }
}
