#[suitest::suite(files_upload)]
#[suitest::suite_cfg(sequential = true)]
mod upload {
    use super::super::user;
    use crate::{
        error::AppError,
        file::model::{CreateDirectory, FileKind},
        test::{self, TestState},
        user::model::CreateSharedFolder,
    };
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
        assert_eq!(first.created_by, Some(ada.id));
        assert_eq!(second.created_by, Some(ada.id));
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
                    ..Default::default()
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
                    ..Default::default()
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
                .filter(|entry| entry.file.name == "same.txt")
                .count(),
            2
        );
        assert_eq!(
            listed
                .iter()
                .filter(|entry| entry.file.name == "notes")
                .count(),
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
        assert_eq!(stored.created_by, Some(ada.id));

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
    async fn finish_upload_all_creates_dirs_and_nested_files(state: TestState) {
        let _ada = user(state, "ada-dir-upload").await;
        let session = test::login(state, "ada-dir-upload", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let photos = Uuid::new_v4();
        let album = Uuid::new_v4();

        let mut nested = state
            .services
            .files
            .start_upload(&session.user, root, "shot.txt".into())
            .await
            .unwrap();
        nested.write(b"nested").await.unwrap();
        let nested = nested.finish().await.unwrap();

        let mut top = state
            .services
            .files
            .start_upload(&session.user, root, "readme.txt".into())
            .await
            .unwrap();
        top.write(b"top").await.unwrap();
        let top = top.finish().await.unwrap();

        let created = state
            .services
            .files
            .finish_upload_all(
                &session.user,
                root,
                None,
                vec![
                    CreateDirectory::new(photos, "Photos".into(), None),
                    CreateDirectory::new(album, "Album".into(), Some(photos)),
                ],
                vec![(nested, Some(album)), (top, Some(photos))],
            )
            .await
            .unwrap();

        assert_eq!(created.len(), 4);
        assert!(
            created
                .iter()
                .any(|entry| entry.kind == FileKind::Directory && entry.name == "Photos")
        );
        assert!(created.iter().any(|entry| entry.kind == FileKind::Directory
            && entry.name == "Album"
            && entry.parent == Some(photos)));

        let under_photos = state
            .services
            .files
            .list(&session.user, root, Some(photos))
            .await
            .unwrap();
        assert_eq!(under_photos.len(), 2);
        assert!(
            under_photos
                .iter()
                .any(|entry| entry.file.name == "Album" && entry.file.kind == FileKind::Directory)
        );
        assert!(
            under_photos
                .iter()
                .any(|entry| entry.file.name == "readme.txt" && entry.file.kind == FileKind::File)
        );

        let under_album = state
            .services
            .files
            .list(&session.user, root, Some(album))
            .await
            .unwrap();
        assert_eq!(under_album.len(), 1);
        assert_eq!(under_album[0].file.name, "shot.txt");
    }
}
