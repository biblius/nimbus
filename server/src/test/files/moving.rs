#[suitest::suite(files_move)]
#[suitest::suite_cfg(sequential = true)]
mod moving {
    use super::super::{ctx, upload_in, user};
    use crate::{
        auth::session::Session,
        error::AppError,
        file::model::{CreateDirectory, MoveFiles},
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
        let username = format!("move-{}", Uuid::new_v4());
        user(state, &username).await;
        let session = test::login(state, &username, "temporary").await;
        let root = test::private_root(state, &session.user).await;
        (session, root)
    }

    /// Empty `ids` independently fails validate before any move logic runs.
    #[test]
    async fn move_rejects_empty_ids(state: TestState, session: Session, root: Uuid) {
        let err = state
            .services
            .files
            .move_files(
                &ctx(state, &session.user, *root).await,
                MoveFiles {
                    ids: vec![],
                    parent: None,
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Validation(_)));
    }

    /// Missing destination folder independently yields ResourceMissing.
    #[test]
    async fn move_parent_missing(state: TestState, session: Session, root: Uuid) {
        let file = test::upload(state, &session.user, *root, "notes.txt", b"notes").await;

        let err = state
            .services
            .files
            .move_files(
                &ctx(state, &session.user, *root).await,
                MoveFiles {
                    ids: vec![file.id],
                    parent: Some(Uuid::new_v4()),
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::ResourceMissing(_)));
    }

    /// Moving selected live entries updates parent; duplicates in `ids` are ignored.
    #[test]
    async fn moves_selected_live_entries_under_the_target_directory(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
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
        let nested = upload_in(
            state,
            &session.user,
            *root,
            Some(folder.id),
            "inside.txt",
            b"inside",
        )
        .await;
        let kept = test::upload(state, &session.user, *root, "keep.txt", b"keep").await;

        state
            .services
            .files
            .move_files(
                &ctx(state, &session.user, *root).await,
                MoveFiles {
                    ids: vec![file.id, folder.id, file.id],
                    parent: Some(destination.id),
                },
            )
            .await
            .unwrap();

        let moved_file = state
            .services
            .files
            .get(&ctx(state, &session.user, *root).await, file.id)
            .await
            .unwrap();
        let moved_folder = state
            .services
            .files
            .get(&ctx(state, &session.user, *root).await, folder.id)
            .await
            .unwrap();
        let nested = state
            .services
            .files
            .get(&ctx(state, &session.user, *root).await, nested.id)
            .await
            .unwrap();
        let kept = state
            .services
            .files
            .get(&ctx(state, &session.user, *root).await, kept.id)
            .await
            .unwrap();

        assert_eq!(moved_file.parent, Some(destination.id));
        assert_eq!(moved_folder.parent, Some(destination.id));
        assert_eq!(nested.parent, Some(folder.id));
        assert_eq!(kept.parent, None);
    }

    /// `parent: None` independently places entries in the workspace root.
    #[test]
    async fn moves_to_root(state: TestState, session: Session, root: Uuid) {
        let folder = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
                CreateDirectory {
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
            *root,
            Some(folder.id),
            "leaf.txt",
            b"leaf",
        )
        .await;

        state
            .services
            .files
            .move_files(
                &ctx(state, &session.user, *root).await,
                MoveFiles {
                    ids: vec![file.id],
                    parent: None,
                },
            )
            .await
            .unwrap();

        let moved = state
            .services
            .files
            .get(&ctx(state, &session.user, *root).await, file.id)
            .await
            .unwrap();
        assert_eq!(moved.parent, None);
    }

    /// Moving a directory into its descendant independently fails (cycle).
    #[test]
    async fn cannot_move_into_itself(state: TestState, session: Session, root: Uuid) {
        let folder = state
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
                    parent: Some(folder.id),
                    name: "nested".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let err = state
            .services
            .files
            .move_files(
                &ctx(state, &session.user, *root).await,
                MoveFiles {
                    ids: vec![folder.id],
                    parent: Some(nested.id),
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Unprocessable(_)));

        let folder = state
            .services
            .files
            .get(&ctx(state, &session.user, *root).await, folder.id)
            .await
            .unwrap();
        assert_eq!(folder.parent, None);
    }

    /// Non-directory parent independently rejects the move.
    #[test]
    async fn cannot_move_into_a_file(state: TestState, session: Session, root: Uuid) {
        let target = test::upload(state, &session.user, *root, "target.txt", b"target").await;
        let file = test::upload(state, &session.user, *root, "notes.txt", b"notes").await;

        let err = state
            .services
            .files
            .move_files(
                &ctx(state, &session.user, *root).await,
                MoveFiles {
                    ids: vec![file.id],
                    parent: Some(target.id),
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Unprocessable(_)));

        let file = state
            .services
            .files
            .get(&ctx(state, &session.user, *root).await, file.id)
            .await
            .unwrap();
        assert_eq!(file.parent, None);
    }
}
