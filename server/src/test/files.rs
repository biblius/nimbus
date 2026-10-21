mod access;
mod copying;
mod deletion;
mod moving;
mod overwrite;
mod processing;
mod renaming;
mod roots;
mod share;
mod trash;
mod tree;
mod upload;

use crate::{
    db::UserRootRepository,
    error::AppError,
    file::{
        AccessMode, UserRootContext,
        model::{FileEntry, RootAssignment},
    },
    test::TestState,
    user::model::{CreateSharedFolder, CreateUser, Role, User},
};
use uuid::Uuid;

pub(super) async fn ctx(state: &TestState, user: &User, root: Uuid) -> UserRootContext {
    state
        .services
        .files
        .accessible_root(user, root)
        .await
        .unwrap()
}

/// Create a shared root whose initial members get [`AccessMode::DEFAULT`].
pub(super) async fn shared_root(state: &TestState, name: &str, members: &[Uuid]) -> Uuid {
    state
        .services
        .users
        .create_shared(CreateSharedFolder {
            name: name.into(),
            quota_bytes: 0,
            members: members.iter().copied().map(Into::into).collect(),
        })
        .await
        .unwrap()
        .id
}

/// Insert or keep a membership row with the given access mode.
pub(super) async fn grant(
    state: &TestState,
    root_id: Uuid,
    user_id: Uuid,
    access_mode: AccessMode,
) {
    UserRootRepository::new(state.pool.clone())
        .assign(RootAssignment {
            root_id,
            user_id,
            access_mode,
        })
        .await
        .unwrap();

    // `assign` is insert-only (`ON CONFLICT DO NOTHING`); force the mode.
    sqlx::query("UPDATE user_roots SET access_mode = $1 WHERE root_id = $2 AND user_id = $3")
        .bind(access_mode)
        .bind(root_id)
        .bind(user_id)
        .execute(&state.pool)
        .await
        .unwrap();
}

pub(super) fn assert_forbidden<T: std::fmt::Debug>(result: Result<T, AppError>) {
    assert!(
        matches!(result, Err(AppError::Forbidden(_))),
        "expected Forbidden, got {result:?}"
    );
}

pub(super) async fn upload_in(
    state: &TestState,
    user: &User,
    root: Uuid,
    parent: Option<Uuid>,
    name: &str,
    bytes: &[u8],
) -> FileEntry {
    let root_ctx = ctx(state, user, root).await;

    let mut writer = state
        .services
        .files
        .start_upload(&root_ctx, name)
        .await
        .unwrap();
    writer.write(bytes).await.unwrap();
    let file = writer.finish().await.unwrap();
    state
        .services
        .files
        .finish_upload(&root_ctx, parent, vec![], vec![(file, parent)])
        .await
        .unwrap()
        .into_iter()
        .next()
        .expect("finish_upload returns the uploaded file")
}

macro_rules! count {
    ($state:expr, $sql:literal, $($id:expr),+ $(,)?) => {
        sqlx::query_scalar::<_, i64>($sql)
            $(.bind($id))+
            .fetch_one(&$state.pool)
            .await
            .unwrap()
    };
}
pub(super) use count;

pub(super) async fn user(state: &TestState, username: &str) -> crate::user::model::UserView {
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
