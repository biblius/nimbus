#[suitest::suite(files_service)]
#[suitest::suite_cfg(sequential = true)]
mod files_service {
    use crate::{
        error::{AppError, AuthFailure},
        file::model::{CreateDirectory, TrashFiles},
        model::PaginationSort,
        test::{self, TestState},
        user::model::{CreateSharedFolder, CreateUser, Role, SetQuota, UpdateRoot, User},
    };
    use chrono::{Duration, Utc};
    use suitest::before_all;
    use uuid::Uuid;

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
    }

    #[test]
    async fn identical_uploads_share_one_blob(state: TestState) {
        let ada = user(state, "ada-dedup").await;
        let session = test::login(state, "ada-dedup", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let stored = test::submit(
            state,
            &session.user,
            root,
            vec![("a.txt", b"abc".as_slice()), ("b.txt", b"abc".as_slice())],
        )
        .await;
        let first = stored[0].as_ref().unwrap();
        let second = stored[1].as_ref().unwrap();

        assert_eq!(first.hash, second.hash);
        assert_eq!(first.created_by, ada.id);
        assert_eq!(second.created_by, ada.id);
        assert_eq!(test::blob_count(state.storage_dir()), 1);
    }

    #[test]
    async fn same_name_can_be_stored_more_than_once(state: TestState) {
        let _ada = user(state, "ada-dupname").await;
        let session = test::login(state, "ada-dupname", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let stored = test::submit(
            state,
            &session.user,
            root,
            vec![
                ("same.txt", b"one".as_slice()),
                ("same.txt", b"two".as_slice()),
            ],
        )
        .await;
        let first = stored[0].as_ref().unwrap();
        let second = stored[1].as_ref().unwrap();
        assert_eq!(first.name, "same.txt");
        assert_eq!(second.name, "same.txt");
        assert_ne!(first.id, second.id);

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
        let notes_again = state
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
        assert_eq!(notes.name, "notes");
        assert_eq!(notes_again.name, "notes");
        assert_ne!(notes.id, notes_again.id);

        let listed = state
            .services
            .files
            .list(&session.user, root, None)
            .await
            .unwrap();
        assert_eq!(
            listed
                .iter()
                .filter(|entry| entry.name == "same.txt")
                .count(),
            2
        );
        assert_eq!(
            listed.iter().filter(|entry| entry.name == "notes").count(),
            2
        );
    }

    #[test]
    async fn shared_root_enforces_quota(state: TestState) {
        let ada = user(state, "ada-quota").await;
        let session = test::login(state, "ada-quota", "temporary").await;
        let shared = state
            .services
            .users
            .create_shared(CreateSharedFolder {
                name: "family".into(),
                quota_bytes: 10,
                members: vec![ada.id],
            })
            .await
            .unwrap();

        let stored = test::upload(state, &session.user, shared.id, "shared.txt", b"abc").await;
        assert!(stored.hash.is_some());
        assert_eq!(stored.created_by, ada.id);

        let overflow = test::submit(
            state,
            &session.user,
            shared.id,
            vec![("big.txt", b"12345678".as_slice())],
        )
        .await;
        assert!(matches!(overflow[0], Err(AppError::QuotaExceeded)));
    }

    #[test]
    async fn unassigned_user_cannot_upload(state: TestState) {
        let ada = user(state, "ada-access").await;
        let _bob = user(state, "bob-access").await;
        let shared = state
            .services
            .users
            .create_shared(CreateSharedFolder {
                name: "private-share".into(),
                quota_bytes: 0,
                members: vec![ada.id],
            })
            .await
            .unwrap();

        let bob = test::login(state, "bob-access", "temporary").await;
        let forbidden = test::submit(
            state,
            &bob.user,
            shared.id,
            vec![("nope.txt", b"zzz".as_slice())],
        )
        .await;
        assert!(matches!(forbidden[0], Err(AppError::Forbidden(_))));
    }

    #[test]
    async fn purge_removes_unreferenced_blobs(state: TestState) {
        let _ada = user(state, "ada-purge").await;
        let session = test::login(state, "ada-purge", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let before = test::blob_count(state.storage_dir());
        let first = test::upload(state, &session.user, root, "a.txt", b"purge-one").await;
        let second = test::upload(state, &session.user, root, "b.txt", b"purge-one").await;
        let extra = test::upload(state, &session.user, root, "c.txt", b"purge-two").await;
        assert_eq!(test::blob_count(state.storage_dir()), before + 2);

        state
            .services
            .files
            .delete(&session.user, first.id)
            .await
            .unwrap();
        state
            .services
            .files
            .delete(&session.user, extra.id)
            .await
            .unwrap();
        assert!(matches!(
            state.services.files.content(&session.user, first.id).await,
            Err(AppError::ResourceMissing(_))
        ));
        assert_eq!(test::blob_count(state.storage_dir()), before + 2);

        state
            .services
            .files
            .delete(&session.user, second.id)
            .await
            .unwrap();
        state
            .services
            .files
            .purge_deleted_before(Utc::now() + Duration::seconds(5))
            .await
            .unwrap();
        assert_eq!(test::blob_count(state.storage_dir()), before);
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
    async fn deleting_a_user_removes_their_files(state: TestState) {
        let _bob = user(state, "bob-delete").await;
        let bob = test::login(state, "bob-delete", "temporary").await;
        let bob_root = test::private_root(state, &bob.user).await;
        let shared_bytes = b"kept-because-bob-has-a-copy";
        let bob_file = test::upload(state, &bob.user, bob_root, "kept.txt", shared_bytes).await;

        let ada = user(state, "ada-delete").await;
        let session = test::login(state, "ada-delete", "temporary").await;
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
        let nested = upload_in(
            state,
            &session.user,
            root,
            Some(notes.id),
            "nested.txt",
            b"ada-nested",
        )
        .await;
        let top = test::upload(state, &session.user, root, "top.txt", b"ada-only").await;
        let copy = test::upload(state, &session.user, root, "copy.txt", shared_bytes).await;
        let blobs_with_ada = test::blob_count(state.storage_dir());

        let empty = user(state, "empty-delete").await;
        let empty_root = test::private_root(
            state,
            &test::login(state, "empty-delete", "temporary").await.user,
        )
        .await;

        state.services.users.delete_user(ada.id).await.unwrap();

        for id in [notes.id, nested.id, top.id, copy.id] {
            assert!(matches!(
                state.services.files.metadata(&session.user, id).await,
                Err(AppError::ResourceMissing(_))
            ));
        }
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM files WHERE root_id = $1", root),
            0
        );
        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM files WHERE created_by = $1",
                ada.id
            ),
            0
        );
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM roots WHERE id = $1", root),
            0
        );
        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM user_folders WHERE user_id = $1",
                ada.id
            ),
            0
        );
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM users WHERE id = $1", ada.id),
            0
        );
        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM sessions WHERE user_id = $1",
                ada.id
            ),
            0
        );
        assert!(matches!(
            state.services.auth.get_session_by_id(session.id).await,
            Err(AppError::Auth(AuthFailure::SessionNotFound))
        ));
        // Bob still references the shared bytes, so that blob stays.
        assert_eq!(test::blob_count(state.storage_dir()), blobs_with_ada - 2);
        let kept = state
            .services
            .files
            .content(&bob.user, bob_file.id)
            .await
            .unwrap();
        assert_eq!(kept.0.id, bob_file.id);

        state.services.users.delete_user(empty.id).await.unwrap();
        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM roots WHERE id = $1",
                empty_root
            ),
            0
        );
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM users WHERE id = $1", empty.id),
            0
        );

        let missing = state.services.users.delete_user(Uuid::new_v4()).await;
        assert!(matches!(missing, Err(AppError::ResourceMissing(_))));
    }

    #[test]
    async fn descendants_cover_every_level(state: TestState) {
        let _ada = user(state, "ada-tree").await;
        let session = test::login(state, "ada-tree", "temporary").await;
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
        let deeper = state
            .services
            .files
            .create_directory(
                &session.user,
                root,
                CreateDirectory {
                    parent: Some(nested.id),
                    name: "deeper".into(),
                },
            )
            .await
            .unwrap();
        let leaf = upload_in(
            state,
            &session.user,
            root,
            Some(deeper.id),
            "leaf.txt",
            b"leaf",
        )
        .await;
        let mid = upload_in(
            state,
            &session.user,
            root,
            Some(notes.id),
            "mid.txt",
            b"mid",
        )
        .await;
        let top = test::upload(state, &session.user, root, "top.txt", b"top").await;

        let repo = crate::db::FileRepository::new(state.pool.clone());
        let descendants = repo.list_descendants(root, Some(notes.id)).await.unwrap();

        let mut ids: Vec<_> = descendants.iter().map(|entry| entry.id).collect();
        ids.sort();
        let mut expected = vec![nested.id, deeper.id, leaf.id, mid.id];
        expected.sort();
        assert_eq!(ids, expected);

        let nested_row = descendants
            .iter()
            .find(|entry| entry.id == nested.id)
            .unwrap();
        assert!(nested_row.file_type.is_none());
        assert!(nested_row.hash.is_none());
        let leaf_row = descendants
            .iter()
            .find(|entry| entry.id == leaf.id)
            .unwrap();
        assert!(leaf_row.file_type.is_some());
        assert_eq!(leaf_row.parent, Some(deeper.id));

        assert!(!ids.contains(&notes.id));
        assert!(!ids.contains(&top.id));
        assert!(
            repo.list_descendants(root, Some(leaf.id))
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    async fn subdirectory_listing_marks_only_its_children_as_query_roots(state: TestState) {
        let _ada = user(state, "ada-subdir").await;
        let session = test::login(state, "ada-subdir", "temporary").await;
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
        let beside = upload_in(
            state,
            &session.user,
            root,
            Some(notes.id),
            "beside.txt",
            b"beside",
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
        let child = upload_in(
            state,
            &session.user,
            root,
            Some(nested.id),
            "child.txt",
            b"child",
        )
        .await;
        let deeper = state
            .services
            .files
            .create_directory(
                &session.user,
                root,
                CreateDirectory {
                    parent: Some(nested.id),
                    name: "deeper".into(),
                },
            )
            .await
            .unwrap();
        let leaf = upload_in(
            state,
            &session.user,
            root,
            Some(deeper.id),
            "leaf.txt",
            b"leaf",
        )
        .await;
        let outside = test::upload(state, &session.user, root, "outside.txt", b"out").await;

        let repo = crate::db::FileRepository::new(state.pool.clone());
        let listed = repo.list_descendants(root, Some(nested.id)).await.unwrap();

        let flags: std::collections::HashMap<_, _> = listed
            .iter()
            .map(|entry| (entry.id, entry.query_root))
            .collect();
        assert_eq!(
            flags,
            [(child.id, true), (deeper.id, true), (leaf.id, false),]
                .into_iter()
                .collect()
        );
        for id in [notes.id, nested.id, beside.id, outside.id] {
            assert!(!flags.contains_key(&id));
        }
    }

    #[test]
    async fn get_tree_nests_descendants_in_query_order(state: TestState) {
        let _ada = user(state, "ada-get-tree").await;
        let session = test::login(state, "ada-get-tree", "temporary").await;
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
        let outside = test::upload(state, &session.user, root, "outside.txt", b"out").await;
        upload_in(state, &session.user, root, Some(notes.id), "b.txt", b"b").await;
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
        upload_in(state, &session.user, root, Some(notes.id), "a.txt", b"a").await;
        let empty = state
            .services
            .files
            .create_directory(
                &session.user,
                root,
                CreateDirectory {
                    parent: Some(notes.id),
                    name: "empty".into(),
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

        let tree = state
            .services
            .files
            .get_tree(&session.user, root, Some(notes.id))
            .await
            .unwrap();
        assert_eq!(tree.id, notes.id);
        assert!(tree.name.is_empty());
        assert_eq!(
            tree.children
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            ["empty", "nested", "a.txt", "b.txt"]
        );
        assert!(tree.children[0].children.is_empty());
        assert_eq!(tree.children[1].id, nested.id);
        assert_eq!(tree.children[1].children.len(), 1);
        assert_eq!(tree.children[1].children[0].id, leaf.id);
        assert!(tree.children.iter().all(|entry| entry.id != outside.id));

        let empty_tree = state
            .services
            .files
            .get_tree(&session.user, root, Some(empty.id))
            .await
            .unwrap();
        assert_eq!(empty_tree.id, empty.id);
        assert!(empty_tree.children.is_empty());

        let root_tree = state
            .services
            .files
            .get_tree(&session.user, root, None)
            .await
            .unwrap();
        assert_eq!(root_tree.id, root);
        assert!(root_tree.name.is_empty());
        assert_eq!(
            root_tree
                .children
                .iter()
                .map(|entry| entry.id)
                .collect::<Vec<_>>(),
            vec![notes.id, outside.id]
        );
        assert_eq!(root_tree.children[0].children.len(), 4);
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

        let notes_place: (Option<Uuid>, Option<Uuid>) =
            sqlx::query_as("SELECT parent, original_parent FROM trashed_files WHERE id = $1")
                .bind(notes.id)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(notes_place, (None, None));

        let inner_place: (Option<Uuid>, Option<Uuid>) =
            sqlx::query_as("SELECT parent, original_parent FROM trashed_files WHERE id = $1")
                .bind(inner.id)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(inner_place, (None, Some(kept.id)));

        let buried_place: (Option<Uuid>, Option<Uuid>) =
            sqlx::query_as("SELECT parent, original_parent FROM trashed_files WHERE id = $1")
                .bind(buried.id)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(buried_place, (Some(nested.id), Some(nested.id)));

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
    async fn purge_removes_a_directory_tree(state: TestState) {
        let _ada = user(state, "ada-tree-purge").await;
        let session = test::login(state, "ada-tree-purge", "temporary").await;
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
            b"tree-purge-leaf",
        )
        .await;
        let shared = b"tree-purge-shared";
        let inside = upload_in(
            state,
            &session.user,
            root,
            Some(notes.id),
            "inside.txt",
            shared,
        )
        .await;
        let outside = test::upload(state, &session.user, root, "outside.txt", shared).await;
        let leaf_hash = leaf.hash.clone().unwrap();
        let shared_hash = outside.hash.clone().unwrap();
        assert_eq!(inside.hash.as_deref(), Some(shared_hash.as_str()));
        let leaf_blob = state
            .storage_dir()
            .join("storage")
            .join("data")
            .join(&leaf_hash);
        let shared_blob = state
            .storage_dir()
            .join("storage")
            .join("data")
            .join(&shared_hash);

        state
            .services
            .files
            .delete(&session.user, notes.id)
            .await
            .unwrap();
        assert!(matches!(
            state.services.files.content(&session.user, leaf.id).await,
            Err(AppError::ResourceMissing(_))
        ));
        assert!(leaf_blob.exists());
        assert!(shared_blob.exists());

        state
            .services
            .files
            .purge_deleted_before(Utc::now() + Duration::seconds(5))
            .await
            .unwrap();

        for id in [notes.id, nested.id, leaf.id, inside.id] {
            assert!(matches!(
                state.services.files.metadata(&session.user, id).await,
                Err(AppError::ResourceMissing(_))
            ));
        }
        assert!(
            state
                .services
                .files
                .content(&session.user, outside.id)
                .await
                .is_ok()
        );
        assert!(!leaf_blob.exists());
        assert!(shared_blob.exists());
    }

    #[test]
    async fn the_last_admin_cannot_be_deleted(state: TestState) {
        let admin = test::login(state, "admin", "admin").await;
        let root = test::private_root(state, &admin.user).await;
        let file = test::upload(state, &admin.user, root, "admin.txt", b"admin-file").await;

        let forbidden = state.services.users.delete_user(admin.user.id).await;
        assert!(matches!(forbidden, Err(AppError::Forbidden(_))));
        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM users WHERE id = $1",
                admin.user.id
            ),
            1
        );
        let still_there = state
            .services
            .files
            .content(&admin.user, file.id)
            .await
            .unwrap();
        assert_eq!(still_there.0.id, file.id);

        let extra = state
            .services
            .users
            .create_user(CreateUser {
                username: "second-admin".into(),
                password: "temporary".into(),
                email: None,
                role: Role::Admin,
            })
            .await
            .unwrap();
        let extra_session = test::login(state, "second-admin", "temporary").await;
        let extra_root = test::private_root(state, &extra_session.user).await;
        let extra_file = test::upload(
            state,
            &extra_session.user,
            extra_root,
            "extra.txt",
            b"extra-admin",
        )
        .await;

        state.services.users.delete_user(extra.id).await.unwrap();
        assert!(matches!(
            state
                .services
                .files
                .metadata(&extra_session.user, extra_file.id)
                .await,
            Err(AppError::ResourceMissing(_))
        ));
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM users WHERE id = $1", extra.id),
            0
        );
        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM users WHERE id = $1",
                admin.user.id
            ),
            1
        );
    }

    #[test]
    async fn admin_can_rename_a_root_and_change_its_quota(state: TestState) {
        let ada = user(state, "ada-rename").await;
        let session = test::login(state, "ada-rename", "temporary").await;
        let shared = state
            .services
            .users
            .create_shared(CreateSharedFolder {
                name: "old-name".into(),
                quota_bytes: 10,
                members: vec![ada.id],
            })
            .await
            .unwrap();

        state
            .services
            .users
            .set_quota(shared.id, SetQuota { quota_bytes: 32 })
            .await
            .unwrap();

        let after_quota = state
            .services
            .files
            .list_accessible_roots(&session.user)
            .await
            .unwrap();
        let limited = after_quota
            .iter()
            .find(|root| root.id == shared.id)
            .unwrap();
        assert_eq!(limited.quota, 32);

        let renamed = state
            .services
            .users
            .update_root(
                shared.id,
                UpdateRoot {
                    name: "new-name".into(),
                    quota_bytes: 64,
                    members: vec![ada.id],
                },
            )
            .await
            .unwrap();
        assert_eq!(renamed.name, "new-name");
        assert_eq!(renamed.quota, 64);

        let roots = state
            .services
            .files
            .list_accessible_roots(&session.user)
            .await
            .unwrap();
        let folder = roots.iter().find(|root| root.id == shared.id).unwrap();
        assert_eq!(folder.name, "new-name");
        assert_eq!(folder.quota, 64);
    }

    #[test]
    async fn private_root_cannot_be_shared(state: TestState) {
        let ada = user(state, "ada-private-share").await;
        let bob = user(state, "bob-private-share").await;
        let session = test::login(state, "ada-private-share", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let assigned = state.services.users.assign(root, bob.id).await;
        assert!(matches!(assigned, Err(AppError::Forbidden(_))));

        let updated = state
            .services
            .users
            .update_root(
                root,
                UpdateRoot {
                    name: "shared-attempt".into(),
                    quota_bytes: 0,
                    members: vec![ada.id, bob.id],
                },
            )
            .await;
        assert!(matches!(updated, Err(AppError::Forbidden(_))));

        let members = state
            .services
            .users
            .list_members(&session.user, root, PaginationSort::default())
            .await
            .unwrap();
        assert!(members.items.is_empty());
        assert_eq!(members.total, 0);
        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM user_folders WHERE root_id = $1",
                root
            ),
            0
        );

        let renamed = state
            .services
            .users
            .update_root(
                root,
                UpdateRoot {
                    name: "ada-renamed".into(),
                    quota_bytes: 128,
                    members: vec![],
                },
            )
            .await
            .unwrap();
        assert_eq!(renamed.name, "ada-renamed");
        assert_eq!(renamed.quota, 128);
        assert_eq!(renamed.owner, Some(ada.id));

        let deleted = state.services.users.delete_root(root).await;
        assert!(matches!(deleted, Err(AppError::Forbidden(_))));
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM roots WHERE id = $1", root),
            1
        );
    }

    async fn upload_in(
        state: &TestState,
        user: &User,
        root: Uuid,
        parent: Option<Uuid>,
        name: &str,
        bytes: &[u8],
    ) -> crate::file::model::FileEntry {
        let mut writer = state
            .services
            .files
            .start_upload(user, root, parent, name.to_string())
            .await
            .unwrap();
        writer.write(bytes).await.unwrap();
        let file = writer.finish().await.unwrap();
        state
            .services
            .files
            .finish_upload(user, root, parent, file)
            .await
            .unwrap()
    }

    macro_rules! count {
        ($state:expr, $sql:literal, $id:expr) => {
            sqlx::query_scalar::<_, i64>($sql)
                .bind($id)
                .fetch_one(&$state.pool)
                .await
                .unwrap()
        };
    }

    async fn user(state: &TestState, username: &str) -> crate::user::model::UserViewFull {
        state
            .services
            .users
            .create_user(CreateUser {
                username: username.into(),
                password: "temporary".into(),
                email: None,
                role: Role::User,
            })
            .await
            .unwrap()
    }
}
