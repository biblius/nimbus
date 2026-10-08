use crate::{
    auth::crypto::hash_password,
    db::{
        Transactor,
        files::{FileRepository, NewRoot},
        sessions::SessionRepository,
        user_folders::UserFolderRepository,
        users::UserRepository,
    },
    email::EmailSender,
    error::AppError,
    file::{
        model::{FileEntry, Root, RootAssignment},
        service::FileService,
        storage::Storage,
    },
    user::model::{
        CreateSharedFolder, CreateUser, Role, SetQuota, UpdateRoot, UpdateUser, User, UserView,
        UserViewFull,
    },
};
use tracing::instrument;
use uuid::Uuid;
use validify::{Validate, Validify};

#[derive(Clone)]
pub struct UserService {
    users: UserRepository,
    files: FileRepository,
    folders: UserFolderRepository,
    sessions: SessionRepository,
    email: EmailSender,
    tx: Transactor,
    storage: Storage,
}

impl UserService {
    pub fn new(
        users: UserRepository,
        files: FileRepository,
        folders: UserFolderRepository,
        sessions: SessionRepository,
        email: EmailSender,
        tx: Transactor,
        storage: Storage,
    ) -> Self {
        Self {
            users,
            files,
            folders,
            sessions,
            email,
            tx,
            storage,
        }
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn ensure_initial_admin(&self) -> Result<(), AppError> {
        if self.users.count().await? > 0 {
            return Ok(());
        }

        tracing::warn!("creating initial admin user 'admin'; change this password");

        self.create_user(CreateUser {
            username: "admin".into(),
            password: "admin".into(),
            email: None,
            role: Role::Admin,
        })
        .await?;

        Ok(())
    }

    #[instrument(skip(self, input), fields(username = %input.username), err(Debug))]
    pub async fn create_user(&self, mut input: CreateUser) -> Result<UserViewFull, AppError> {
        input.validify()?;

        if self.users.exists(&input.username).await? {
            return Err(AppError::ResourceConflict(format!(
                "username {} is already in use",
                input.username
            )));
        }

        input.password = hash_password(&input.password)?;

        let user_id = Uuid::new_v4();
        let root_id = Uuid::new_v4();

        let mut tx = self.tx.begin().await?;

        let user = self.users.insert(&mut *tx, user_id, &input).await?;

        self.files
            .insert_root(
                &mut *tx,
                NewRoot {
                    id: root_id,
                    name: input.username.clone(),
                    quota: 0,
                },
            )
            .await?;

        self.folders
            .insert(
                &mut *tx,
                RootAssignment {
                    root_id,
                    user_id,
                    primary: true,
                },
            )
            .await?;

        tx.commit().await?;

        if let Some(email) = input.email.as_deref()
            && let Err(error) = self
                .email
                .send_account_created(email, &input.username)
                .await
        {
            tracing::error!(%error, "welcome email was not sent");
        }

        Ok(user.into())
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn list_users(&self) -> Result<Vec<UserViewFull>, AppError> {
        self.users.list_active().await
    }

    #[instrument(skip(self, input), fields(user_id = %user_id, role = ?input.role), err(Debug))]
    pub async fn update_user(
        &self,
        user_id: Uuid,
        mut input: UpdateUser,
    ) -> Result<UserView, AppError> {
        input.validify()?;

        let user = self.users.get_active_by_id(user_id).await?;

        if user.role == Role::Admin && input.role != Role::Admin {
            let admins = self.users.count_active_admins().await?;
            if admins <= 1 {
                return Err(AppError::Forbidden(
                    "the last admin cannot be demoted".into(),
                ));
            }
        }

        if let Some(ref username) = input.username
            && *username != user.username
            && self.users.exists(username).await?
        {
            return Err(AppError::ResourceConflict(format!(
                "username {username} is already in use",
            )));
        }

        if let Some(email) = input.email.as_deref()
            && let Some(owner) = self.users.get_by_email(email).await?
            && owner.id != user_id
        {
            return Err(AppError::ResourceConflict("email is already in use".into()));
        }

        let password = match input.password.as_deref() {
            Some(password) => Some(hash_password(password)?),
            None => None,
        };

        let user = self
            .users
            .update(
                user_id,
                input.username.as_deref(),
                input.email.as_deref(),
                input.role,
                password.as_deref(),
            )
            .await?;

        if password.is_some() {
            self.sessions.expire_all(user_id).await?;
        }

        Ok(user)
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn delete_user(&self, id: Uuid) -> Result<(), AppError> {
        let user = self.users.get_active_by_id(id).await?;

        if user.role == Role::Admin {
            let admins = self.users.count_active_admins().await?;

            if admins <= 1 {
                return Err(AppError::Forbidden(
                    "the last admin cannot be deleted".into(),
                ));
            }
        }

        let mut tx = self.tx.begin().await?;

        let private_root = self.files.get_private_root_with(&mut *tx, id).await?;

        let files = self
            .files
            .list_all_files_with(&mut *tx, private_root.id)
            .await?;

        let mut tx = FileService::delete_recursive(&self.files, &self.storage, tx, files).await?;

        self.files
            .delete_root_with(&mut *tx, private_root.id)
            .await?;

        self.users.delete_with(&mut *tx, id).await?;

        tx.commit().await?;

        Ok(())
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn create_shared(&self, mut input: CreateSharedFolder) -> Result<Root, AppError> {
        input.validify()?;

        FileEntry::valid_name(&input.name)?;

        let mut members = Vec::new();

        for member_id in input.members {
            let member = self.users.get_active_by_id(member_id).await?;
            members.push(member.id);
        }

        let root_id = Uuid::new_v4();

        let mut tx = self.tx.begin().await?;

        let root = self
            .files
            .insert_root(
                &mut *tx,
                NewRoot {
                    id: root_id,
                    name: input.name.clone(),
                    quota: input.quota_bytes,
                },
            )
            .await?;

        for member_id in members {
            self.folders
                .insert(
                    &mut *tx,
                    RootAssignment {
                        root_id,
                        user_id: member_id,
                        primary: false,
                    },
                )
                .await?;
        }

        tx.commit().await?;

        Ok(root)
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn list_all_roots(&self) -> Result<Vec<Root>, AppError> {
        self.files.list_all_roots().await
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn update_root(&self, id: Uuid, mut input: UpdateRoot) -> Result<Root, AppError> {
        input.validify()?;
        FileEntry::valid_name(&input.name)?;
        self.files
            .update_root(id, &input.name, input.quota_bytes)
            .await
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn set_quota(&self, folder_id: Uuid, input: SetQuota) -> Result<(), AppError> {
        input.validate()?;

        self.files.set_quota(folder_id, input.quota_bytes).await?;

        Ok(())
    }

    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn list_members(
        &self,
        user: &User,
        folder_id: Uuid,
    ) -> Result<Vec<UserView>, AppError> {
        if user.role != Role::Admin && !self.folders.can_access(user.id, folder_id).await? {
            return Err(AppError::Forbidden(
                "you do not have access to this folder".into(),
            ));
        }

        self.folders.list_users_in(folder_id).await
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn assign(&self, user_id: Uuid, root_id: Uuid) -> Result<(), AppError> {
        if self.files.get_root(root_id).await?.is_none() {
            return Err(AppError::ResourceMissing("folder does not exist".into()));
        }

        self.users.get_active_by_id(user_id).await?;

        self.folders
            .assign(RootAssignment {
                root_id,
                user_id,
                primary: false,
            })
            .await
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn unassign(&self, folder_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        if self.folders.is_primary(folder_id, user_id).await? {
            return Err(AppError::Forbidden(
                "the private folder cannot be unassigned".into(),
            ));
        }

        self.folders.remove(folder_id, user_id).await
    }
}
