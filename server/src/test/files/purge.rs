#[suitest::suite(files_purge)]
#[suitest::suite_cfg(sequential = true)]
mod purge {
    use super::super::{upload_in, user};
    use crate::{
        error::AppError,
        file::model::CreateDirectory,
        test::{self, TestState},
    };
    use chrono::{Duration, Utc};
    use suitest::before_all;

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
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
}
