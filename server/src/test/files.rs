#[suitest::suite(files_service)]
#[suitest::suite_cfg(sequential = true)]
mod files_service {
    use crate::{
        error::AppError,
        test::{self, TestState},
        user::model::{CreateSharedFolder, CreateUser, Role, SetQuota, UpdateRoot},
    };
    use chrono::{Duration, Utc};
    use suitest::before_all;

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
