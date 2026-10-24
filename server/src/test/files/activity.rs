#[suitest::suite(files_activity)]
#[suitest::suite_cfg(sequential = true)]
mod activity {
    use super::super::{ctx, upload_in, user};
    use crate::{
        activity::{Activity, ActivityData, ActivityTarget},
        auth::session::Session,
        file::model::{
            CreateDirectory, MoveFiles, RenameFile, RestoreFile, ShareFiles, SharingMode,
            TrashFiles,
        },
        model::{List, Pagination},
        test::{self, TestState},
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
        let username = format!("activity-{}", Uuid::new_v4());
        user(state, &username).await;
        let session = test::login(state, &username, "temporary").await;
        let root = test::private_root(state, &session.user).await;
        (session, root)
    }

    async fn wait_until<F, Fut>(timeout: std::time::Duration, mut check: F)
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if check().await {
                return;
            }
            if tokio::time::Instant::now() >= deadline {
                panic!("condition not met within {timeout:?}");
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }

    async fn list(
        state: &TestState,
        session: &Session,
        root: Uuid,
        page: usize,
        per_page: usize,
    ) -> List<Activity> {
        state
            .services
            .files
            .list_activities(
                &ctx(state, &session.user, root).await,
                Pagination::new(page, per_page),
            )
            .await
            .unwrap()
    }

