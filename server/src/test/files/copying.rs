#[suitest::suite(files_copy)]
#[suitest::suite_cfg(sequential = true)]
mod copying {
    use super::super::{upload_in, user};
    use crate::{
        error::AppError,
        file::model::{CopyFile, CreateDirectory, FileKind},
        test::{self, TestState},
        user::model::CreateSharedFolder,
    };
    use suitest::before_all;

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
    }

    #[test]
    async fn copies_file_into_another_root_with_new_id_and_created_at(state: TestState) {
        let ada = user(state, "ada-copy").await;
        let session = test::login(state, "ada-copy", "temporary").await;
        let private = test::private_root(state, &session.user).await;
        let shared = state
            .services
            .users
            .create_shared(CreateSharedFolder {
                name: "team-copy".into(),
                quota_bytes: 0,
                members: vec![ada.id],
            })
            .await
            .unwrap();

        let source = test::upload(state, &session.user, shared.id, "notes.txt", b"notes").await;

        state
            .services
            .files
            .copy_file(
                &session.user,
                shared.id,
                CopyFile {
                    id: source.id,
                    target_root_id: private,
                    parent: None,
                },
            )
            .await
            .unwrap();

        let listed = state
            .services
            .files
            .list(&session.user, private, None)
            .await
            .unwrap();
        let copied = listed
            .into_iter()
            .find(|entry| entry.file.name == "notes.txt")
            .expect("copied file");

        assert_ne!(copied.file.id, source.id);
        assert_eq!(copied.file.root_id, private);
        assert_eq!(copied.file.parent, None);
        assert_eq!(copied.file.hash, source.hash);
        assert_eq!(copied.file.size_bytes, source.size_bytes);
        assert_eq!(copied.file.file_type, source.file_type);
        assert_eq!(copied.file.created_by, source.created_by);
        assert_eq!(copied.metadata, None);
        assert!(copied.file.created_at >= source.created_at);

        let original = state
            .services
            .files
            .metadata(&session.user, source.id)
            .await
            .unwrap();
        assert_eq!(original.root_id, shared.id);
    }

    #[test]
    async fn copies_directory_with_descendants(state: TestState) {
        let ada = user(state, "ada-copy-tree").await;
        let session = test::login(state, "ada-copy-tree", "temporary").await;
        let private = test::private_root(state, &session.user).await;
        let shared = state
            .services
            .users
            .create_shared(CreateSharedFolder {
                name: "tree-copy".into(),
                quota_bytes: 0,
                members: vec![ada.id],
            })
            .await
            .unwrap();

        let folder = state
            .services
            .files
            .create_directory(
                &session.user,
                shared.id,
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
            shared.id,
            Some(folder.id),
            "inside.txt",
            b"inside",
        )
        .await;

        state
            .services
            .files
            .copy_file(
                &session.user,
                shared.id,
                CopyFile {
                    id: folder.id,
                    target_root_id: private,
                    parent: None,
                },
            )
            .await
            .unwrap();

        let listed = state
            .services
            .files
            .list(&session.user, private, None)
            .await
            .unwrap();
        let copied_folder = listed
            .into_iter()
            .find(|entry| entry.file.name == "photos")
            .expect("copied folder");
        assert_eq!(copied_folder.file.kind, FileKind::Directory);
        assert_ne!(copied_folder.file.id, folder.id);

        let children = state
            .services
            .files
            .list(&session.user, private, Some(copied_folder.file.id))
            .await
            .unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].file.name, "inside.txt");
        assert_ne!(children[0].file.id, nested.id);
        assert_eq!(children[0].file.hash, nested.hash);
    }

    #[test]
    async fn copies_into_a_target_directory(state: TestState) {
        let _ada = user(state, "ada-copy-parent").await;
        let session = test::login(state, "ada-copy-parent", "temporary").await;
        let private = test::private_root(state, &session.user).await;

        let destination = state
            .services
            .files
            .create_directory(
                &session.user,
                private,
                CreateDirectory {
                    parent: None,
                    name: "archive".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let file = test::upload(state, &session.user, private, "memo.txt", b"memo").await;

        state
            .services
            .files
            .copy_file(
                &session.user,
                private,
                CopyFile {
                    id: file.id,
                    target_root_id: private,
                    parent: Some(destination.id),
                },
            )
            .await
            .unwrap();

        let children = state
            .services
            .files
            .list(&session.user, private, Some(destination.id))
            .await
            .unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].file.name, "memo.txt");
        assert_ne!(children[0].file.id, file.id);
    }

    #[test]
    async fn rejects_copy_without_access(state: TestState) {
        let ada = user(state, "ada-copy-forbidden").await;
        let _bob = user(state, "bob-copy-forbidden").await;
        let session = test::login(state, "ada-copy-forbidden", "temporary").await;
        let private = test::private_root(state, &session.user).await;
        let shared = state
            .services
            .users
            .create_shared(CreateSharedFolder {
                name: "locked-copy".into(),
                quota_bytes: 0,
                members: vec![ada.id],
            })
            .await
            .unwrap();
        let file = test::upload(state, &session.user, shared.id, "secret.txt", b"x").await;

        let bob = test::login(state, "bob-copy-forbidden", "temporary").await;
        let bob_private = test::private_root(state, &bob.user).await;

        let forbidden = state
            .services
            .files
            .copy_file(
                &bob.user,
                shared.id,
                CopyFile {
                    id: file.id,
                    target_root_id: bob_private,
                    parent: None,
                },
            )
            .await;
        assert!(matches!(forbidden, Err(AppError::Forbidden(_))));

        let still_forbidden = state
            .services
            .files
            .copy_file(
                &bob.user,
                private,
                CopyFile {
                    id: file.id,
                    target_root_id: bob_private,
                    parent: None,
                },
            )
            .await;
        assert!(matches!(
            still_forbidden,
            Err(AppError::Forbidden(_) | AppError::ResourceMissing(_))
        ));
    }

    #[test]
    async fn enforces_quota_on_the_target_root(state: TestState) {
        let ada = user(state, "ada-copy-quota").await;
        let session = test::login(state, "ada-copy-quota", "temporary").await;
        let private = test::private_root(state, &session.user).await;
        let shared = state
            .services
            .users
            .create_shared(CreateSharedFolder {
                name: "tiny-copy".into(),
                quota_bytes: 4,
                members: vec![ada.id],
            })
            .await
            .unwrap();

        let file = test::upload(state, &session.user, private, "big.txt", b"12345").await;

        let overflow = state
            .services
            .files
            .copy_file(
                &session.user,
                private,
                CopyFile {
                    id: file.id,
                    target_root_id: shared.id,
                    parent: None,
                },
            )
            .await;
        assert!(matches!(overflow, Err(AppError::QuotaExceeded)));
    }
}
