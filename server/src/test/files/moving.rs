#[suitest::suite(files_move)]
#[suitest::suite_cfg(sequential = true)]
mod moving {
    use super::super::{upload_in, user};
    use crate::{
        error::AppError,
        file::model::{CreateDirectory, MoveFiles},
        test::{self, TestState},
    };
    use suitest::before_all;

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
    }

    #[test]
    async fn moves_selected_live_entries_under_the_target_directory(state: TestState) {
        let _ada = user(state, "ada-move").await;
        let session = test::login(state, "ada-move", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let destination = state
            .services
            .files
            .create_directory(&super::super::ctx(state, &session.user, root).await, CreateDirectory {
                    parent: None,
                    name: "archive".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let file = test::upload(state, &session.user, root, "notes.txt", b"notes").await;
        let folder = state
            .services
            .files
            .create_directory(&super::super::ctx(state, &session.user, root).await, CreateDirectory {
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
            root,
            Some(folder.id),
            "inside.txt",
            b"inside",
        )
        .await;
        let kept = test::upload(state, &session.user, root, "keep.txt", b"keep").await;

        state
            .services
            .files
            .move_files(&super::super::ctx(state, &session.user, root).await, MoveFiles {
                    ids: vec![file.id, folder.id, file.id],
                    parent: Some(destination.id),
                },
            )
            .await
            .unwrap();

        let moved_file = state
            .services
            .files
            .metadata(&super::super::ctx(state, &session.user, root).await, file.id)
            .await
            .unwrap();
        let moved_folder = state
            .services
            .files
            .metadata(&super::super::ctx(state, &session.user, root).await, folder.id)
            .await
            .unwrap();
        let nested = state
            .services
            .files
            .metadata(&super::super::ctx(state, &session.user, root).await, nested.id)
            .await
            .unwrap();
        let kept = state
            .services
            .files
            .metadata(&super::super::ctx(state, &session.user, root).await, kept.id)
            .await
            .unwrap();

        assert_eq!(moved_file.parent, Some(destination.id));
        assert_eq!(moved_folder.parent, Some(destination.id));
        assert_eq!(nested.parent, Some(folder.id));
        assert_eq!(kept.parent, None);
    }

    #[test]
    async fn moves_entries_to_the_root(state: TestState) {
        let _ada = user(state, "ada-move-root").await;
        let session = test::login(state, "ada-move-root", "temporary").await;
        let root = test::private_root(state, &session.user).await;
        let folder = state
            .services
            .files
            .create_directory(&super::super::ctx(state, &session.user, root).await, CreateDirectory {
                    parent: None,
                    name: "box".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let file = upload_in(
            state,
            &session.user,
            root,
            Some(folder.id),
            "leaf.txt",
            b"leaf",
        )
        .await;

        state
            .services
            .files
            .move_files(&super::super::ctx(state, &session.user, root).await, MoveFiles {
                    ids: vec![file.id],
                    parent: None,
                },
            )
            .await
            .unwrap();

        let moved = state
            .services
            .files
            .metadata(&super::super::ctx(state, &session.user, root).await, file.id)
            .await
            .unwrap();
        assert_eq!(moved.parent, None);
    }

    #[test]
    async fn cannot_move_a_directory_into_itself(state: TestState) {
        let _ada = user(state, "ada-move-cycle").await;
        let session = test::login(state, "ada-move-cycle", "temporary").await;
        let root = test::private_root(state, &session.user).await;
        let folder = state
            .services
            .files
            .create_directory(&super::super::ctx(state, &session.user, root).await, CreateDirectory {
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
            .create_directory(&super::super::ctx(state, &session.user, root).await, CreateDirectory {
                    parent: Some(folder.id),
                    name: "nested".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let error = state
            .services
            .files
            .move_files(&super::super::ctx(state, &session.user, root).await, MoveFiles {
                    ids: vec![folder.id],
                    parent: Some(nested.id),
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::Unprocessable(_)));

        let folder = state
            .services
            .files
            .metadata(&super::super::ctx(state, &session.user, root).await, folder.id)
            .await
            .unwrap();
        assert_eq!(folder.parent, None);
    }

    #[test]
    async fn cannot_move_into_a_file(state: TestState) {
        let _ada = user(state, "ada-move-file").await;
        let session = test::login(state, "ada-move-file", "temporary").await;
        let root = test::private_root(state, &session.user).await;
        let target = test::upload(state, &session.user, root, "target.txt", b"target").await;
        let file = test::upload(state, &session.user, root, "notes.txt", b"notes").await;

        let error = state
            .services
            .files
            .move_files(&super::super::ctx(state, &session.user, root).await, MoveFiles {
                    ids: vec![file.id],
                    parent: Some(target.id),
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::Unprocessable(_)));

        let file = state
            .services
            .files
            .metadata(&super::super::ctx(state, &session.user, root).await, file.id)
            .await
            .unwrap();
        assert_eq!(file.parent, None);
    }
}
