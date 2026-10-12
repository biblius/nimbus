#[suitest::suite(files_tree)]
#[suitest::suite_cfg(sequential = true)]
mod tree {
    use super::super::{upload_in, user};
    use crate::{
        file::model::CreateDirectory,
        test::{self, TestState},
    };
    use suitest::before_all;

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
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
}
