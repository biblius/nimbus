#[suitest::suite(files_trash)]
#[suitest::suite_cfg(sequential = true)]
mod trash {
    use super::super::{count, upload_in, user};
    use crate::{
        error::AppError,
        file::model::{CreateDirectory, RestoreFile, TrashFiles},
        test::{self, TestState},
    };
    use chrono::Utc;
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
            state.services.files.content(&session.user, first.id).await,
            Err(AppError::ResourceMissing(_))
        ));
        assert!(matches!(
            state.services.files.content(&session.user, second.id).await,
            Err(AppError::ResourceMissing(_))
        ));
        assert!(
            state
                .services
                .files
                .content(&session.user, kept.id)
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
            state.services.files.content(&session.user, kept.id).await,
            Err(AppError::ResourceMissing(_))
        ));
        assert!(
            state
                .services
                .files
                .content(&bob.user, foreign.id)
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
            .files
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
        let created = inside.created_at;

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
            .files
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
            .files
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
            .files
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
            .files
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
}
