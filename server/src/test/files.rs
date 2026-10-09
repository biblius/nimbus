#[suitest::suite(files_service)]
#[suitest::suite_cfg(sequential = true)]
mod files_service {
    use crate::{
        error::{AppError, AuthFailure},
        file::model::CreateDirectory,
        test::{self, TestState},
        user::model::{CreateSharedFolder, CreateUser, Role, SetQuota, UpdateRoot, User},
    };
    use chrono::{Duration, Utc};
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
        assert_eq!(first.created_by, ada.id);
        assert_eq!(second.created_by, ada.id);
        assert_eq!(test::blob_count(state.storage_dir()), 1);
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
        assert_eq!(stored.created_by, ada.id);

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
    async fn deleting_a_user_removes_their_files(state: TestState) {
        let _bob = user(state, "bob-delete").await;
        let bob = test::login(state, "bob-delete", "temporary").await;
        let bob_root = test::private_root(state, &bob.user).await;
        let shared_bytes = b"kept-because-bob-has-a-copy";
        let bob_file = test::upload(state, &bob.user, bob_root, "kept.txt", shared_bytes).await;

        let ada = user(state, "ada-delete").await;
        let session = test::login(state, "ada-delete", "temporary").await;
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
        let nested = upload_in(
            state,
            &session.user,
            root,
            Some(notes.id),
            "nested.txt",
            b"ada-nested",
        )
        .await;
        let top = test::upload(state, &session.user, root, "top.txt", b"ada-only").await;
        let copy = test::upload(state, &session.user, root, "copy.txt", shared_bytes).await;
        let blobs_with_ada = test::blob_count(state.storage_dir());

        let empty = user(state, "empty-delete").await;
        let empty_root = test::private_root(
            state,
            &test::login(state, "empty-delete", "temporary").await.user,
        )
        .await;

        state.services.users.delete_user(ada.id).await.unwrap();

        for id in [notes.id, nested.id, top.id, copy.id] {
            assert!(matches!(
                state.services.files.metadata(&session.user, id).await,
                Err(AppError::ResourceMissing(_))
            ));
        }
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM files WHERE root_id = $1", root),
            0
        );
        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM files WHERE created_by = $1",
                ada.id
            ),
            0
        );
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM roots WHERE id = $1", root),
            0
        );
        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM user_folders WHERE user_id = $1",
                ada.id
            ),
            0
        );
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM users WHERE id = $1", ada.id),
            0
        );
        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM sessions WHERE user_id = $1",
                ada.id
            ),
            0
        );
        assert!(matches!(
            state.services.auth.get_session_by_id(session.id).await,
            Err(AppError::Auth(AuthFailure::SessionNotFound))
        ));
        // Bob still references the shared bytes, so that blob stays.
        assert_eq!(test::blob_count(state.storage_dir()), blobs_with_ada - 2);
        let kept = state
            .services
            .files
            .content(&bob.user, bob_file.id)
            .await
            .unwrap();
        assert_eq!(kept.0.id, bob_file.id);

        state.services.users.delete_user(empty.id).await.unwrap();
        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM roots WHERE id = $1",
                empty_root
            ),
            0
        );
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM users WHERE id = $1", empty.id),
            0
        );

        let missing = state.services.users.delete_user(Uuid::new_v4()).await;
        assert!(matches!(missing, Err(AppError::ResourceMissing(_))));
    }

    #[test]
    async fn the_last_admin_cannot_be_deleted(state: TestState) {
        let admin = test::login(state, "admin", "admin").await;
        let root = test::private_root(state, &admin.user).await;
        let file = test::upload(state, &admin.user, root, "admin.txt", b"admin-file").await;

        let forbidden = state.services.users.delete_user(admin.user.id).await;
        assert!(matches!(forbidden, Err(AppError::Forbidden(_))));
        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM users WHERE id = $1",
                admin.user.id
            ),
            1
        );
        let still_there = state
            .services
            .files
            .content(&admin.user, file.id)
            .await
            .unwrap();
        assert_eq!(still_there.0.id, file.id);

        let extra = state
            .services
            .users
            .create_user(CreateUser {
                username: "second-admin".into(),
                password: "temporary".into(),
                email: None,
                role: Role::Admin,
            })
            .await
            .unwrap();
        let extra_session = test::login(state, "second-admin", "temporary").await;
        let extra_root = test::private_root(state, &extra_session.user).await;
        let extra_file = test::upload(
            state,
            &extra_session.user,
            extra_root,
            "extra.txt",
            b"extra-admin",
        )
        .await;

        state.services.users.delete_user(extra.id).await.unwrap();
        assert!(matches!(
            state
                .services
                .files
                .metadata(&extra_session.user, extra_file.id)
                .await,
            Err(AppError::ResourceMissing(_))
        ));
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM users WHERE id = $1", extra.id),
            0
        );
        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM users WHERE id = $1",
                admin.user.id
            ),
            1
        );
    }

    #[test]
    async fn admin_can_rename_a_root_and_change_its_quota(state: TestState) {
        let ada = user(state, "ada-rename").await;
        let session = test::login(state, "ada-rename", "temporary").await;
        let shared = state
            .services
            .users
            .create_shared(CreateSharedFolder {
                name: "old-name".into(),
                quota_bytes: 10,
                members: vec![ada.id],
            })
            .await
            .unwrap();

        state
            .services
            .users
            .set_quota(shared.id, SetQuota { quota_bytes: 32 })
            .await
            .unwrap();

        let after_quota = state
            .services
            .files
            .list_roots(&session.user)
            .await
            .unwrap();
        let limited = after_quota
            .iter()
            .find(|root| root.id == shared.id)
            .unwrap();
        assert_eq!(limited.quota, 32);

        let renamed = state
            .services
            .users
            .update_root(
                shared.id,
                UpdateRoot {
                    name: "new-name".into(),
                    quota_bytes: 64,
                    members: vec![ada.id],
                },
            )
            .await
            .unwrap();
        assert_eq!(renamed.name, "new-name");
        assert_eq!(renamed.quota, 64);

        let roots = state
            .services
            .files
            .list_roots(&session.user)
            .await
            .unwrap();
        let folder = roots.iter().find(|root| root.id == shared.id).unwrap();
        assert_eq!(folder.name, "new-name");
        assert_eq!(folder.quota, 64);
    }

    #[test]
    async fn private_root_cannot_be_shared(state: TestState) {
        let ada = user(state, "ada-private-share").await;
        let bob = user(state, "bob-private-share").await;
        let session = test::login(state, "ada-private-share", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let assigned = state.services.users.assign(root, bob.id).await;
        assert!(matches!(assigned, Err(AppError::Forbidden(_))));

        let updated = state
            .services
            .users
            .update_root(
                root,
                UpdateRoot {
                    name: "shared-attempt".into(),
                    quota_bytes: 0,
                    members: vec![ada.id, bob.id],
                },
            )
            .await;
        assert!(matches!(updated, Err(AppError::Forbidden(_))));

        let members = state
            .services
            .users
            .list_members(&session.user, root)
            .await
            .unwrap();
        assert!(members.is_empty());
        assert_eq!(
            count!(
                state,
                "SELECT COUNT(*) FROM user_folders WHERE root_id = $1",
                root
            ),
            0
        );

        let renamed = state
            .services
            .users
            .update_root(
                root,
                UpdateRoot {
                    name: "ada-renamed".into(),
                    quota_bytes: 128,
                    members: vec![],
                },
            )
            .await
            .unwrap();
        assert_eq!(renamed.name, "ada-renamed");
        assert_eq!(renamed.quota, 128);
        assert_eq!(renamed.owner, Some(ada.id));

        let deleted = state.services.users.delete_root(root).await;
        assert!(matches!(deleted, Err(AppError::Forbidden(_))));
        assert_eq!(
            count!(state, "SELECT COUNT(*) FROM roots WHERE id = $1", root),
            1
        );
    }

    async fn upload_in(
        state: &TestState,
        user: &User,
        root: Uuid,
        parent: Option<Uuid>,
        name: &str,
        bytes: &[u8],
    ) -> crate::file::model::FileEntry {
        let mut writer = state
            .services
            .files
            .start_upload(user, root, parent, name.to_string())
            .await
            .unwrap();
        writer.write(bytes).await.unwrap();
        let file = writer.finish().await.unwrap();
        state
            .services
            .files
            .finish_upload(user, root, parent, file)
            .await
            .unwrap()
    }

    macro_rules! count {
        ($state:expr, $sql:literal, $id:expr) => {
            sqlx::query_scalar::<_, i64>($sql)
                .bind($id)
                .fetch_one(&$state.pool)
                .await
                .unwrap()
        };
    }

    async fn user(state: &TestState, username: &str) -> crate::user::model::UserViewFull {
        state
            .services
            .users
            .create_user(CreateUser {
                username: username.into(),
                password: "temporary".into(),
                email: None,
                role: Role::User,
            })
            .await
            .unwrap()
    }
}
