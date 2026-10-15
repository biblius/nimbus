use crate::{
    auth::crypto::hash_password,
    db::{
        Transactor,
        deletion::DeletionRepository,
        files::{FileTable, FileRepository, TrashRepository},
        sessions::SessionRepository,
        user_folders::UserFolderRepository,
        users::UserRepository,
    },
    email::EmailSender,
    error::AppError,
    file::{
        model::{Cleanup, FileEntry, RootAssignment, RootView},
        queue::{DeletionQueue, DeletionQueueHandle, DeletionResult},
    },
    model::{List, PaginationSort},
    user::model::{
        CreateSharedFolder, CreateUser, Member, Role, SetQuota, UpdateRoot, UpdateSelf, UpdateUser,
        User, UserSearch, UserSort, UserView, UserViewFull,
    },
};
use std::collections::HashSet;
use tokio::sync::mpsc;
use tracing::instrument;
use uuid::Uuid;
use validify::{Validate, Validify};

#[derive(Clone)]
pub struct UserService {
    users: UserRepository,
    files: FileRepository,
    trash: TrashRepository,
    folders: UserFolderRepository,
    sessions: SessionRepository,
    email: EmailSender,
    tx: Transactor,
    deletions: DeletionRepository,
}

impl UserService {
    pub fn new(
        users: UserRepository,
        files: FileRepository,
        trash: TrashRepository,
        folders: UserFolderRepository,
        sessions: SessionRepository,
        email: EmailSender,
        tx: Transactor,
        deletions: DeletionRepository,
    ) -> Self {
        Self {
            users,
            files,
            trash,
            folders,
            sessions,
            email,
            tx,
            deletions,
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
            return Err(AppError::ResourceConflict("Username already in use".into()));
        }

        input.password = hash_password(&input.password)?;

        let user_id = Uuid::new_v4();
        let root_id = Uuid::new_v4();

        let mut tx = self.tx.begin().await?;

        let user = self.users.insert(&mut *tx, user_id, &input).await?;

        self.files
            .insert_root(&mut *tx, root_id, &input.username, 0, Some(user_id))
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
    pub async fn list_users(
        &self,
        pagination: PaginationSort<UserSort, UserSearch>,
    ) -> Result<List<UserViewFull>, AppError> {
        pagination.validate()?;
        self.users.list_paginated(pagination).await
    }

    #[instrument(skip(self, input), fields(user_id = %user_id, role = ?input.role), err(Debug))]
    pub async fn update_user(
        &self,
        user_id: Uuid,
        mut input: UpdateUser,
    ) -> Result<UserView, AppError> {
        input.validify()?;

        let user = self.users.get_by_id(user_id).await?;

        if user.role == Role::Admin && input.role != Role::Admin {
            let admins = self.users.count_admins().await?;
            if admins <= 1 {
                return Err(AppError::Forbidden(
                    "Cannot delete last administrator".into(),
                ));
            }
        }

        if let Some(ref username) = input.username
            && *username != user.username
            && self.users.exists(username).await?
        {
            return Err(AppError::ResourceConflict(
                "Username is already in use".into(),
            ));
        }

        if let Some(email) = input.email.as_deref()
            && let Some(owner) = self.users.get_by_email(email).await?
            && owner.id != user_id
        {
            return Err(AppError::ResourceConflict("Email is already in use".into()));
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

    /// Update the signed-in user's own profile. Role is left unchanged.
    #[instrument(skip(self, input), fields(user_id = %user_id, session_id = %session_id), err(Debug))]
    pub async fn update_self(
        &self,
        user_id: Uuid,
        session_id: Uuid,
        mut input: UpdateSelf,
    ) -> Result<UserViewFull, AppError> {
        input.validify()?;

        let user = self.users.get_by_id(user_id).await?;

        if let Some(ref username) = input.username
            && *username != user.username
            && self.users.exists(username).await?
        {
            return Err(AppError::ResourceConflict(
                "Username is already in use".into(),
            ));
        }

        if let Some(email) = input.email.as_deref()
            && let Some(owner) = self.users.get_by_email(email).await?
            && owner.id != user_id
        {
            return Err(AppError::ResourceConflict("Email is already in use".into()));
        }

        let password = match input.password.as_deref() {
            Some(password) => Some(hash_password(password)?),
            None => None,
        };

        self.users
            .update(
                user_id,
                input.username.as_deref(),
                input.email.as_deref(),
                user.role,
                password.as_deref(),
            )
            .await?;

        if password.is_some() {
            self.sessions.expire_others(user_id, session_id).await?;
        }

        let updated = self.users.get_by_id(user_id).await?;
        Ok(updated.into())
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn delete_user(
        &self,
        id: Uuid,
        handle: DeletionQueueHandle,
    ) -> Result<mpsc::Receiver<DeletionResult>, AppError> {
        let user = self.users.get_by_id(id).await?;

        if user.role == Role::Admin {
            let admins = self.users.count_admins().await?;

            if admins <= 1 {
                return Err(AppError::Forbidden(
                    "Cannot delete last administrator".into(),
                ));
            }
        }

        let private_root = self.files.get_private_root(id).await?;

        let live = self.files.list_file_ids(private_root.id).await?;

        let trashed = self.trash.list_trashed_ids(private_root.id).await?;

        let request_id = Uuid::new_v4();

        let created_by = id.to_string();

        self.deletions
            .enqueue(request_id, &created_by, FileTable::Live, &live)
            .await?;

        self.deletions
            .enqueue(request_id, &created_by, FileTable::Trashed, &trashed)
            .await?;

        let rx = DeletionQueue::submit(
            handle,
            request_id,
            Some(Cleanup::User {
                user_id: user.id,
                root_id: private_root.id,
            }),
        )
        .await?;

        Ok(rx)
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn create_shared(&self, mut input: CreateSharedFolder) -> Result<RootView, AppError> {
        input.validify()?;

        FileEntry::valid_name(&input.name)?;

        let mut members = Vec::new();

        for member_id in input.members {
            let member = self.users.get_by_id(member_id).await?;
            members.push(member.id);
        }

        let root_id = Uuid::new_v4();

        let mut tx = self.tx.begin().await?;

        let root = self
            .files
            .insert_root(&mut *tx, root_id, &input.name, input.quota_bytes, None)
            .await?;

        for member_id in members {
            self.folders
                .insert(
                    &mut *tx,
                    RootAssignment {
                        root_id,
                        user_id: member_id,
                    },
                )
                .await?;
        }

        tx.commit().await?;

        Ok(RootView {
            id: root.id,
            name: root.name,
            quota: root.quota,
            owner: root.owner,
            created_at: root.created_at,
            updated_at: root.updated_at,
        })
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn list_private_roots(&self) -> Result<Vec<RootView>, AppError> {
        self.files.list_private_roots().await
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn list_shared_roots(&self) -> Result<Vec<RootView>, AppError> {
        self.files.list_shared_roots().await
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn get_root(&self, id: Uuid) -> Result<RootView, AppError> {
        let root = self
            .files
            .get_root(id)
            .await?
            .ok_or_else(|| AppError::ResourceMissing("Folder does not exist".into()))?;

        Ok(RootView {
            id: root.id,
            name: root.name,
            quota: root.quota,
            owner: root.owner,
            created_at: root.created_at,
            updated_at: root.updated_at,
        })
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn update_root(&self, id: Uuid, mut input: UpdateRoot) -> Result<RootView, AppError> {
        input.validify()?;
        FileEntry::valid_name(&input.name)?;

        let existing = self
            .files
            .get_root(id)
            .await?
            .ok_or_else(|| AppError::ResourceMissing("Folder does not exist".into()))?;

        if existing.owner.is_some() {
            if !input.members.is_empty() {
                return Err(AppError::Forbidden(
                    "A private folder cannot be shared".into(),
                ));
            }

            let root = self
                .files
                .update_root(id, &input.name, input.quota_bytes)
                .await?;

            return Ok(RootView {
                id: root.id,
                name: root.name,
                quota: root.quota,
                owner: root.owner,
                created_at: root.created_at,
                updated_at: root.updated_at,
            });
        }

        let current = self.folders.list_users_in(id).await?;
        let have: HashSet<Uuid> = current.iter().map(|member| member.id).collect();
        let want: HashSet<Uuid> = input.members.into_iter().collect();
        let add: Vec<Uuid> = want.difference(&have).copied().collect();
        let remove: Vec<Uuid> = have.difference(&want).copied().collect();

        for user_id in &add {
            self.users.get_by_id(*user_id).await?;
        }

        let root = self
            .files
            .update_root(id, &input.name, input.quota_bytes)
            .await?;

        for user_id in &add {
            self.folders
                .assign(RootAssignment {
                    root_id: id,
                    user_id: *user_id,
                })
                .await?;
        }

        for user_id in &remove {
            self.folders.remove(id, *user_id).await?;
        }

        Ok(RootView {
            id: root.id,
            name: root.name,
            quota: root.quota,
            owner: root.owner,
            created_at: root.created_at,
            updated_at: root.updated_at,
        })
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn delete_root(
        &self,
        user: &User,
        id: Uuid,
        handle: DeletionQueueHandle,
    ) -> Result<mpsc::Receiver<DeletionResult>, AppError> {
        let root = self
            .files
            .get_root(id)
            .await?
            .ok_or_else(|| AppError::ResourceMissing("Folder does not exist".into()))?;

        if root.owner.is_some() {
            return Err(AppError::Forbidden(
                "Delete the user to remove their private folder".into(),
            ));
        }

        let live = self.files.list_file_ids(id).await?;
        let trashed = self.trash.list_trashed_ids(id).await?;

        let request_id = Uuid::new_v4();

        self.deletions
            .enqueue(request_id, &user.id.to_string(), FileTable::Live, &live)
            .await?;

        self.deletions
            .enqueue(
                request_id,
                &user.id.to_string(),
                FileTable::Trashed,
                &trashed,
            )
            .await?;

        let rx =
            DeletionQueue::submit(handle, request_id, Some(Cleanup::Root { root_id: root.id }))
                .await?;

        Ok(rx)
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
        pagination: PaginationSort<UserSort, UserSearch>,
    ) -> Result<List<Member>, AppError> {
        pagination.validate()?;

        if user.role != Role::Admin && !self.folders.can_access(user.id, folder_id).await? {
            return Err(AppError::Forbidden(
                "You do not have access to this folder".into(),
            ));
        }

        self.folders
            .list_users_in_paginated(folder_id, pagination)
            .await
    }

    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn list_all_members(
        &self,
        user: &User,
        folder_id: Uuid,
    ) -> Result<Vec<Member>, AppError> {
        if user.role != Role::Admin && !self.folders.can_access(user.id, folder_id).await? {
            return Err(AppError::Forbidden(
                "You do not have access to this folder".into(),
            ));
        }

        self.folders.list_users_in(folder_id).await
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn assign(&self, root_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        let root = self
            .files
            .get_root(root_id)
            .await?
            .ok_or_else(|| AppError::ResourceMissing("Folder does not exist".into()))?;

        if root.owner.is_some() {
            return Err(AppError::Forbidden(
                "A private folder cannot be shared".into(),
            ));
        }

        self.users.get_by_id(user_id).await?;

        self.folders
            .assign(RootAssignment { root_id, user_id })
            .await
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn unassign(&self, folder_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        let root = self
            .files
            .get_root(folder_id)
            .await?
            .ok_or_else(|| AppError::ResourceMissing("Folder does not exist".into()))?;

        if root.owner.is_some() {
            return Err(AppError::Forbidden(
                "A private folder cannot be unassigned".into(),
            ));
        }

        self.folders.remove(folder_id, user_id).await
    }
}
