#[suitest::suite(files_trash)]
#[suitest::suite_cfg(sequential = true)]
mod trash {
    use super::super::{count, upload_in, user};
    use crate::{
        db::FileRepository,
        error::AppError,
        file::model::{CreateDirectory, RestoreFile, TrashFiles},
        test::{self, TestState},
    };
    use chrono::Utc;
    use image::ImageFormat;
    use suitest::before_all;
    use uuid::Uuid;

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
    }

    #[test]
    async fn trash_removes_every_listed_file(state: TestState) {
        let _ada = user(state, "ada-trash").await;
        let session = test::login(state, "ada-trash", "temporary").await;
        let root = test::private_root(state, &session.user).await;
        let first = test::upload(state, &session.user, root, "one.txt", b"one").await;
        let second = test::upload(state, &session.user, root, "two.txt", b"two").await;
        let kept = test::upload(state, &session.user, root, "keep.txt", b"keep").await;

        state
            .services
            .files
            .delete_many(
                &session.user,
                root,
                TrashFiles {
                    ids: vec![first.id, second.id, second.id],
                },
            )
            .await
            .unwrap();

        assert!(matches!(
            state
                .services
                .files
                .content_for_download(&session.user, first.id)
                .await,
            Err(AppError::ResourceMissing(_))
        ));
        assert!(matches!(
            state
                .services
                .files
                .content_for_download(&session.user, second.id)
                .await,
            Err(AppError::ResourceMissing(_))
        ));
        assert!(
            state
                .services
                .files
                .content_for_download(&session.user, kept.id)
                .await
                .is_ok()
        );

        let _bob = user(state, "bob-trash").await;
        let bob = test::login(state, "bob-trash", "temporary").await;
        let other = test::private_root(state, &bob.user).await;
        let foreign = test::upload(state, &bob.user, other, "foreign.txt", b"no").await;

        state
            .services
            .files
            .delete_many(
                &session.user,
                root,
                TrashFiles {
                    ids: vec![first.id, kept.id, foreign.id],
                },
            )
            .await
            .unwrap();

        assert!(matches!(
            state
                .services
                .files
                .content_for_download(&session.user, kept.id)
                .await,
            Err(AppError::ResourceMissing(_))
        ));
        assert!(
            state
                .services
                .files
                .content_for_download(&bob.user, foreign.id)
                .await
                .is_ok()
        );
    }

    #[test]
    async fn trash_tree_roots_are_deleted_and_keep_every_child(state: TestState) {
        let _ada = user(state, "ada-trash-tree").await;
        let session = test::login(state, "ada-trash-tree", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let live = test::upload(state, &session.user, root, "live.txt", b"live").await;
        let notes = state
            .services
            .files
            .create_directory(
                &session.user,
                root,
                CreateDirectory {
                    parent: None,
                    name: "notes".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let live_child = upload_in(
            state,
            &session.user,
            root,
            Some(notes.id),
            "live-child.txt",
            b"live-child",
        )
        .await;
        let nested = state
            .services
            .files
            .create_directory(
                &session.user,
                root,
                CreateDirectory {
                    parent: Some(notes.id),
                    name: "nested".into(),
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
        let buried = upload_in(
            state,
            &session.user,
            root,
            Some(nested.id),
            "buried.txt",
            b"buried",
        )
        .await;
        let gone = upload_in(
            state,
            &session.user,
            root,
            Some(notes.id),
            "gone.txt",
            b"gone",
        )
        .await;
        let archive = state
            .services
            .files
            .create_directory(
                &session.user,
                root,
                CreateDirectory {
                    parent: Some(notes.id),
                    name: "archive".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let kept_inside = upload_in(
            state,
            &session.user,
            root,
            Some(archive.id),
            "kept-inside.txt",
            b"kept-inside",
        )
        .await;
        let deep = state
            .services
            .files
            .create_directory(
                &session.user,
                root,
                CreateDirectory {
                    parent: Some(archive.id),
                    name: "deep".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let bottom = upload_in(
            state,
            &session.user,
            root,
            Some(deep.id),
            "bottom.txt",
            b"bottom",
        )
        .await;
        let loose = test::upload(state, &session.user, root, "loose.txt", b"loose").await;
        let kept = state
            .services
            .files
            .create_directory(
                &session.user,
                root,
                CreateDirectory {
                    parent: None,
                    name: "kept".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let inner = upload_in(
            state,
            &session.user,
            root,
            Some(kept.id),
            "inner.txt",
            b"inner",
        )
        .await;

        state
            .services
            .files
            .delete_many(
                &session.user,
                root,
                TrashFiles {
                    ids: vec![
                        notes.id, gone.id, loose.id, inner.id, buried.id, archive.id, deep.id,
                    ],
                },
            )
            .await
            .unwrap();

        let tree = state
            .services
            .trash
            .get_trash_tree(&session.user, root)
            .await
            .unwrap();
        assert_eq!(tree.id, root);
        assert!(tree.name.is_empty());
        assert_eq!(
            tree.children
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            ["notes", "inner.txt", "loose.txt"]
        );

        for child in &tree.children {
            let parent: Option<Uuid> =
                sqlx::query_scalar("SELECT parent FROM trashed_files WHERE id = $1")
                    .bind(child.id)
                    .fetch_one(&state.pool)
                    .await
                    .unwrap();
            assert!(parent.is_none());
            assert_eq!(
                count!(state, "SELECT COUNT(*) FROM files WHERE id = $1", child.id),
                0
            );
        }

        let buried_parent: Option<Uuid> =
            sqlx::query_scalar("SELECT parent FROM trashed_files WHERE id = $1")
                .bind(buried.id)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(buried_parent, Some(nested.id));

        let notes_node = tree
            .children
            .iter()
            .find(|entry| entry.id == notes.id)
            .unwrap();
        assert_eq!(
            notes_node
                .children
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            ["archive", "nested", "gone.txt", "live-child.txt"]
        );

        let archive_node = notes_node
            .children
            .iter()
            .find(|entry| entry.id == archive.id)
            .unwrap();
        assert_eq!(
            archive_node
                .children
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            ["deep", "kept-inside.txt"]
        );
        assert_eq!(archive_node.children[0].id, deep.id);
        assert_eq!(archive_node.children[0].children.len(), 1);
        assert_eq!(archive_node.children[0].children[0].id, bottom.id);

        let nested_node = notes_node
            .children
            .iter()
            .find(|entry| entry.id == nested.id)
            .unwrap();
        assert_eq!(
            nested_node
                .children
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            ["buried.txt", "leaf.txt"]
        );
        assert_eq!(nested_node.children[1].id, leaf.id);

        for id in [
            notes.id,
            live_child.id,
            nested.id,
            leaf.id,
            kept_inside.id,
            bottom.id,
            archive.id,
            deep.id,
            gone.id,
            buried.id,
        ] {
            assert_eq!(
                count!(state, "SELECT COUNT(*) FROM files WHERE id = $1", id),
                0
            );
        }
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM files WHERE id = $1", live.id),
            1
        );
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM files WHERE id = $1", kept.id),
            1
        );

        fn count(tree: &crate::file::tree::FileTree, id: Uuid) -> usize {
            usize::from(tree.id == id)
                + tree
                    .children
                    .iter()
                    .map(|child| count(child, id))
                    .sum::<usize>()
        }
        assert_eq!(count(&tree, buried.id), 1);
        assert_eq!(count(&tree, archive.id), 1);
        assert_eq!(count(&tree, deep.id), 1);
        assert_eq!(count(&tree, gone.id), 1);
        assert_eq!(count(&tree, live.id), 0);
        assert_eq!(count(&tree, kept.id), 0);
    }

    #[test]
    async fn restore_all_puts_every_trash_root_at_the_chosen_parent(state: TestState) {
        let _ada = user(state, "ada-restore-all").await;
        let session = test::login(state, "ada-restore-all", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let inbox = state
            .services
            .files
            .create_directory(
                &session.user,
                root,
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
                &session.user,
                root,
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
            root,
            Some(notes.id),
            "leaf.txt",
            b"leaf",
        )
        .await;
        let loose = test::upload(state, &session.user, root, "loose.txt", b"loose").await;

        state
            .services
            .files
            .delete_many(
                &session.user,
                root,
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
                &session.user,
                root,
                RestoreFile {
                    parent: Some(inbox.id),
                },
            )
            .await
            .unwrap();

        let trash = state
            .services
            .trash
            .get_trash_tree(&session.user, root)
            .await
            .unwrap();
        assert!(trash.children.is_empty());

        let restored_notes = state
            .services
            .files
            .metadata(&session.user, notes.id)
            .await
            .unwrap();
        assert_eq!(restored_notes.parent, Some(inbox.id));

        let restored_loose = state
            .services
            .files
            .metadata(&session.user, loose.id)
            .await
            .unwrap();
        assert_eq!(restored_loose.parent, Some(inbox.id));

        let restored_leaf = state
            .services
            .files
            .metadata(&session.user, leaf.id)
            .await
            .unwrap();
        assert_eq!(restored_leaf.parent, Some(notes.id));
    }

    #[test]
    async fn restore_puts_the_file_at_the_chosen_parent(state: TestState) {
        let _ada = user(state, "ada-restore").await;
        let session = test::login(state, "ada-restore", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let notes = state
            .services
            .files
            .create_directory(
                &session.user,
                root,
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
            root,
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
            .delete(&session.user, inside.id)
            .await
            .unwrap();

        let trashed_at: Option<chrono::DateTime<Utc>> =
            sqlx::query_scalar("SELECT trashed_at FROM trashed_files WHERE id = $1")
                .bind(inside.id)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert!(trashed_at.is_some());
        let stored_created: chrono::DateTime<Utc> =
            sqlx::query_scalar("SELECT created_at FROM trashed_files WHERE id = $1")
                .bind(inside.id)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(stored_created, created);

        let missing_folder = state
            .services
            .trash
            .restore(
                &session.user,
                root,
                inside.id,
                RestoreFile {
                    parent: Some(Uuid::new_v4()),
                },
            )
            .await;
        assert!(matches!(missing_folder, Err(AppError::Unprocessable(_))));

        state
            .services
            .trash
            .restore(
                &session.user,
                root,
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
            .metadata(&session.user, inside.id)
            .await
            .unwrap();
        assert_eq!(back.parent, Some(notes.id));
        assert_eq!(back.created_at, created);

        state
            .services
            .files
            .delete(&session.user, notes.id)
            .await
            .unwrap();

        state
            .services
            .trash
            .restore(&session.user, root, inside.id, RestoreFile { parent: None })
            .await
            .unwrap();

        let moved = state
            .services
            .files
            .metadata(&session.user, inside.id)
            .await
            .unwrap();
        assert_eq!(moved.parent, None);
        assert_eq!(moved.created_at, created);

        let boxed = state
            .services
            .files
            .create_directory(
                &session.user,
                root,
                CreateDirectory {
                    parent: None,
                    name: "box".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let leaf = upload_in(
            state,
            &session.user,
            root,
            Some(boxed.id),
            "leaf.txt",
            b"leaf",
        )
        .await;

        state
            .services
            .files
            .delete(&session.user, boxed.id)
            .await
            .unwrap();

        state
            .services
            .trash
            .restore(&session.user, root, boxed.id, RestoreFile { parent: None })
            .await
            .unwrap();

        let restored_leaf = state
            .services
            .files
            .metadata(&session.user, leaf.id)
            .await
            .unwrap();
        assert_eq!(restored_leaf.parent, Some(boxed.id));
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
    async fn trash_tree_and_preview_endpoint_keep_image_thumbs(state: TestState) {
        let _ada = user(state, "ada-trash-preview").await;
        let session = test::login(state, "ada-trash-preview", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let img = image::RgbImage::from_fn(24, 24, |_, _| image::Rgb([10, 200, 90]));
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Png)
            .unwrap();

        let file = test::upload(state, &session.user, root, "thumb.png", &buf).await;
        let files = FileRepository::new(state.pool.clone());

        wait_until(std::time::Duration::from_secs(5), || {
            let files = files.clone();
            async move {
                matches!(
                    files.get_live(file.id).await,
                    Ok(Some(row))
                        if row.metadata.as_ref().is_some_and(|meta| meta.preview_64.is_some())
                )
            }
        })
        .await;

        let preview_hash = files
            .get_live(file.id)
            .await
            .unwrap()
            .expect("live row")
            .metadata
            .expect("metadata")
            .0
            .preview_64
            .expect("preview_64 hash");

        state
            .services
            .files
            .delete(&session.user, file.id)
            .await
            .unwrap();

        let tree = state
            .services
            .trash
            .get_trash_tree(&session.user, root)
            .await
            .unwrap();
        let node = tree
            .children
            .iter()
            .find(|entry| entry.id == file.id)
            .expect("trashed image in tree");
        assert_eq!(
            node.metadata
                .as_ref()
                .and_then(|meta| meta.preview_64.as_deref()),
            Some(preview_hash.as_str())
        );
        assert!(node.file_type.is_some());

        let preview = state
            .services
            .files
            .preview(
                &session.user,
                file.id,
                crate::file::model::PreviewSize::Icon,
            )
            .await
            .unwrap();
        assert_eq!(preview.mime, "image/webp");
        assert!(preview.len > 0);
    }
}