    async fn wait_for_total(
        state: &TestState,
        session: &Session,
        root: Uuid,
        expected: usize,
    ) -> List<Activity> {
        wait_until(std::time::Duration::from_secs(5), || async {
            list(state, session, root, 1, expected.max(1) + 5)
                .await
                .total
                == expected
        })
        .await;
        list(state, session, root, 1, expected.max(1) + 5).await
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

    /// Empty root independently lists zero activities.
    #[test]
    async fn empty_root_lists_nothing(state: TestState, session: Session, root: Uuid) {
        let listed = list(state, session, *root, 1, 10).await;
        assert_eq!(listed.total, 0);
        assert!(listed.items.is_empty());
    }

    /// Uploading a single file independently records a Create(Single) activity.
    #[test]
    async fn upload_logs_create_single(state: TestState, session: Session, root: Uuid) {
        let file = test::upload(state, &session.user, *root, "notes.txt", b"notes").await;

        let listed = wait_for_total(state, session, *root, 1).await;
        assert_eq!(listed.items[0].username, session.user.username);
        assert_eq!(listed.items[0].root_id, *root);
        assert!(matches!(
            &listed.items[0].data,
            ActivityData::Create(ActivityTarget::Single { id, name })
                if *id == file.id && name == "notes.txt"
        ));
    }

    /// Multi-file upload independently records Create(Multiple).
    #[test]
    async fn upload_many_logs_create_multiple(state: TestState, session: Session, root: Uuid) {
        test::submit(
            state,
            &session.user,
            *root,
            vec![("a.txt", b"a"), ("b.txt", b"b")],
        )
        .await
        .into_iter()
        .for_each(|entry| {
            entry.unwrap();
        });

        let listed = wait_for_total(state, session, *root, 1).await;
        assert!(matches!(
            &listed.items[0].data,
            ActivityData::Create(ActivityTarget::Multiple { count: 2 })
        ));
    }

    /// Creating a directory independently records Create(Single).
    #[test]
    async fn create_directory_logs_create(state: TestState, session: Session, root: Uuid) {
        let folder = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
                CreateDirectory {
                    parent: None,
                    name: "photos".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let listed = wait_for_total(state, session, *root, 1).await;
        assert!(matches!(
            &listed.items[0].data,
            ActivityData::Create(ActivityTarget::Single { id, name })
                if *id == folder.id && name == "photos"
        ));
    }

    /// Renaming a live file independently records Rename with from/to.
    #[test]
    async fn rename_logs_rename(state: TestState, session: Session, root: Uuid) {
        let file = test::upload(state, &session.user, *root, "notes.txt", b"notes").await;
        wait_for_total(state, session, *root, 1).await;

        state
            .services
            .files
            .rename(
                &ctx(state, &session.user, *root).await,
                file.id,
                RenameFile {
                    name: "journal.txt".into(),
                },
            )
            .await
            .unwrap();

        let listed = wait_for_total(state, session, *root, 2).await;
        assert!(matches!(
            &listed.items[0].data,
            ActivityData::Rename { id, from, to }
                if *id == file.id && from == "notes.txt" && to == "journal.txt"
        ));
    }

    /// Same-name rename independently skips activity.
    #[test]
    async fn same_name_rename_skips_activity(state: TestState, session: Session, root: Uuid) {
        let file = test::upload(state, &session.user, *root, "notes.txt", b"notes").await;
        wait_for_total(state, session, *root, 1).await;

        state
            .services
            .files
            .rename(
                &ctx(state, &session.user, *root).await,
                file.id,
                RenameFile {
                    name: "notes.txt".into(),
                },
            )
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert_eq!(list(state, session, *root, 1, 10).await.total, 1);
    }

    /// Soft-deleting a live file independently records Trash(Single).
    #[test]
    async fn delete_logs_trash_single(state: TestState, session: Session, root: Uuid) {
        let file = test::upload(state, &session.user, *root, "notes.txt", b"notes").await;
        wait_for_total(state, session, *root, 1).await;

        state
            .services
            .files
            .delete(&ctx(state, &session.user, *root).await, file.id)
            .await
            .unwrap();

        let listed = wait_for_total(state, session, *root, 2).await;
        assert!(matches!(
            &listed.items[0].data,
            ActivityData::Trash(ActivityTarget::Single { id, name })
                if *id == file.id && name == "notes.txt"
        ));
    }

    /// Bulk trash independently records Trash(Multiple) even for one id.
    #[test]
    async fn delete_many_logs_trash_multiple(state: TestState, session: Session, root: Uuid) {
        let file = test::upload(state, &session.user, *root, "notes.txt", b"notes").await;
        wait_for_total(state, session, *root, 1).await;

        state
            .services
            .files
            .delete_many(
                &ctx(state, &session.user, *root).await,
                TrashFiles {
                    ids: vec![file.id],
                },
            )
            .await
            .unwrap();

        let listed = wait_for_total(state, session, *root, 2).await;
        assert!(matches!(
            &listed.items[0].data,
            ActivityData::Trash(ActivityTarget::Multiple { count: 1 })
        ));
    }

    /// Moving a file independently records Move with destination folder name.
    #[test]
    async fn move_logs_move_to_directory(state: TestState, session: Session, root: Uuid) {
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
        let file = test::upload(state, &session.user, *root, "notes.txt", b"notes").await;
        wait_for_total(state, session, *root, 2).await;

        state
            .services
            .files
            .move_files(
                &ctx(state, &session.user, *root).await,
                MoveFiles {
                    ids: vec![file.id],
                    parent: Some(destination.id),
                },
            )
            .await
            .unwrap();

        let listed = wait_for_total(state, session, *root, 3).await;
        assert!(matches!(
            &listed.items[0].data,
            ActivityData::Move {
                target: ActivityTarget::Single { id, name },
                to
            } if *id == file.id && name == "notes.txt" && to == "archive"
        ));
    }

    /// Sharing a file independently records Share(Single).
    #[test]
    async fn share_logs_share_single(state: TestState, session: Session, root: Uuid) {
        let file = upload_in(state, &session.user, *root, None, "notes.txt", b"hello").await;
        wait_for_total(state, session, *root, 1).await;

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

        let listed = wait_for_total(state, session, *root, 2).await;
        assert!(matches!(
            &listed.items[0].data,
            ActivityData::Share(ActivityTarget::Single { id, name })
                if *id == file.id && name == "notes.txt"
        ));
    }

    /// Restoring a trashed file independently records Restore with destination name.
    #[test]
    async fn restore_logs_restore(state: TestState, session: Session, root: Uuid) {
        let notes = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
                CreateDirectory {
                    parent: None,
                    name: "notes".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let inside = upload_in(
            state,
            &session.user,
            *root,
            Some(notes.id),
            "inside.txt",
            b"restore-inside",
        )
        .await;
        wait_for_total(state, session, *root, 2).await;

        state
            .services
            .files
            .delete(&ctx(state, &session.user, *root).await, inside.id)
            .await
            .unwrap();
        wait_for_total(state, session, *root, 3).await;

        state
            .services
            .trash
            .restore(
                &ctx(state, &session.user, *root).await,
                inside.id,
                RestoreFile {
                    parent: Some(notes.id),
                },
            )
            .await
            .unwrap();

        let listed = wait_for_total(state, session, *root, 4).await;
        assert!(matches!(
            &listed.items[0].data,
            ActivityData::Restore {
                target: ActivityTarget::Single { id, name },
                to
            } if *id == inside.id && name == "inside.txt" && to == "notes"
        ));
    }

    /// Overwriting text contents independently records Edit.
    #[test]
    async fn overwrite_logs_edit(state: TestState, session: Session, root: Uuid) {
        let original_bytes = format!("act-old-{}", Uuid::new_v4()).into_bytes();
        let next_bytes = format!("act-new-{}", Uuid::new_v4()).into_bytes();
        let file = test::upload(state, &session.user, *root, "notes.txt", &original_bytes).await;
        wait_for_total(state, session, *root, 1).await;

        overwrite(state, session, *root, file.id, &next_bytes).await;

        let listed = wait_for_total(state, session, *root, 2).await;
        assert!(matches!(
            &listed.items[0].data,
            ActivityData::Edit { id, name }
                if *id == file.id && name == "notes.txt"
        ));
    }

    /// Same-hash overwrite independently skips Edit activity.
    #[test]
    async fn same_hash_overwrite_skips_activity(state: TestState, session: Session, root: Uuid) {
        let bytes = format!("act-same-{}", Uuid::new_v4()).into_bytes();
        let file = test::upload(state, &session.user, *root, "notes.txt", &bytes).await;
        wait_for_total(state, session, *root, 1).await;

        overwrite(state, session, *root, file.id, &bytes).await;

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert_eq!(list(state, session, *root, 1, 10).await.total, 1);
    }

    /// Listing pages newest-first and keeps the full total across pages.
    #[test]
    async fn pages_newest_first_and_keep_full_total(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        for name in ["a", "b", "c"] {
            state
                .services
                .files
                .create_directory(
                    &ctx(state, &session.user, *root).await,
                    CreateDirectory {
                        parent: None,
                        name: name.into(),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
        }
        wait_for_total(state, session, *root, 3).await;

        let first = list(state, session, *root, 1, 2).await;
        assert_eq!(first.total, 3);
        assert_eq!(first.items.len(), 2);
        assert!(first.items[0].created_at >= first.items[1].created_at);

        let second = list(state, session, *root, 2, 2).await;
        assert_eq!(second.total, 3);
        assert_eq!(second.items.len(), 1);

        let past_the_end = list(state, session, *root, 3, 2).await;
        assert_eq!(past_the_end.total, 3);
        assert!(past_the_end.items.is_empty());
    }

    /// Activities are independently scoped to the root being listed.
    #[test]
    async fn lists_only_the_requested_root(state: TestState, session: Session, root: Uuid) {
        test::upload(state, &session.user, *root, "mine.txt", b"mine").await;
        wait_for_total(state, session, *root, 1).await;

        let other_name = format!("activity-other-{}", Uuid::new_v4());
        user(state, &other_name).await;
        let other_session = test::login(state, &other_name, "temporary").await;
        let other_root = test::private_root(state, &other_session.user).await;
        test::upload(state, &other_session.user, other_root, "theirs.txt", b"theirs").await;
        wait_for_total(state, &other_session, other_root, 1).await;

        let mine = list(state, session, *root, 1, 10).await;
        assert_eq!(mine.total, 1);
        assert_eq!(mine.items[0].root_id, *root);
        assert!(matches!(
            &mine.items[0].data,
            ActivityData::Create(ActivityTarget::Single { name, .. }) if name == "mine.txt"
        ));

        let theirs = list(state, &other_session, other_root, 1, 10).await;
        assert_eq!(theirs.total, 1);
        assert_eq!(theirs.items[0].root_id, other_root);
        assert!(matches!(
            &theirs.items[0].data,
            ActivityData::Create(ActivityTarget::Single { name, .. }) if name == "theirs.txt"
        ));
    }
}
