mod live;
mod share;
mod trash;

pub use live::LiveFileService;
pub use share::ShareService;
pub use trash::TrashFileService;

use crate::{
    db::user_folders::UserFolderRepository,
    error::AppError,
    user::model::{Role, User},
};
use uuid::Uuid;

pub(crate) async fn accessible(
    folders: &UserFolderRepository,
    user: &User,
    root_id: Uuid,
) -> Result<(), AppError> {
    if user.role != Role::Admin && !folders.can_access(user.id, root_id).await? {
        return Err(AppError::Forbidden(
            "You do not have access to this folder".into(),
        ));
    }
    Ok(())
}
