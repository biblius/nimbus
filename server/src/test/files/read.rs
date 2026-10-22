#[suitest::suite(files_read)]
#[suitest::suite_cfg(sequential = true)]
mod read {
    use super::super::{ctx, upload_in, user};
    use crate::{
        auth::session::Session,
        error::AppError,
        file::model::CreateDirectory,
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
        let username = format!("read-{}", Uuid::new_v4());
        user(state, &username).await;
        let session = test::login(state, &username, "temporary").await;
        let root = test::private_root(state, &session.user).await;
        (session, root)
    }

    /// Missing id independently yields ResourceMissing on get.
    #[test]
    async fn get_missing_file_returns_resource_missing(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let err = state
            .services
            .files
            .get(&ctx(state, &session.user, *root).await, Uuid::new_v4())
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::ResourceMissing(_)));
    }

    /// Missing id independently yields ResourceMissing on ancestors.
    #[test]
    async fn ancestors_missing_file_returns_resource_missing(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let err = state
            .services
            .files
            .ancestors(&ctx(state, &session.user, *root).await, Uuid::new_v4())
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::ResourceMissing(_)));
    }

    /// Live file id returns the stored entry.
    #[test]
    async fn get_returns_live_entry(state: TestState, session: Session, root: Uuid) {
        let file = test::upload(state, &session.user, *root, "notes.txt", b"notes").await;

        let live = state
            .services
            .files
            .get(&ctx(state, &session.user, *root).await, file.id)
            .await
            .unwrap();

        assert_eq!(live.id, file.id);
        assert_eq!(live.name, "notes.txt");
        assert_eq!(live.parent, file.parent);
    }

    /// Nested file yields root-to-leaf ancestor chain.
    #[test]
    async fn ancestors_returns_chain(state: TestState, session: Session, root: Uuid) {
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
        let nested = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
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
            *root,
            Some(nested.id),
            "leaf.txt",
            b"leaf",
        )
        .await;

        let chain = state
            .services
            .files
            .ancestors(&ctx(state, &session.user, *root).await, leaf.id)
            .await
            .unwrap();

        assert_eq!(
            chain.iter().map(|entry| entry.id).collect::<Vec<_>>(),
            vec![notes.id, nested.id, leaf.id]
        );
    }

    /// Directory listing returns only direct children.
    #[test]
    async fn list_returns_children(state: TestState, session: Session, root: Uuid) {
        let folder = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
                CreateDirectory {
                    parent: None,
                    name: "docs".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let alpha = upload_in(
            state,
            &session.user,
            *root,
            Some(folder.id),
            "a.txt",
            b"a",
        )
        .await;
        let beta = upload_in(
            state,
            &session.user,
            *root,
            Some(folder.id),
            "b.txt",
            b"b",
        )
        .await;
        test::upload(state, &session.user, *root, "outside.txt", b"out").await;

        let mut listed = state
            .services
            .files
            .list(&ctx(state, &session.user, *root).await, Some(folder.id))
            .await
            .unwrap();
        listed.sort_by(|left, right| left.file.name.cmp(&right.file.name));

        assert_eq!(
            listed.iter().map(|entry| entry.file.id).collect::<Vec<_>>(),
            vec![alpha.id, beta.id]
        );
    }

    /// get_tree nests descendants and honors query order.
    #[test]
    async fn get_tree_nests_descendants(state: TestState, session: Session, root: Uuid) {
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
        let outside = test::upload(state, &session.user, *root, "outside.txt", b"out").await;
        upload_in(state, &session.user, *root, Some(notes.id), "b.txt", b"b").await;
        let nested = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
                CreateDirectory {
                    parent: Some(notes.id),
                    name: "nested".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        upload_in(state, &session.user, *root, Some(notes.id), "a.txt", b"a").await;
        let empty = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
                CreateDirectory {
                    parent: Some(notes.id),
                    name: "empty".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let leaf = upload_in(
            state,
            &session.user,
            *root,
            Some(nested.id),
            "leaf.txt",
            b"leaf",
        )
        .await;

        let tree = state
            .services
            .files
            .get_tree(&ctx(state, &session.user, *root).await, Some(notes.id))
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
            .get_tree(&ctx(state, &session.user, *root).await, Some(empty.id))
            .await
            .unwrap();
        assert_eq!(empty_tree.id, empty.id);
        assert!(empty_tree.children.is_empty());

        let root_tree = state
            .services
            .files
            .get_tree(&ctx(state, &session.user, *root).await, None)
            .await
            .unwrap();
        assert_eq!(root_tree.id, *root);
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

    /// usage sums live file bytes; directories are excluded.
    #[test]
    async fn usage_sums_file_bytes(state: TestState, session: Session, root: Uuid) {
        state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
                CreateDirectory {
                    parent: None,
                    name: "ignored".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        test::upload(state, &session.user, *root, "one.bin", &[0u8; 5]).await;
        test::upload(state, &session.user, *root, "two.bin", &[0u8; 12]).await;

        let usage = state
            .services
            .files
            .usage(&ctx(state, &session.user, *root).await)
            .await
            .unwrap();

        assert_eq!(usage, 17);
    }
}
