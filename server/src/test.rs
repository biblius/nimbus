#![allow(unused_imports)]

use chrono::{Duration, Utc};
use uuid::Uuid;

use crate::{
    auth::{
        model::{LoginCredentials, LoginResult, PasswordChange},
        service::AuthenticationService,
        token::TokenStore,
    },
    cache::MemoryCache,
    db::{
        self, FileRepository, SessionRepository, Transactor, UserFolderRepository, UserRepository,
    },
    email::EmailSender,
    file::{service::FileService, storage::Storage},
    user::{
        model::{CreateSharedFolder, CreateUser, Role},
        service::UserService,
    },
};

#[tokio::test]
async fn memory_cache_roundtrip() {
    let cache = MemoryCache::new();
    cache.set_ex("k", "v", 60).await.unwrap();
    assert_eq!(cache.get_del("k").await.unwrap().as_deref(), Some("v"));
    assert_eq!(cache.get_del("k").await.unwrap(), None);
}

#[cfg(all(feature = "sqlite", feature = "memory-cache"))]
struct Harness {
    auth: AuthenticationService,
    users: UserService,
    files: FileService,
    dir: std::path::PathBuf,
}

#[cfg(all(feature = "sqlite", feature = "memory-cache"))]
impl Harness {
    async fn new() -> Self {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .with_test_writer()
            .try_init();
        let dir = std::env::temp_dir().join(format!("nimbus-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(dir.join("db")).await.unwrap();
        let url = format!(
            "sqlite://{}?mode=rwc",
            dir.join("db").join("nimbus.db").display()
        );
        let pool = db::connect(&url).await.unwrap();
        db::migrate(&pool).await.unwrap();

        let cache = MemoryCache::new();
        let email = EmailSender::new(
            String::new(),
            String::new(),
            "Nimbus".into(),
            "http://localhost".into(),
        );
        let users = UserRepository::new(pool.clone());
        let sessions = SessionRepository::new(pool.clone());
        let folders = UserFolderRepository::new(pool.clone());
        let files_repo = FileRepository::new(pool.clone());
        let auth = AuthenticationService::new(
            users.clone(),
            sessions.clone(),
            TokenStore::new(cache),
            email.clone(),
        );
        let user_service = UserService::new(
            users,
            files_repo.clone(),
            folders.clone(),
            sessions,
            email,
            Transactor::new(pool.clone()),
        );
        let storage = Storage::new(dir.join("storage"), 1024 * 1024);
        let files = FileService::new(files_repo, folders, storage, 30, Transactor::new(pool));
        files.prepare().await.unwrap();
        user_service.ensure_initial_admin().await.unwrap();

        Self {
            auth,
            users: user_service,
            files,
            dir,
        }
    }
}

#[cfg(all(feature = "sqlite", feature = "memory-cache"))]
impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[cfg(all(feature = "sqlite", feature = "memory-cache"))]
#[tokio::test]
async fn admin_seed_groups_quota_and_dedup() {
    let harness = Harness::new().await;

    let login = harness
        .auth
        .login(
            LoginCredentials {
                username: "admin".into(),
                password: "admin".into(),
                remember: false,
            },
            Some("127.0.0.1".into()),
            None,
        )
        .await
        .unwrap();
    let LoginResult::Session { session } = login else {
        panic!("expected a session");
    };
    assert_eq!(session.user.role, Role::Admin);
    assert!(session.first_login);
    let admin_session = harness.auth.get_session_by_id(session.id).await.unwrap();
    assert_eq!(admin_session.user.role, Role::Admin);
    assert!(admin_session.user.password.is_some());

    let ada = harness
        .users
        .create_user(CreateUser {
            username: "ada".into(),
            password: "temporary".into(),
            email: None,
            role: Role::User,
        })
        .await
        .unwrap();
    let _bob = harness
        .users
        .create_user(CreateUser {
            username: "bob".into(),
            password: "temporary".into(),
            email: None,
            role: Role::User,
        })
        .await
        .unwrap();

    let shared_root = harness
        .users
        .create_shared(CreateSharedFolder {
            name: "family".into(),
            quota_bytes: 10,
            members: vec![ada.id],
        })
        .await
        .unwrap();

    let ada_login = harness
        .auth
        .login(
            LoginCredentials {
                username: "ada".into(),
                password: "temporary".into(),
                remember: true,
            },
            None,
            None,
        )
        .await
        .unwrap();
    let LoginResult::Session { session: ada_view } = ada_login else {
        panic!("expected ada session");
    };
    let ada_session = harness.auth.get_session_by_id(ada_view.id).await.unwrap();

    let private_root = harness
        .files
        .folders(&ada_session.user)
        .await
        .unwrap()
        .private_folder
        .root
        .id;
    let stored = submit(
        &harness,
        &ada_session.user,
        private_root,
        vec![("a.txt", b"abc".as_slice()), ("b.txt", b"abc".as_slice())],
    )
    .await;
    let first = stored[0].as_ref().unwrap();
    let second = stored[1].as_ref().unwrap();
    assert_eq!(first.hash, second.hash);
    assert_eq!(blob_count(&harness.dir), 1);

    let private_extra = upload(&harness, &ada_session.user, private_root, "c.txt", b"12345").await;
    assert!(private_extra.hash.is_some());
    assert_eq!(blob_count(&harness.dir), 2);

    let shared = upload(
        &harness,
        &ada_session.user,
        shared_root.id,
        "shared.txt",
        b"abc",
    )
    .await;
    assert_eq!(shared.hash, first.hash);
    assert_eq!(blob_count(&harness.dir), 2);

    let overflow = submit(
        &harness,
        &ada_session.user,
        shared_root.id,
        vec![("big.txt", b"12345678".as_slice())],
    )
    .await;
    assert!(matches!(
        overflow[0],
        Err(crate::error::AppError::QuotaExceeded)
    ));

    let bob_login = harness
        .auth
        .login(
            LoginCredentials {
                username: "bob".into(),
                password: "temporary".into(),
                remember: false,
            },
            None,
            None,
        )
        .await
        .unwrap();
    let LoginResult::Session { session: bob_view } = bob_login else {
        panic!("expected bob session");
    };
    let bob_session = harness.auth.get_session_by_id(bob_view.id).await.unwrap();
    let forbidden = submit(
        &harness,
        &bob_session.user,
        shared_root.id,
        vec![("nope.txt", b"zzz".as_slice())],
    )
    .await;
    assert!(matches!(
        forbidden[0],
        Err(crate::error::AppError::Forbidden(_))
    ));

    harness
        .files
        .delete(&ada_session.user, first.id)
        .await
        .unwrap();
    harness
        .files
        .delete(&ada_session.user, private_extra.id)
        .await
        .unwrap();
    assert!(matches!(
        harness.files.content(&ada_session.user, first.id).await,
        Err(crate::error::AppError::ResourceMissing(_))
    ));
    assert_eq!(blob_count(&harness.dir), 2);

    harness
        .files
        .delete(&ada_session.user, second.id)
        .await
        .unwrap();
    harness
        .files
        .delete(&ada_session.user, shared.id)
        .await
        .unwrap();
    harness
        .files
        .purge_deleted_before(Utc::now() + Duration::seconds(5))
        .await
        .unwrap();
    assert_eq!(blob_count(&harness.dir), 0);

    let weak = harness
        .auth
        .change_password(
            ada_session,
            PasswordChange {
                current_password: "temporary".into(),
                new_password: "admin".into(),
            },
        )
        .await;
    assert!(matches!(weak, Err(crate::error::AppError::Validation(_))));
}

#[cfg(all(feature = "sqlite", feature = "memory-cache"))]
async fn upload(
    harness: &Harness,
    user: &crate::user::model::User,
    parent: Uuid,
    name: &str,
    bytes: &[u8],
) -> crate::file::model::FileEntry {
    let stored = submit(harness, user, parent, vec![(name, bytes)]).await;
    stored.into_iter().next().unwrap().unwrap()
}

#[cfg(all(feature = "sqlite", feature = "memory-cache"))]
async fn submit(
    harness: &Harness,
    user: &crate::user::model::User,
    parent: Uuid,
    files: Vec<(&str, &[u8])>,
) -> Vec<Result<crate::file::model::FileEntry, crate::error::AppError>> {
    let mut stored = Vec::with_capacity(files.len());
    for (name, bytes) in files {
        let mut writer = harness.files.start_upload(name.to_string()).await.unwrap();
        writer.write(bytes).await.unwrap();
        let file = writer.finish().await.unwrap();
        stored.push(harness.files.finish_upload(user, parent, file).await);
    }
    stored
}

#[cfg(all(feature = "sqlite", feature = "memory-cache"))]
fn blob_count(dir: &std::path::Path) -> usize {
    let root = dir.join("storage").join("data");
    if !root.exists() {
        return 0;
    }
    walk(&root)
}

#[cfg(all(feature = "sqlite", feature = "memory-cache"))]
fn walk(path: &std::path::Path) -> usize {
    let mut count = 0;
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            count += walk(&entry.path());
        } else {
            count += 1;
        }
    }
    count
}
