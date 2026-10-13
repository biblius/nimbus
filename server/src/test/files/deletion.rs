#[suitest::suite(files_deletion)]
#[suitest::suite_cfg(sequential = true)]
mod deletion {
    use super::super::{count, upload_in, user};
    use crate::{
        db::{DeletionRepository, deletion::SYSTEM, files::FileTable},
        file::model::{Cleanup, CreateDirectory, FileRef, TrashFiles},
        file::queue::DeletionQueue,
        test::{self, TestState},
        user::model::CreateSharedFolder,
    };
    use chrono::Utc;
    use suitest::before_all;
    use uuid::Uuid;

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
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

    #[test]
    async fn deleting_a_shared_root_hard_deletes_live_and_trash(state: TestState) {
        let ada = user(state, "ada-del-shared").await;
        let session = test::login(state, "ada-del-shared", "temporary").await;
        let admin = test::login(state, "admin", "admin").await;

        let shared = state
            .services
            .users
            .create_shared(CreateSharedFolder {
                name: "doomed".into(),
                quota_bytes: 0,
                members: vec![ada.id],
            })
            .await
            .unwrap();

        let notes = state
            .services
            .files
            .create_directory(
                &session.user,
                shared.id,
                CreateDirectory {
                    parent: None,
                    name: "notes".into(),
                },
            )
            .await
            .unwrap();
        let nested = upload_in(
            state,
            &session.user,
            shared.id,
            Some(notes.id),
            "nested.txt",
            b"nested-shared",
        )
        .await;
        let top = test::upload(state, &session.user, shared.id, "top.txt", b"top-shared").await;
        let loose = test::upload(state, &session.user, shared.id, "loose.txt", b"loose").await;

        state
            .services
            .files
            .delete_many(
                &session.user,
                shared.id,
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
            .delete_root(&admin.user, shared.id, state.queue.clone())
            .await
            .unwrap();
        let results = state.await_deletion(rx).await;
        assert!(!results.is_empty());
        assert!(results.iter().all(|result| result.success));

        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM files WHERE root_id = $1",
                shared.id
            ),
            0
        );
        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM trashed_files WHERE root_id = $1",
                shared.id
            ),
            0
        );
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM roots WHERE id = $1", shared.id),
            0
        );
        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM file_deletion_queue WHERE file_id = $1",
                nested.id
            ),
            0
        );
        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM file_deletion_queue WHERE file_id = $1",
                top.id
            ),
            0
        );
        assert!(test::blob_count(state.storage_dir()) < blobs_before);
    }

    #[test]
    async fn system_sweep_hard_deletes_queued_trash(state: TestState) {
        let _ada = user(state, "ada-del-sweep").await;
        let session = test::login(state, "ada-del-sweep", "temporary").await;
        let root = test::private_root(state, &session.user).await;
        let file = test::upload(state, &session.user, root, "expire-me.txt", b"expire").await;
        let blobs_before = test::blob_count(state.storage_dir());

        state
            .services
            .files
            .delete_many(&session.user, root, TrashFiles { ids: vec![file.id] })
            .await
            .unwrap();

        DeletionRepository::new(state.pool.clone())
            .enqueue(
                Uuid::new_v4(),
                SYSTEM,
                FileTable::Trashed,
                &[FileRef { id: file.id }],
            )
            .await
            .unwrap();

        wait_until(std::time::Duration::from_secs(5), || async {
            count!(
                state,
                "SELECT COUNT(*) FROM trashed_files WHERE id = $1",
                file.id
            ) == 0
        })
        .await;

        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM file_deletion_queue WHERE file_id = $1",
                file.id
            ),
            0
        );
        assert_eq!(test::blob_count(state.storage_dir()), blobs_before - 1);
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM roots WHERE id = $1", root),
            1
        );
    }

    #[test]
    async fn requeue_clears_error_and_finishes_root_cleanup(state: TestState) {
        let ada = user(state, "ada-del-requeue").await;
        let session = test::login(state, "ada-del-requeue", "temporary").await;
        let admin = test::login(state, "admin", "admin").await;

        let shared = state
            .services
            .users
            .create_shared(CreateSharedFolder {
                name: "requeue-me".into(),
                quota_bytes: 0,
                members: vec![ada.id],
            })
            .await
            .unwrap();
        let file = test::upload(state, &session.user, shared.id, "stuck.txt", b"stuck").await;

        state
            .services
            .files
            .delete_many(&session.user, shared.id, TrashFiles { ids: vec![file.id] })
            .await
            .unwrap();

        let stale_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO file_deletion_queue (id, file_id, origin, created_by, created_at, error)
            VALUES ($1, $2, 'trash', $3, $4, 'previous failure')
            "#,
        )
        .bind(stale_id)
        .bind(file.id)
        .bind(SYSTEM)
        .bind(Utc::now())
        .execute(&state.pool)
        .await
        .unwrap();

        let rx = state
            .services
            .users
            .delete_root(&admin.user, shared.id, state.queue.clone())
            .await
            .unwrap();
        state.await_deletion(rx).await;

        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM trashed_files WHERE id = $1",
                file.id
            ),
            0
        );
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM roots WHERE id = $1", shared.id),
            0
        );
        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM file_deletion_queue WHERE file_id = $1",
                file.id
            ),
            0
        );
    }

    #[test]
    async fn deleting_a_user_nulls_created_by_on_shared_files(state: TestState) {
        let ada = user(state, "ada-del-null").await;
        let bob = user(state, "bob-del-null").await;
        let ada_session = test::login(state, "ada-del-null", "temporary").await;
        let bob_session = test::login(state, "bob-del-null", "temporary").await;

        let shared = state
            .services
            .users
            .create_shared(CreateSharedFolder {
                name: "keeps-files".into(),
                quota_bytes: 0,
                members: vec![ada.id, bob.id],
            })
            .await
            .unwrap();

        let file = test::upload(
            state,
            &ada_session.user,
            shared.id,
            "ada-wrote.txt",
            b"still-here",
        )
        .await;

        let rx = state
            .services
            .users
            .delete_user(ada.id, state.queue.clone())
            .await
            .unwrap();
        state.await_deletion(rx).await;

        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM users WHERE id = $1", ada.id),
            0
        );
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM files WHERE id = $1", file.id),
            1
        );

        let created_by: Option<Uuid> =
            sqlx::query_scalar("SELECT created_by FROM files WHERE id = $1")
                .bind(file.id)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(created_by, None);

        let content = state
            .services
            .files
            .content(&bob_session.user, file.id)
            .await
            .unwrap();
        assert_eq!(content.0.id, file.id);
    }

    #[test]
    async fn leftover_files_block_user_cleanup_until_fully_queued(state: TestState) {
        let ada = user(state, "ada-del-skip").await;
        let session = test::login(state, "ada-del-skip", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let parent = state
            .services
            .files
            .create_directory(
                &session.user,
                root,
                CreateDirectory {
                    parent: None,
                    name: "parent".into(),
                },
            )
            .await
            .unwrap();
        let child = upload_in(
            state,
            &session.user,
            root,
            Some(parent.id),
            "child.txt",
            b"child",
        )
        .await;

        // Queue only the directory. The child stays under the root and blocks cleanup.
        let request_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO file_deletion_queue (id, file_id, origin, created_by, created_at, error)
            VALUES ($1, $2, 'files', $3, $4, NULL)
            "#,
        )
        .bind(request_id)
        .bind(parent.id)
        .bind(ada.id.to_string())
        .bind(Utc::now())
        .execute(&state.pool)
        .await
        .unwrap();

        let rx = DeletionQueue::submit(
            state.queue.clone(),
            request_id,
            Cleanup::User {
                user_id: ada.id,
                root_id: root,
            },
        )
        .await
        .unwrap();
        state.await_deletion(rx).await;

        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM files WHERE id = $1", child.id),
            1
        );
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM roots WHERE id = $1", root),
            1
        );
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM users WHERE id = $1", ada.id),
            1
        );

        let rx = state
            .services
            .users
            .delete_user(ada.id, state.queue.clone())
            .await
            .unwrap();
        state.await_deletion(rx).await;
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM users WHERE id = $1", ada.id),
            0
        );
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM roots WHERE id = $1", root),
            0
        );
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM files WHERE id = $1", child.id),
            0
        );
    }
}
