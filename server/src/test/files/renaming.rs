#[suitest::suite(files_rename)]
#[suitest::suite_cfg(sequential = true)]
mod renaming {
    use super::super::user;
    use crate::{
        error::AppError,
        file::model::{CreateDirectory, RenameFile},
        test::{self, TestState},
    };
    use suitest::before_all;

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
    }

    #[test]
    async fn renames_a_live_file(state: TestState) {
        let _ada = user(state, "ada-rename-file").await;
        let session = test::login(state, "ada-rename-file", "temporary").await;
        let root = test::private_root(state, &session.user).await;
        let file = test::upload(state, &session.user, root, "notes.txt", b"notes").await;

        state
            .services
            .files
            .rename(&super::super::ctx(state, &session.user, root).await, file.id,
                RenameFile {
                    name: "journal.txt".into(),
                },
            )
            .await
            .unwrap();

        let renamed = state
            .services
            .files
            .metadata(&super::super::ctx(state, &session.user, root).await, file.id)
            .await
            .unwrap();

        assert_eq!(renamed.id, file.id);
        assert_eq!(renamed.name, "journal.txt");
        assert_eq!(renamed.parent, file.parent);
    }

    #[test]
    async fn renames_a_directory(state: TestState) {
        let _ada = user(state, "ada-rename-dir").await;
        let session = test::login(state, "ada-rename-dir", "temporary").await;
        let root = test::private_root(state, &session.user).await;
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

        state
            .services
            .files
            .rename(&super::super::ctx(state, &session.user, root).await, folder.id,
                RenameFile {
                    name: "albums".into(),
                },
            )
            .await
            .unwrap();

        let renamed = state
            .services
            .files
            .metadata(&super::super::ctx(state, &session.user, root).await, folder.id)
            .await
            .unwrap();

        assert_eq!(renamed.id, folder.id);
        assert_eq!(renamed.name, "albums");
    }

    #[test]
    async fn rejects_empty_names(state: TestState) {
        let _ada = user(state, "ada-rename-empty").await;
        let session = test::login(state, "ada-rename-empty", "temporary").await;
        let root = test::private_root(state, &session.user).await;
        let file = test::upload(state, &session.user, root, "notes.txt", b"notes").await;

        let err = state
            .services
            .files
            .rename(&super::super::ctx(state, &session.user, root).await, file.id,
                RenameFile { name: "   ".into() },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Validation(_)));
    }
}
