#[suitest::suite(files_trash)]
#[suitest::suite_cfg(sequential = true)]
mod trash {
    use super::super::{ctx, count, shared_root, upload_in, user};
    use crate::{
        auth::session::Session,
        error::AppError,
        file::model::{CreateDirectory, RestoreFile, TrashFiles},
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
        let username = format!("trash-{}", Uuid::new_v4());
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

    /// Missing live file independently yields ResourceMissing.
    #[test]
    async fn delete_missing_file(state: TestState, session: Session, root: Uuid) {
        let err = state
            .services
            .files
            .delete(
                &ctx(state, &session.user, *root).await,
                Uuid::new_v4(),
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::ResourceMissing(_)));
    }

    /// Soft-deleting a live file moves it out of `files` and into trash.
    #[test]
    async fn delete_moves_live_to_trash(state: TestState, session: Session, root: Uuid) {
        let file = test::upload(state, &session.user, *root, "notes.txt", b"notes").await;

        state
            .services
            .files
            .delete(
                &ctx(state, &session.user, *root).await,
                file.id,
            )
            .await
            .unwrap();

        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM files WHERE id = $1", file.id),
            0
        );
        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM trashed_files WHERE id = $1",
                file.id
            ),
            1
        );
        assert!(matches!(
            state
                .services
                .files
                .content_for_download(&ctx(state, &session.user, *root).await, file.id)
                .await,
            Err(AppError::ResourceMissing(_))
        ));
    }

    /// Empty `ids` independently fails validate before any trash logic runs.
    #[test]
    async fn delete_many_rejects_empty_ids(state: TestState, session: Session, root: Uuid) {
        let err = state
            .services
            .files
            .delete_many(
                &ctx(state, &session.user, *root).await,
                TrashFiles { ids: vec![] },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Validation(_)));
    }

    /// Missing trashed row independently yields ResourceMissing.
    #[test]
    async fn restore_missing_trashed(state: TestState, session: Session, root: Uuid) {
        let err = state
            .services
            .trash
            .restore(
                &ctx(state, &session.user, *root).await,
                Uuid::new_v4(),
                RestoreFile { parent: None },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::ResourceMissing(_)));
    }

    /// Restore target folder missing independently yields Unprocessable.
    #[test]
    async fn restore_parent_missing(state: TestState, session: Session, root: Uuid) {
        let file = test::upload(state, &session.user, *root, "gone.txt", b"gone").await;

        state
            .services
            .files
            .delete(
                &ctx(state, &session.user, *root).await,
                file.id,
            )
            .await
            .unwrap();

        let err = state
            .services
            .trash
            .restore(
                &ctx(state, &session.user, *root).await,
                file.id,
                RestoreFile {
                    parent: Some(Uuid::new_v4()),
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Unprocessable(_)));
    }

    /// Restore target that is not a directory independently yields Unprocessable.
    #[test]
    async fn restore_parent_not_directory(state: TestState, session: Session, root: Uuid) {
        let sibling = test::upload(state, &session.user, *root, "live.txt", b"live").await;
        let trashed = test::upload(state, &session.user, *root, "trashed.txt", b"trash").await;

        state
            .services
            .files
            .delete(
                &ctx(state, &session.user, *root).await,
                trashed.id,
            )
            .await
            .unwrap();

        let err = state
            .services
            .trash
            .restore(
                &ctx(state, &session.user, *root).await,
                trashed.id,
                RestoreFile {
                    parent: Some(sibling.id),
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Unprocessable(_)));
    }

    /// Restoring a trashed file puts it back under the chosen parent and keeps metadata.
    #[test]
    async fn restore_success(state: TestState, session: Session, root: Uuid) {
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

        let created: chrono::DateTime<Utc> =
            sqlx::query_scalar("SELECT created_at FROM files WHERE id = $1")
                .bind(inside.id)
                .fetch_one(&state.pool)
                .await
                .unwrap();

        state
            .services
            .files
            .delete(
                &ctx(state, &session.user, *root).await,
                inside.id,
            )
            .await
            .unwrap();

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

        let back = state
            .services
            .files
            .get(&ctx(state, &session.user, *root).await, inside.id)
            .await
            .unwrap();
        assert_eq!(back.parent, Some(notes.id));
        assert_eq!(back.created_at, created);
        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM trashed_files WHERE id = $1",
                inside.id
            ),
            0
        );
    }

    /// Restoring when trash is empty is a no-op.
    #[test]
    async fn restore_all_empty_trash_no_op(state: TestState, session: Session, root: Uuid) {
        state
            .services
            .trash
            .restore_all(
                &ctx(state, &session.user, *root).await,
                RestoreFile { parent: None },
            )
            .await
            .unwrap();

        let tree = state
            .services
            .trash
            .get_trash_tree(&ctx(state, &session.user, *root).await)
            .await
            .unwrap();
        assert!(tree.children.is_empty());
    }

    /// `restore_all` moves every trash root under the chosen parent and clears trash.
    #[test]
    async fn restore_all_puts_trash_roots_at_chosen_parent(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let inbox = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
                CreateDirectory {
                    parent: None,
                    name: "inbox".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
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
        let leaf = upload_in(
            state,
            &session.user,
            *root,
            Some(notes.id),
            "leaf.txt",
            b"leaf",
        )
        .await;
        let loose = test::upload(state, &session.user, *root, "loose.txt", b"loose").await;

        state
            .services
            .files
            .delete_many(
                &ctx(state, &session.user, *root).await,
                TrashFiles {
                    ids: vec![notes.id, loose.id],
                },
            )
            .await
            .unwrap();

        state
            .services
            .trash
            .restore_all(
                &ctx(state, &session.user, *root).await,
                RestoreFile {
                    parent: Some(inbox.id),
                },
            )
            .await
            .unwrap();

        let trash = state
            .services
            .trash
            .get_trash_tree(&ctx(state, &session.user, *root).await)
            .await
            .unwrap();
        assert!(trash.children.is_empty());

        let restored_notes = state
            .services
            .files
            .get(&ctx(state, &session.user, *root).await, notes.id)
            .await
            .unwrap();
        assert_eq!(restored_notes.parent, Some(inbox.id));

        let restored_loose = state
            .services
            .files
            .get(&ctx(state, &session.user, *root).await, loose.id)
            .await
            .unwrap();
        assert_eq!(restored_loose.parent, Some(inbox.id));

        let restored_leaf = state
            .services
            .files
            .get(&ctx(state, &session.user, *root).await, leaf.id)
            .await
            .unwrap();
        assert_eq!(restored_leaf.parent, Some(notes.id));
    }

    /// Trash tree lists trashed roots at the top level and nests descendants.
    #[test]
    async fn get_trash_tree_lists_trashed_roots_and_children(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
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
        let child = upload_in(
            state,
            &session.user,
            *root,
            Some(notes.id),
            "child.txt",
            b"child",
        )
        .await;
        let loose = test::upload(state, &session.user, *root, "loose.txt", b"loose").await;
        let kept = test::upload(state, &session.user, *root, "kept.txt", b"kept").await;

        state
            .services
            .files
            .delete_many(
                &ctx(state, &session.user, *root).await,
                TrashFiles {
                    ids: vec![notes.id, loose.id],
                },
            )
            .await
            .unwrap();

        let tree = state
            .services
            .trash
            .get_trash_tree(&ctx(state, &session.user, *root).await)
            .await
            .unwrap();

        assert_eq!(tree.id, *root);
        assert!(tree.name.is_empty());
        assert_eq!(tree.children.len(), 2);
        assert!(
            tree.children
                .iter()
                .any(|entry| entry.id == notes.id && entry.name == "notes")
        );
        assert!(
            tree.children
                .iter()
                .any(|entry| entry.id == loose.id && entry.name == "loose.txt")
        );

        let notes_node = tree
            .children
            .iter()
            .find(|entry| entry.id == notes.id)
            .unwrap();
        assert_eq!(notes_node.children.len(), 1);
        assert_eq!(notes_node.children[0].id, child.id);

        for id in [notes.id, child.id, loose.id] {
            assert_eq!(
                count!(state, "SELECT COUNT(*) FROM files WHERE id = $1", id),
                0
            );
        }
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM files WHERE id = $1", kept.id),
            1
        );
    }

    /// Hard-deleting missing trash independently yields ResourceMissing.
    #[test]
    async fn delete_trash_missing(state: TestState, session: Session, root: Uuid) {
        let err = state
            .services
            .trash
            .delete_trash(
                &ctx(state, &session.user, *root).await,
                Uuid::new_v4(),
                state.queue.clone(),
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::ResourceMissing(_)));
    }

    /// Permanent delete streams results for the whole trashed subtree and leaves other trash.
    #[test]
    async fn delete_trash_hard_deletes_tree_and_keeps_other_trash(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let directory = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
                CreateDirectory {
                    parent: None,
                    name: "old".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let child = upload_in(
            state,
            &session.user,
            *root,
            Some(directory.id),
            "child.txt",
            b"child",
        )
        .await;
        let kept = test::upload(state, &session.user, *root, "kept.txt", b"kept").await;

        state
            .services
            .files
            .delete_many(
                &ctx(state, &session.user, *root).await,
                TrashFiles {
                    ids: vec![directory.id, kept.id],
                },
            )
            .await
            .unwrap();

        let rx = state
            .services
            .trash
            .delete_trash(
                &ctx(state, &session.user, *root).await,
                directory.id,
                state.queue.clone(),
            )
            .await
            .unwrap();
        let results = state.await_deletion(rx).await;

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|result| result.success));
        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM trashed_files WHERE id IN ($1, $2)",
                directory.id,
                child.id
            ),
            0
        );
        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM trashed_files WHERE id = $1",
                kept.id
            ),
            1
        );
    }

    /// The deletion worker sweeps trash past the retention window.
    #[test]
    async fn sweep_hard_deletes_expired_trash(state: TestState, session: Session, root: Uuid) {
        let file = test::upload(state, &session.user, *root, "expire-me.txt", b"expire").await;
        let blobs_before = test::blob_count(state.storage_dir());

        state
            .services
            .files
            .delete(
                &ctx(state, &session.user, *root).await,
                file.id,
            )
            .await
            .unwrap();

        let past = Utc::now() - Duration::days(31);
        sqlx::query("UPDATE trashed_files SET trashed_at = $1 WHERE id = $2")
            .bind(past)
            .bind(file.id)
            .execute(&state.pool)
            .await
            .unwrap();

        wait_until(std::time::Duration::from_secs(5), || async {
            count!(
                state,
                "SELECT COUNT(*) FROM trashed_files WHERE id = $1",
                file.id
            ) == 0
                && test::blob_count(state.storage_dir()) == blobs_before - 1
        })
        .await;
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM roots WHERE id = $1", *root),
            1
        );
    }

    /// Deleting a shared root hard-deletes live files, trash, and the root row.
    #[test]
    async fn deleting_shared_root_hard_deletes_live_and_trash(state: TestState) {
        let ada = user(state, &format!("trash-shared-{}", Uuid::new_v4())).await;
        let session = test::login(state, &ada.username, "temporary").await;
        let admin = test::login(state, "admin", "admin").await;

        let shared = shared_root(state, "doomed", &[ada.id]).await;

        let notes = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, shared).await,
                CreateDirectory {
                    parent: None,
                    name: "notes".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let _nested = upload_in(
            state,
            &session.user,
            shared,
            Some(notes.id),
            "nested.txt",
            b"nested",
        )
        .await;
        let loose = test::upload(state, &session.user, shared, "loose.txt", b"loose").await;

        state
            .services
            .files
            .delete_many(
                &ctx(state, &session.user, shared).await,
                TrashFiles {
                    ids: vec![loose.id, notes.id],
                },
            )
            .await
            .unwrap();

        let blobs_before = test::blob_count(state.storage_dir());

        let rx = state
            .services
            .users
            .delete_root(&admin.user, shared, state.queue.clone())
            .await
            .unwrap();
        let results = state.await_deletion(rx).await;
        assert!(!results.is_empty());
        assert!(results.iter().all(|result| result.success));

        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM files WHERE root_id = $1",
                shared
            ),
            0
        );
        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM trashed_files WHERE root_id = $1",
                shared
            ),
            0
        );
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM roots WHERE id = $1", shared),
            0
        );
        assert!(test::blob_count(state.storage_dir()) < blobs_before);
    }
}
