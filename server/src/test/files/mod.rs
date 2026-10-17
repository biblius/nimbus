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
    file::model::FileEntry,
    test::TestState,
    user::model::{CreateUser, Role, User},
};
use uuid::Uuid;

pub(super) async fn upload_in(
    state: &TestState,
    user: &User,
    root: Uuid,
    parent: Option<Uuid>,
    name: &str,
    bytes: &[u8],
) -> FileEntry {
    let mut writer = state
        .services
        .files
        .start_upload(user, root, name.to_string())
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
