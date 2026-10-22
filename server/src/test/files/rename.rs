#[suitest::suite(files_rename)]
#[suitest::suite_cfg(sequential = true)]
mod rename {
    use super::super::{ctx, user};
    use crate::{
        auth::session::Session,
        error::AppError,
        file::model::{CreateDirectory, RenameFile},
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
        let username = format!("rename-{}", Uuid::new_v4());
        user(state, &username).await;
        let session = test::login(state, &username, "temporary").await;
        let root = test::private_root(state, &session.user).await;
        (session, root)
    }

    /// Empty name independently fails validify (file held present).
    #[test]
    async fn rejects_empty_name(state: TestState, session: Session, root: Uuid) {
        let file = test::upload(state, &session.user, *root, "notes.txt", b"notes").await;

        let err = state
            .services
            .files
            .rename(
                &ctx(state, &session.user, *root).await,
                file.id,
                RenameFile { name: "   ".into() },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Validation(_)));
    }

    /// Slash in the name independently fails valid_name (validify held true).
    #[test]
    async fn rejects_invalid_name(state: TestState, session: Session, root: Uuid) {
        let file = test::upload(state, &session.user, *root, "notes.txt", b"notes").await;

        let err = state
            .services
            .files
            .rename(
                &ctx(state, &session.user, *root).await,
                file.id,
                RenameFile {
                    name: "a/b.txt".into(),
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Unprocessable(_)));
    }

    /// Missing live file independently yields ResourceMissing.
    #[test]
    async fn missing_file_returns_resource_missing(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let err = state
            .services
            .files
            .rename(
                &ctx(state, &session.user, *root).await,
                Uuid::new_v4(),
                RenameFile {
                    name: "gone.txt".into(),
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::ResourceMissing(_)));
    }

    /// Same name independently no-ops without error.
    #[test]
    async fn same_name_is_no_op(state: TestState, session: Session, root: Uuid) {
        let file = test::upload(state, &session.user, *root, "notes.txt", b"notes").await;

        state
            .services
            .files
            .rename(
                &ctx(state, &session.user, *root).await,
                file.id,
                RenameFile {
                    name: "notes.txt".into(),
                },
            )
            .await
            .unwrap();

        let same = state
            .services
            .files
            .get(&ctx(state, &session.user, *root).await, file.id)
            .await
            .unwrap();
        assert_eq!(same.name, "notes.txt");
        assert_eq!(same.hash, file.hash);
    }

    /// Renaming a live file updates only the display name.
    #[test]
    async fn renames_a_live_file(state: TestState, session: Session, root: Uuid) {
        let file = test::upload(state, &session.user, *root, "notes.txt", b"notes").await;

        state
            .services
            .files
            .rename(
                &ctx(state, &session.user, *root).await,
                file.id,
                RenameFile {
                    name: "journal.txt".into(),
                },
            )
            .await
            .unwrap();

        let renamed = state
            .services
            .files
            .get(&ctx(state, &session.user, *root).await, file.id)
            .await
            .unwrap();

        assert_eq!(renamed.id, file.id);
        assert_eq!(renamed.name, "journal.txt");
        assert_eq!(renamed.parent, file.parent);
    }

    /// Renaming a directory uses the same path as a file.
    #[test]
    async fn renames_a_directory(state: TestState, session: Session, root: Uuid) {
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

        state
            .services
            .files
            .rename(
                &ctx(state, &session.user, *root).await,
                folder.id,
                RenameFile {
                    name: "albums".into(),
                },
            )
            .await
            .unwrap();

        let renamed = state
            .services
            .files
            .get(&ctx(state, &session.user, *root).await, folder.id)
            .await
            .unwrap();

        assert_eq!(renamed.id, folder.id);
        assert_eq!(renamed.name, "albums");
    }
}
