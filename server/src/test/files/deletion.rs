#[suitest::suite(files_deletion)]
#[suitest::suite_cfg(sequential = true)]
mod deletion {
    use super::super::{count, upload_in, user};
    use crate::{
        db::{FileRepository, files::FileTable},
        file::model::{Cleanup, CreateDirectory, TrashFiles},
        file::queue::{DeletionQueue, PendingDeletion},
        test::{self, TestState},
        user::model::CreateSharedFolder,
    };
    use chrono::{Duration, Utc};
    use image::ImageFormat;
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

    /// Age a trashed row past the test retention window (30 days).
    async fn expire_trash(state: &TestState, file_id: Uuid) {
        let past = Utc::now() - Duration::days(31);
        sqlx::query("UPDATE trashed_files SET trashed_at = $1 WHERE id = $2")
            .bind(past)
            .bind(file_id)
            .execute(&state.pool)
            .await
            .unwrap();
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
                members: vec![ada.id.into()],
            })
            .await
            .unwrap();

        let notes = state
            .services
            .files
            .create_directory(&super::super::ctx(state, &session.user, shared.id).await, CreateDirectory {
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
            shared.id,
            Some(notes.id),
            "nested.txt",
            b"nested-shared",
        )
        .await;
        let _top = test::upload(state, &session.user, shared.id, "top.txt", b"top-shared").await;
        let loose = test::upload(state, &session.user, shared.id, "loose.txt", b"loose").await;

        state
            .services
            .files
            .delete_many(&super::super::ctx(state, &session.user, shared.id).await, TrashFiles {
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
        assert!(test::blob_count(state.storage_dir()) < blobs_before);
    }

    #[test]
    async fn sweep_hard_deletes_expired_trash(state: TestState) {
        let _ada = user(state, "ada-del-sweep").await;
        let session = test::login(state, "ada-del-sweep", "temporary").await;
        let root = test::private_root(state, &session.user).await;
        let file = test::upload(state, &session.user, root, "expire-me.txt", b"expire").await;
        let blobs_before = test::blob_count(state.storage_dir());

        state
            .services
            .files
            .delete_many(&super::super::ctx(state, &session.user, root).await, TrashFiles { ids: vec![file.id] })
            .await
            .unwrap();

        expire_trash(state, file.id).await;

        // Row delete commits before blob GC in the same sweep — wait for both.
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
            count!(state, "SELECT COUNT(*) FROM roots WHERE id = $1", root),
            1
        );
    }

    #[test]
    async fn deleting_one_trash_tree_streams_results_and_keeps_other_trash(state: TestState) {
        let _ada = user(state, "ada-del-trash-tree").await;
        let session = test::login(state, "ada-del-trash-tree", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let directory = state
            .services
            .files
            .create_directory(&super::super::ctx(state, &session.user, root).await, CreateDirectory {
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
            root,
            Some(directory.id),
            "child.txt",
            b"child",
        )
        .await;
        let kept = test::upload(state, &session.user, root, "kept.txt", b"kept").await;

        state
            .services
            .files
            .delete_many(&super::super::ctx(state, &session.user, root).await, TrashFiles {
                    ids: vec![directory.id, kept.id],
                },
            )
            .await
            .unwrap();

        let rx = state
            .services
            .trash
            .delete_trash(&super::super::ctx(state, &session.user, root).await, directory.id, state.queue.clone())
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

    fn png_bytes() -> Vec<u8> {
        let img = image::RgbImage::from_fn(32, 32, |_, _| image::Rgb([40, 120, 200]));
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Png)
            .unwrap();
        buf
    }

    async fn await_previews(state: &TestState, root_id: Uuid, file_id: Uuid) -> (String, String) {
        let files = FileRepository::new(state.pool.clone());

        wait_until(std::time::Duration::from_secs(5), || {
            let files = files.clone();
            async move {
                matches!(
                    files.get_live(root_id, file_id).await,
                    Ok(Some(row))
                        if row.metadata.preview_64.is_some() && row.metadata.preview_256.is_some()
                )
            }
        })
        .await;

        let meta = files
            .get_live(root_id, file_id)
            .await
            .unwrap()
            .expect("file row")
            .metadata;

        (
            meta.preview_64.expect("preview_64 hash"),
            meta.preview_256.expect("preview_256 hash"),
        )
    }

    #[test]
    async fn hard_delete_removes_preview_blobs(state: TestState) {
        let _ada = user(state, "ada-del-preview").await;
        let session = test::login(state, "ada-del-preview", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let file = test::upload(state, &session.user, root, "shot.png", &png_bytes()).await;
        let (icon_hash, tile_hash) = await_previews(state, root, file.id).await;
        let icon_path = test::preview_path(state.storage_dir(), &icon_hash);
        let tile_path = test::preview_path(state.storage_dir(), &tile_hash);
        assert!(icon_path.is_file());
        assert!(tile_path.is_file());

        let previews_before = test::preview_count(state.storage_dir());

        state
            .services
            .files
            .delete_many(&super::super::ctx(state, &session.user, root).await, TrashFiles { ids: vec![file.id] })
            .await
            .unwrap();

        expire_trash(state, file.id).await;

        wait_until(std::time::Duration::from_secs(5), || async {
            count!(
                state,
                "SELECT COUNT(*) FROM trashed_files WHERE id = $1",
                file.id
            ) == 0
                && !icon_path.exists()
                && !tile_path.exists()
                && test::preview_count(state.storage_dir()) == previews_before - 2
        })
        .await;
    }

    #[test]
    async fn shared_preview_survives_until_last_file_hard_deleted(state: TestState) {
        let _ada = user(state, "ada-del-preview-share").await;
        let session = test::login(state, "ada-del-preview-share", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let png = png_bytes();
        let first = test::upload(state, &session.user, root, "a.png", &png).await;
        let second = test::upload(state, &session.user, root, "b.png", &png).await;

        let (icon_hash, tile_hash) = await_previews(state, root, first.id).await;
        assert_eq!(
            await_previews(state, root, second.id).await,
            (icon_hash.clone(), tile_hash.clone())
        );
        let icon_path = test::preview_path(state.storage_dir(), &icon_hash);
        let tile_path = test::preview_path(state.storage_dir(), &tile_hash);

        state
            .services
            .files
            .delete_many(&super::super::ctx(state, &session.user, root).await, TrashFiles {
                    ids: vec![first.id],
                },
            )
            .await
            .unwrap();

        expire_trash(state, first.id).await;

        wait_until(std::time::Duration::from_secs(5), || async {
            count!(
                state,
                "SELECT COUNT(*) FROM trashed_files WHERE id = $1",
                first.id
            ) == 0
        })
        .await;

        assert!(
            icon_path.is_file() && tile_path.is_file(),
            "shared previews must remain while second file exists"
        );

        state
            .services
            .files
            .delete_many(&super::super::ctx(state, &session.user, root).await, TrashFiles {
                    ids: vec![second.id],
                },
            )
            .await
            .unwrap();

        expire_trash(state, second.id).await;

        wait_until(std::time::Duration::from_secs(5), || async {
            count!(
                state,
                "SELECT COUNT(*) FROM trashed_files WHERE id = $1",
                second.id
            ) == 0
                && !icon_path.exists()
                && !tile_path.exists()
        })
        .await;
    }

    #[test]
    async fn delete_root_hard_deletes_trashed_files(state: TestState) {
        let ada = user(state, "ada-del-requeue").await;
        let session = test::login(state, "ada-del-requeue", "temporary").await;
        let admin = test::login(state, "admin", "admin").await;

        let shared = state
            .services
            .users
            .create_shared(CreateSharedFolder {
                name: "requeue-me".into(),
                quota_bytes: 0,
                members: vec![ada.id.into()],
            })
            .await
            .unwrap();
        let file = test::upload(state, &session.user, shared.id, "stuck.txt", b"stuck").await;

        state
            .services
            .files
            .delete_many(&super::super::ctx(state, &session.user, shared.id).await, TrashFiles { ids: vec![file.id] })
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
                members: vec![ada.id.into(), bob.id.into()],
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
            .content_for_download(&super::super::ctx(state, &bob_session.user, shared.id).await, file.id)
            .await
            .unwrap();
        assert_eq!(content.0, file.name);
    }

    #[test]
    async fn leftover_files_block_user_cleanup_until_fully_queued(state: TestState) {
        let ada = user(state, "ada-del-skip").await;
        let session = test::login(state, "ada-del-skip", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let parent = state
            .services
            .files
            .create_directory(&super::super::ctx(state, &session.user, root).await, CreateDirectory {
                    parent: None,
                    name: "parent".into(),
                    ..Default::default()
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

        // Submit only the directory. The child stays under the root and blocks cleanup.
        let request_id = Uuid::new_v4();
        let rx = DeletionQueue::submit(
            state.queue.clone(),
            request_id,
            vec![PendingDeletion {
                file_id: parent.id,
                root_id: root,
                origin: FileTable::Live,
            }],
            Some(Cleanup::User {
                user_id: ada.id,
                root_id: root,
            }),
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
