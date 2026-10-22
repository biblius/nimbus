#[suitest::suite(files_upload)]
#[suitest::suite_cfg(sequential = true)]
mod upload {
    use super::super::{ctx, user};
    use crate::{
        auth::session::Session,
        error::AppError,
        file::model::{CreateDirectory, CreateTextFile, FileKind},
        test::{self, TestState},
        user::model::CreateSharedFolder,
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
        let username = format!("upload-{}", Uuid::new_v4());
        user(state, &username).await;
        let session = test::login(state, &username, "temporary").await;
        let root = test::private_root(state, &session.user).await;
        (session, root)
    }

    /// Empty name independently fails validify (parent held absent).
    #[test]
    async fn create_directory_rejects_empty_name(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let err = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
                CreateDirectory {
                    parent: None,
                    name: "   ".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Validation(_)));
    }

    /// Slash in the name independently fails valid_name (validify held true).
    #[test]
    async fn create_directory_rejects_invalid_name(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let err = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
                CreateDirectory {
                    parent: None,
                    name: "a/b".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Unprocessable(_)));
    }

    /// Valid name and write access independently create a directory entry.
    #[test]
    async fn create_directory_success(state: TestState, session: Session, root: Uuid) {
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

        assert_eq!(folder.kind, FileKind::Directory);
        assert_eq!(folder.name, "photos");
        assert_eq!(folder.parent, None);
    }

    /// Empty name independently fails validify (parent held absent).
    #[test]
    async fn create_text_file_rejects_empty_name(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let err = state
            .services
            .files
            .create_text_file(
                &ctx(state, &session.user, *root).await,
                CreateTextFile {
                    name: "   ".into(),
                    parent: None,
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Validation(_)));
    }

    /// Valid name independently creates an empty text file at the root.
    #[test]
    async fn create_text_file_success(state: TestState, session: Session, root: Uuid) {
        let file = state
            .services
            .files
            .create_text_file(
                &ctx(state, &session.user, *root).await,
                CreateTextFile {
                    name: "notes.txt".into(),
                    parent: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(file.kind, FileKind::File);
        assert_eq!(file.name, "notes.txt");
        assert_eq!(file.parent, None);
        assert_eq!(file.size_bytes, 0);
    }

    /// Invalid staged file name independently fails valid_name in finish_upload.
    #[test]
    async fn finish_upload_rejects_invalid_file_name(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let mut writer = state
            .services
            .files
            .start_upload(
                &ctx(state, &session.user, *root).await,
                "a/b.txt",
            )
            .await
            .unwrap();
        writer.write(b"x").await.unwrap();
        let staged = writer.finish().await.unwrap();

        let err = state
            .services
            .files
            .finish_upload(
                &ctx(state, &session.user, *root).await,
                None,
                vec![],
                vec![(staged, None)],
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Unprocessable(_)));
    }

    /// finish_upload with an explicit parent places every file under that parent.
    #[test]
    async fn finish_upload_all_places_files_under_parent(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let folder = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
                CreateDirectory {
                    parent: None,
                    name: "Inbox".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let mut first = state
            .services
            .files
            .start_upload(
                &ctx(state, &session.user, *root).await,
                "a.txt",
            )
            .await
            .unwrap();
        first.write(b"a").await.unwrap();
        let first = first.finish().await.unwrap();

        let mut second = state
            .services
            .files
            .start_upload(
                &ctx(state, &session.user, *root).await,
                "b.txt",
            )
            .await
            .unwrap();
        second.write(b"b").await.unwrap();
        let second = second.finish().await.unwrap();

        let created = state
            .services
            .files
            .finish_upload(
                &ctx(state, &session.user, *root).await,
                Some(folder.id),
                vec![],
                vec![(first, Some(folder.id)), (second, Some(folder.id))],
            )
            .await
            .unwrap();

        assert_eq!(created.len(), 2);
        assert!(created.iter().all(|entry| entry.parent == Some(folder.id)));

        let listed = state
            .services
            .files
            .list(
                &ctx(state, &session.user, *root).await,
                Some(folder.id),
            )
            .await
            .unwrap();
        assert_eq!(listed.len(), 2);
        let names: Vec<_> = listed
            .iter()
            .map(|entry| entry.file.name.as_str())
            .collect();
        assert!(names.contains(&"a.txt"));
        assert!(names.contains(&"b.txt"));
    }

    /// finish_upload can create nested directories and attach staged files in one call.
    #[test]
    async fn finish_upload_all_creates_dirs_and_nested_files(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let photos = Uuid::new_v4();
        let album = Uuid::new_v4();

        let mut nested = state
            .services
            .files
            .start_upload(
                &ctx(state, &session.user, *root).await,
                "shot.txt",
            )
            .await
            .unwrap();
        nested.write(b"nested").await.unwrap();
        let nested = nested.finish().await.unwrap();

        let mut top = state
            .services
            .files
            .start_upload(
                &ctx(state, &session.user, *root).await,
                "readme.txt",
            )
            .await
            .unwrap();
        top.write(b"top").await.unwrap();
        let top = top.finish().await.unwrap();

        let created = state
            .services
            .files
            .finish_upload(
                &ctx(state, &session.user, *root).await,
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
            .list(
                &ctx(state, &session.user, *root).await,
                Some(photos),
            )
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
            .list(
                &ctx(state, &session.user, *root).await,
                Some(album),
            )
            .await
            .unwrap();
        assert_eq!(under_album.len(), 1);
        assert_eq!(under_album[0].file.name, "shot.txt");
    }

    /// Identical bytes independently share one stored blob.
    #[test]
    async fn identical_uploads_share_one_blob(state: TestState, session: Session, root: Uuid) {
        // Unique payload so earlier suite uploads do not inflate the absolute count.
        let bytes = format!("dedup-{}", Uuid::new_v4()).into_bytes();
        let before = test::blob_count(state.storage_dir());

        let stored = test::submit(
            state,
            &session.user,
            *root,
            vec![("a.txt", bytes.as_slice()), ("b.txt", bytes.as_slice())],
        )
        .await;
        let first = stored[0].as_ref().unwrap();
        let second = stored[1].as_ref().unwrap();

        assert_eq!(first.hash, second.hash);
        assert_eq!(first.created_by, Some(session.user.id));
        assert_eq!(second.created_by, Some(session.user.id));
        assert_eq!(test::blob_count(state.storage_dir()), before + 1);
    }

    /// Duplicate display names independently create distinct file and directory rows.
    #[test]
    async fn same_name_can_be_stored_more_than_once(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let stored = test::submit(
            state,
            &session.user,
            *root,
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
                &ctx(state, &session.user, *root).await,
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
                &ctx(state, &session.user, *root).await,
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
            .list(&ctx(state, &session.user, *root).await, None)
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

    /// Shared root quota independently rejects uploads that exceed the limit.
    #[test]
    async fn shared_root_enforces_quota(state: TestState) {
        let username = format!("upload-quota-{}", Uuid::new_v4());
        let ada = user(state, &username).await;
        let session = test::login(state, &username, "temporary").await;
        let shared = state
            .services
            .users
            .create_shared(CreateSharedFolder {
                name: "family".into(),
                quota_bytes: 10,
                members: vec![ada.id.into()],
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

    /// User without root assignment independently cannot upload to a shared root.
    #[test]
    async fn unassigned_user_cannot_upload(state: TestState) {
        let ada_name = format!("upload-access-ada-{}", Uuid::new_v4());
        let bob_name = format!("upload-access-bob-{}", Uuid::new_v4());
        let ada = user(state, &ada_name).await;
        let _bob = user(state, &bob_name).await;
        let shared = state
            .services
            .users
            .create_shared(CreateSharedFolder {
                name: "private-share".into(),
                quota_bytes: 0,
                members: vec![ada.id.into()],
            })
            .await
            .unwrap();

        let bob = test::login(state, &bob_name, "temporary").await;
        let forbidden = test::submit(
            state,
            &bob.user,
            shared.id,
            vec![("nope.txt", b"zzz".as_slice())],
        )
        .await;
        assert!(matches!(forbidden[0], Err(AppError::Forbidden(_))));
    }
}
