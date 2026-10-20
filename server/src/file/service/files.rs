use super::super::{
    model::{CreateDirectory, CreateTextFile, FileEntry, FileKind, FileView},
    queue::{ProcessingQueue, ProcessingQueueHandle},
    storage::{Storage, TempFile, UploadWriter},
};
use crate::{
    db::{
        files::{FileRepository, NewFile, ShareRepository, TrashRepository},
        tx::Transactor,
        user_roots::UserRootRepository,
    },
    error::AppError,
    file::{
        access::{AccessMode, UserRootContext},
        format::{FileType, TextFormat},
        model::{
            CopyFiles, DownloadFiles, FileContentPreview, MAX_ANCESTOR_DEPTH, MoveFiles,
            PreviewSize, RenameFile, RootView, ShareFiles, TrashFiles, UnshareFiles,
        },
        tree::{FileEntryTree, FileTree},
    },
    user::model::{Role, User},
};
use chrono::Utc;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};
use tracing::instrument;
use uuid::Uuid;
use validify::{Validate, Validify};

#[derive(Clone)]
pub struct FileService {
    files: FileRepository,
    share: ShareRepository,
    trash: TrashRepository,
    folders: UserRootRepository,
    storage: Storage,
    processing: ProcessingQueueHandle,
    tx: Transactor,
}

impl FileService {
    pub fn new(
        files: FileRepository,
        share: ShareRepository,
        trash: TrashRepository,
        folders: UserRootRepository,
        storage: Storage,
        processing: ProcessingQueueHandle,
        tx: Transactor,
    ) -> Self {
        Self {
            files,
            share,
            trash,
            folders,
            storage,
            processing,
            tx,
        }
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn prepare(&self) -> Result<(), AppError> {
        self.storage.prepare().await
    }

    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn list_accessible_roots(&self, user: &User) -> Result<Vec<RootView>, AppError> {
        self.files.list_accessible_roots(user.id).await
    }

    /// Bytes stored in a root. Directories are not counted.
    #[instrument(skip(self, ctx), fields(user_id = %ctx.user.id, root_id = %ctx.root.id), err(Debug))]
    pub async fn usage(&self, ctx: &UserRootContext) -> Result<i64, AppError> {
        self.files.root_quota_usage(ctx.root.id).await
    }

    #[instrument(skip(self, ctx), fields(user_id = %ctx.user.id, root_id = %ctx.root.id), err(Debug))]
    pub async fn list(
        &self,
        ctx: &UserRootContext,
        dir_id: Option<Uuid>,
    ) -> Result<Vec<FileView>, AppError> {
        self.files.list_files(ctx.root.id, dir_id).await
    }

    /// Directories from the root down to `file_id`, including `file_id`.
    #[instrument(skip(self, ctx), fields(user_id = %ctx.user.id, root_id = %ctx.root.id), err(Debug))]
    pub async fn ancestors(
        &self,
        ctx: &UserRootContext,
        file_id: Uuid,
    ) -> Result<Vec<FileEntry>, AppError> {
        let chain = self.files.list_ancestors(ctx.root.id, file_id).await?;

        if chain.is_empty() {
            return Err(AppError::ResourceMissing("File does not exist".into()));
        }

        Ok(chain)
    }

    #[instrument(skip(self, ctx), fields(user_id = %ctx.user.id, root_id = %ctx.root.id), err(Debug))]
    pub async fn metadata(&self, ctx: &UserRootContext, id: Uuid) -> Result<FileEntry, AppError> {
        self.files
            .get_live(ctx.root.id, id)
            .await?
            .ok_or_else(|| AppError::ResourceMissing("File does not exist".into()))
    }

    #[instrument(skip(self, ctx), fields(user_id = %ctx.user.id, root_id = %ctx.root.id), err(Debug))]
    pub async fn create_directory(
        &self,
        ctx: &UserRootContext,
        mut input: CreateDirectory,
    ) -> Result<FileEntry, AppError> {
        input.validify()?;
        ctx.require(AccessMode::WRITE)?;

        let name = FileEntry::valid_name(&input.name)?.to_string();

        let entry = self
            .files
            .create(NewFile {
                id: Uuid::new_v4(),
                root_id: ctx.root.id,
                created_by: ctx.user.id,
                kind: FileKind::Directory,
                name,
                parent: input.parent,
                file_type: None,
                hash: None,
                size_bytes: 0,
            })
            .await?;

        Ok(entry)
    }

    #[instrument(skip(self, ctx), fields(root_id = %ctx.root.id, user_id = %ctx.user.id, name=%input.name), err(Debug))]
    pub async fn create_text_file(
        &self,
        ctx: &UserRootContext,
        mut input: CreateTextFile,
    ) -> Result<FileEntry, AppError> {
        input.validify()?;

        ctx.require(AccessMode::WRITE)?;

        let mut writer = self.start_upload(ctx, input.name).await?;
        let mut file = writer.finish().await?;

        // Keep the entry editable via overwrite even if the name has no text extension.
        if !matches!(file.file_type, FileType::Text(_)) {
            file.file_type = FileType::Text(TextFormat::Txt);
        }

        let mut files = self
            .finish_upload(ctx, input.parent, vec![], vec![(file, input.parent)])
            .await?;

        let Some(file) = files.pop() else {
            tracing::error!("finish_upload returned no items; this should not be possible!");
            return Err(AppError::Internal);
        };

        Ok(file)
    }

    /// Start an [`UploadWriter`] using the given name.
    ///
    /// Write permission must already be verified by the caller (typically via
    /// [`UserRootContext::require`]). This only opens the temp writer.
    #[instrument(skip(self, ctx, name), fields(user_id = %ctx.user.id, root_id = %ctx.root.id), err(Debug))]
    pub async fn start_upload(
        &self,
        ctx: &UserRootContext,
        name: impl Into<String>,
    ) -> Result<UploadWriter, AppError> {
        let _ctx = ctx;
        self.storage.writer(name).await
    }

    /// Persist a directory upload. Insert staged dirs and files, then move blobs into storage.
    ///
    /// The [`TempFile`]s should be obtained via [`Self::start_upload`].
    #[instrument(
        skip(self, ctx, dirs, files),
        fields(user_id = %ctx.user.id, root_id = %ctx.root.id),
        err(Debug)
    )]
    pub async fn finish_upload(
        &self,
        ctx: &UserRootContext,
        parent: Option<Uuid>,
        dirs: Vec<CreateDirectory>,
        mut files: Vec<(TempFile, Option<Uuid>)>,
    ) -> Result<Vec<FileEntry>, AppError> {
        ctx.require(AccessMode::WRITE)?;

        for dir in dirs.iter() {
            FileEntry::valid_name(&dir.name)?;
        }

        for (file, _) in files.iter() {
            FileEntry::valid_name(&file.name)?;
        }

        let root_id = ctx.root.id;
        let quota = self.files.root_quota(root_id).await?;

        if quota > 0 {
            let used = self.files.root_quota_usage(root_id).await?;
            let total: i64 = files.iter().map(|(file, _)| file.size).sum();

            if used.saturating_add(total) > quota {
                return Err(AppError::QuotaExceeded);
            }
        }

        // Directory uploads land under one parent; inherit that parent's share settings.
        let (sharing_mode, share_expires_at) = match parent {
            Some(id) => self
                .files
                .get_live(root_id, id)
                .await?
                .map(|entry| (entry.sharing_mode, entry.share_expires_at))
                .unwrap_or((None, None)),
            None => (None, None),
        };

        let now = Utc::now();

        let mut entries: Vec<_> = dirs
            .into_iter()
            .map(|dir| FileEntry {
                id: dir.id,
                root_id,
                created_by: Some(ctx.user.id),
                kind: FileKind::Directory,
                name: dir.name,
                parent: dir.parent,
                file_type: None,
                hash: None,
                size_bytes: 0,
                sharing_mode,
                share_expires_at,
                metadata: Default::default(),
                created_at: now,
                updated_at: now,
            })
            .collect();

        entries.extend(files.iter_mut().map(|(file, parent)| FileEntry {
            id: Uuid::new_v4(),
            root_id,
            created_by: Some(ctx.user.id),
            kind: FileKind::File,
            name: std::mem::take(&mut file.name),
            parent: *parent,
            file_type: Some(std::mem::replace(
                &mut file.file_type,
                FileType::Other(String::new()),
            )),
            hash: Some(file.hash.clone()),
            size_bytes: file.size,
            sharing_mode,
            share_expires_at,
            metadata: Default::default(),
            created_at: now,
            updated_at: now,
        }));

        let mut tx = self.tx.begin().await?;

        self.files.insert_many(&mut tx, &entries).await?;

        for (file, _) in files.iter_mut() {
            self.storage.persist(file).await?;
        }

        tx.commit().await?;

        for entry in entries.iter().filter(|entry| entry.kind == FileKind::File) {
            if let Err(error) = ProcessingQueue::submit(&self.processing, entry).await {
                tracing::error!(file = %entry.id, %error, "failed to enqueue file for processing");
            }
        }

        Ok(entries)
    }

    /// Open a writer that will replace an existing file's blob.
    #[instrument(skip(self, ctx), fields(user_id = %ctx.user.id, root_id = %ctx.root.id, file_id = %file_id), err(Debug))]
    pub async fn start_overwrite(
        &self,
        ctx: &UserRootContext,
        file_id: Uuid,
    ) -> Result<(FileEntry, UploadWriter), AppError> {
        ctx.require(AccessMode::WRITE)?;

        let file = self
            .files
            .get_live(ctx.root.id, file_id)
            .await?
            .ok_or_else(|| AppError::ResourceMissing("File does not exist".into()))?;

        if file.kind != FileKind::File {
            return Err(AppError::ResourceMissing("File does not exist".into()));
        }

        if file
            .file_type
            .as_ref()
            .is_none_or(|ty| !matches!(ty, FileType::Text(_)))
        {
            return Err(AppError::Unprocessable(
                "Cannot overwrite contents - not a text file".into(),
            ));
        }

        let writer = self.storage.writer(file.name.as_str()).await?;

        Ok((file, writer))
    }

    /// Persist new bytes and point the existing file row at them.
    #[instrument(skip(self, ctx, existing, file), fields(user_id = %ctx.user.id, file_id = %existing.id, name = %file.name), err(Debug))]
    pub async fn finish_overwrite(
        &self,
        ctx: &UserRootContext,
        existing: FileEntry,
        mut file: TempFile,
    ) -> Result<(), AppError> {
        ctx.require(AccessMode::WRITE)?;

        if existing.root_id != ctx.root.id {
            return Err(AppError::ResourceMissing("File does not exist".into()));
        }

        let Some(old_hash) = existing.hash.as_deref() else {
            // Should never happen
            return Err(AppError::Unprocessable(
                "Original file is missing hash".into(),
            ));
        };

        let content_changed = old_hash != file.hash.as_str();

        if !content_changed {
            tracing::info!("file contents are the same, skipping overwrite");
            return Ok(());
        }

        let quota = self.files.root_quota(existing.root_id).await?;

        if quota > 0 {
            let used = self.files.root_quota_usage(existing.root_id).await?;

            if used
                .saturating_sub(existing.size_bytes)
                .saturating_add(file.size)
                > quota
            {
                return Err(AppError::QuotaExceeded);
            }
        }

        // Text files with no extension can cause problems
        //
        // Prefer from extension and fallback to existing.
        let file_type = existing
            .file_type
            .as_ref()
            .unwrap_or(&FileType::Text(TextFormat::Txt));

        let mut tx = self.tx.begin().await?;

        self.files
            .update_content(
                &mut *tx,
                existing.root_id,
                existing.id,
                &file.hash,
                file.size,
                file_type,
            )
            .await?;

        self.storage.persist(&mut file).await?;

        tx.commit().await?;

        match self.files.count_hash(old_hash).await {
            Ok(count) => {
                if count == 0
                    && let Err(error) = self.storage.remove(old_hash).await
                {
                    tracing::error!(%error, hash = %old_hash, "failed to remove replaced blob");
                }
            }
            Err(error) => {
                tracing::error!(%error, hash = %old_hash, "failed to count replaced blob");
            }
        }

        Ok(())
    }

    /// Single-file blob for streaming / range requests. Directories are rejected.
    #[instrument(skip(self, ctx), fields(user_id = %ctx.user.id, root_id = %ctx.root.id), err(Debug))]
    pub async fn content_for_stream(
        &self,
        ctx: &UserRootContext,
        id: Uuid,
    ) -> Result<(String, PathBuf, &'static str), AppError> {
        let file = self
            .files
            .get_live(ctx.root.id, id)
            .await?
            .ok_or_else(|| AppError::ResourceMissing("File does not exist".into()))?;

        if file.kind != FileKind::File {
            return Err(AppError::Unprocessable(
                "Directories cannot be streamed".into(),
            ));
        }

        let hash = file.hash.as_deref().ok_or(AppError::Internal)?;
        let path = self.storage.path(hash);

        if !tokio::fs::try_exists(&path).await? {
            tracing::error!(%id, hash, "file row has no blob on disk");
            return Err(AppError::Internal);
        }

        let mime = file
            .file_type
            .as_ref()
            .map(|file_type| file_type.mime())
            .unwrap_or("application/octet-stream");

        Ok((file.name, path, mime))
    }

    /// Files stream as-is; directories are returned as a zip streamed while being built.
    #[instrument(skip(self, ctx), fields(user_id = %ctx.user.id, root_id = %ctx.root.id), err(Debug))]
    pub async fn content_for_download(
        &self,
        ctx: &UserRootContext,
        id: Uuid,
    ) -> Result<(String, super::DownloadBody), AppError> {
        let file = self
            .files
            .get_live(ctx.root.id, id)
            .await?
            .ok_or_else(|| AppError::ResourceMissing("File does not exist".into()))?;

        match file.kind {
            FileKind::File => {
                let hash = file.hash.as_deref().ok_or(AppError::Internal)?;
                let path = self.storage.path(hash);

                if !tokio::fs::try_exists(&path).await? {
                    tracing::error!(%id, hash, "file row has no blob on disk");
                    return Err(AppError::Internal);
                }

                let mime = file
                    .file_type
                    .as_ref()
                    .map(|file_type| file_type.mime())
                    .unwrap_or("application/octet-stream");

                Ok((file.name, super::DownloadBody::File { path, mime }))
            }
            FileKind::Directory => {
                let children = self
                    .files
                    .list_descendants(file.root_id, Some(file.id))
                    .await?;

                let blob_paths = super::blob_paths_for(&self.storage, &children).await?;
                let name = format!("{}.zip", file.name);

                let tree = FileTree {
                    id: file.id,
                    name: file.name,
                    kind: file.kind,
                    file_type: file.file_type,
                    metadata: file.metadata.into_option(),
                    children: FileTree::from_entries(children),
                };

                Ok((name, super::zip_directory_stream(tree, blob_paths)))
            }
        }
    }

    /// Zip every top-level entry in a root the user can access.
    #[instrument(skip(self, ctx), fields(user_id = %ctx.user.id, root_id = %ctx.root.id), err(Debug))]
    pub async fn content_for_root_download(
        &self,
        ctx: &UserRootContext,
    ) -> Result<(String, super::DownloadBody), AppError> {
        let children = self.files.list_descendants(ctx.root.id, None).await?;

        let blob_paths = super::blob_paths_for(&self.storage, &children).await?;

        let name = format!("{}.zip", ctx.root.name);

        let tree = FileTree {
            id: ctx.root.id,
            name: ctx.root.name.clone(),
            kind: FileKind::Directory,
            file_type: None,
            metadata: Default::default(),
            children: FileTree::from_entries(children),
        };

        Ok((name, super::zip_directory_stream(tree, blob_paths)))
    }

    /// Zip the given live files/directories into one streamed archive.
    #[instrument(skip(self, ctx, input), fields(user_id = %ctx.user.id, root_id = %ctx.root.id), err(Debug))]
    pub async fn download_dir(
        &self,
        ctx: &UserRootContext,
        input: DownloadFiles,
    ) -> Result<(String, super::DownloadBody), AppError> {
        input.validate()?;

        let mut ids = Vec::with_capacity(input.ids.len());
        let mut selected = HashSet::with_capacity(input.ids.len());

        for id in input.ids {
            if selected.insert(id) {
                ids.push(id);
            }
        }

        let subtree = self.files.list_with_subtrees(ctx.root.id, &ids).await?;

        let entries = subtree
            .into_iter()
            .map(|entry| FileEntryTree {
                query_root: selected.contains(&entry.id),
                id: entry.id,
                kind: entry.kind,
                name: entry.name,
                parent: entry.parent,
                file_type: entry.file_type,
                hash: entry.hash,
                metadata: entry.metadata,
            })
            .collect::<Vec<_>>();

        let blob_paths = super::blob_paths_for(&self.storage, &entries).await?;

        let tree = FileTree {
            id: Uuid::nil(),
            name: "download".into(),
            kind: FileKind::Directory,
            file_type: None,
            metadata: None,
            children: FileTree::from_entries(entries),
        };

        Ok((
            "download.zip".into(),
            super::zip_directory_stream(tree, blob_paths),
        ))
    }

    /// Serve a generated preview image for a file, when one exists.
    ///
    /// Works for both live and trashed files so trash trees can show thumbnails.
    #[instrument(skip(self, ctx), fields(user_id = %ctx.user.id, root_id = %ctx.root.id, ?size), err(Debug))]
    pub async fn preview(
        &self,
        ctx: &UserRootContext,
        id: Uuid,
        size: PreviewSize,
    ) -> Result<FileContentPreview, AppError> {
        let file = if let Some(file) = self.files.get_live(ctx.root.id, id).await? {
            file
        } else {
            self.trash
                .get_trashed(ctx.root.id, id)
                .await?
                .ok_or_else(|| AppError::ResourceMissing("File does not exist".into()))?
        };

        if file.kind != FileKind::File {
            return Err(AppError::Unprocessable(
                "Directories do not have previews".into(),
            ));
        }

        if file.metadata.is_empty() {
            return Err(AppError::ResourceMissing("Preview does not exist".into()));
        }

        let hash = match size {
            PreviewSize::Icon => file.metadata.preview_64.as_deref(),
            PreviewSize::Tile => file.metadata.preview_256.as_deref(),
        }
        .ok_or_else(|| AppError::ResourceMissing("Preview does not exist".into()))?;

        let path = self.storage.preview_path(hash);

        let preview = FileContentPreview::new(path, "image/webp").await?;

        Ok(preview)
    }

    #[instrument(skip(self, ctx), fields(user_id = %ctx.user.id, root_id = %ctx.root.id), err(Debug))]
    pub async fn delete(&self, ctx: &UserRootContext, id: Uuid) -> Result<(), AppError> {
        ctx.require(AccessMode::WRITE)?;

        let file = self
            .files
            .get_live(ctx.root.id, id)
            .await?
            .ok_or_else(|| AppError::ResourceMissing("File does not exist".into()))?;

        self.trash
            .trash(file.root_id, ctx.user.id, &[file.id])
            .await
    }

    /// Move every listed file in the root to the trash, including descendants.
    #[instrument(skip(self, ctx, input), fields(user_id = %ctx.user.id, root_id = %ctx.root.id), err(Debug))]
    pub async fn delete_many(
        &self,
        ctx: &UserRootContext,
        input: TrashFiles,
    ) -> Result<(), AppError> {
        input.validate()?;
        ctx.require(AccessMode::WRITE)?;

        self.trash.trash(ctx.root.id, ctx.user.id, &input.ids).await
    }

    /// Change the display name of a live file or directory.
    #[instrument(skip(self, ctx, input), fields(user_id = %ctx.user.id, root_id = %ctx.root.id, file_id = %id), err(Debug))]
    pub async fn rename(
        &self,
        ctx: &UserRootContext,
        id: Uuid,
        mut input: RenameFile,
    ) -> Result<(), AppError> {
        input.validify()?;
        ctx.require(AccessMode::WRITE)?;

        let file = self
            .files
            .get_live(ctx.root.id, id)
            .await?
            .ok_or_else(|| AppError::ResourceMissing("File does not exist".into()))?;

        let name = FileEntry::valid_name(&input.name)?;

        if file.name == name {
            return Ok(());
        }

        self.files.update_name(file.root_id, id, name).await?;

        Ok(())
    }

    /// Change the parent of live files. Trashed files are rejected.
    #[instrument(skip(self, ctx, input), fields(user_id = %ctx.user.id, root_id = %ctx.root.id), err(Debug))]
    pub async fn move_files(
        &self,
        ctx: &UserRootContext,
        input: MoveFiles,
    ) -> Result<(), AppError> {
        input.validate()?;
        ctx.require(AccessMode::WRITE)?;

        let root_id = ctx.root.id;
        let mut ids = Vec::with_capacity(input.ids.len());
        let mut seen = HashSet::with_capacity(input.ids.len());

        for id in input.ids {
            if seen.insert(id) {
                ids.push(id);
            }
        }

        // /foo/bar: reject moving /foo into /bar (or any descendant). This is
        // what keeps the tree acyclic; the depth/duplicate checks below are
        // only defensive against corrupt data.
        if let Some(parent) = input.parent {
            let chain = self.files.list_ancestors(root_id, parent).await?;

            if chain.is_empty() {
                return Err(AppError::ResourceMissing("Folder does not exist".into()));
            }

            if chain.len() as i64 >= MAX_ANCESTOR_DEPTH {
                return Err(AppError::Unprocessable(
                    "Cannot move a folder into itself".into(),
                ));
            }

            let mut path_seen = HashSet::with_capacity(chain.len());

            for entry in chain {
                if !path_seen.insert(entry.id) || seen.contains(&entry.id) {
                    return Err(AppError::Unprocessable(
                        "Cannot move a folder into itself".into(),
                    ));
                }

                if entry.kind != FileKind::Directory {
                    return Err(AppError::Unprocessable("Not a directory".into()));
                }
            }
        }

        self.files.move_to_parent(root_id, input.parent, &ids).await
    }

    /// Copy live files (and descendants, for directories) into `input.target_root_id`.
    ///
    /// Each copied row gets a new id and `created_at`, other metadata is retained.
    /// Blob content is shared by hash and no filesystem copy is performed.
    ///
    /// Requires write access on the **target** root (creating copies). Source root
    /// access is already verified by [`UserRootContext`].
    #[instrument(skip(self, ctx, input), fields(user_id = %ctx.user.id, root_id = %ctx.root.id), err(Debug))]
    pub async fn copy_files(
        &self,
        ctx: &UserRootContext,
        input: CopyFiles,
    ) -> Result<(), AppError> {
        input.validate()?;

        let target = self
            .accessible_root(&ctx.user, input.target_root_id)
            .await?;
        target.require(AccessMode::WRITE)?;

        if let Some(parent) = input.parent {
            let entry = self
                .files
                .get_live(input.target_root_id, parent)
                .await?
                .ok_or_else(|| AppError::Unprocessable("Target directory does not exist".into()))?;

            if entry.kind != FileKind::Directory {
                return Err(AppError::Unprocessable("Target is not a directory".into()));
            }
        }

        let root_id = ctx.root.id;
        let mut ids = Vec::with_capacity(input.ids.len());
        let mut copying = HashSet::with_capacity(input.ids.len());

        for id in input.ids {
            if copying.insert(id) {
                ids.push(id);
            }
        }

        let subtree = self.files.list_with_subtrees(root_id, &ids).await?;

        let mut id_remap = HashMap::with_capacity(subtree.len());

        for entry in subtree.iter() {
            id_remap.insert(entry.id, Uuid::new_v4());
        }

        let now = Utc::now();
        let mut copies = Vec::with_capacity(subtree.len());
        let mut size = 0_i64;

        for entry in subtree {
            if entry.kind == FileKind::File {
                size = size.saturating_add(entry.size_bytes);
            }

            let parent = match entry.parent {
                // Parent is also in this copy, keep nesting under the new id.
                Some(parent) if id_remap.contains_key(&parent) => Some(id_remap[&parent]),
                // Selected root (parent outside the copy, or was null), or a
                // defensive fallback if list_with_subtrees were unsound.
                _ => input.parent,
            };

            copies.push(FileEntry {
                id: id_remap[&entry.id],
                root_id: input.target_root_id,
                created_by: entry.created_by,
                kind: entry.kind,
                name: entry.name,
                parent,
                file_type: entry.file_type,
                hash: entry.hash,
                size_bytes: entry.size_bytes,
                // Copies are private until explicitly shared again.
                sharing_mode: None,
                share_expires_at: None,
                metadata: entry.metadata,
                created_at: now,
                updated_at: entry.updated_at,
            });
        }

        let quota = self.files.root_quota(input.target_root_id).await?;

        if quota > 0 {
            let used = self.files.root_quota_usage(input.target_root_id).await?;

            if used.saturating_add(size) > quota {
                return Err(AppError::QuotaExceeded);
            }
        }

        let mut tx = self.tx.begin().await?;

        self.files.insert_many(&mut tx, &copies).await?;

        tx.commit().await?;

        Ok(())
    }

    pub async fn get_tree(
        &self,
        ctx: &UserRootContext,
        parent: Option<Uuid>,
    ) -> Result<FileTree, AppError> {
        let files = self.files.list_descendants(ctx.root.id, parent).await?;

        Ok(FileTree {
            id: parent.unwrap_or(ctx.root.id),
            name: String::new(),
            kind: FileKind::Directory,
            file_type: None,
            metadata: Default::default(),
            children: FileTree::from_entries(files),
        })
    }

    /// Mark live files (and every descendant, for directories) as publicly shared.
    #[instrument(skip(self, ctx, input), fields(user_id = %ctx.user.id, root_id = %ctx.root.id, ?input.sharing_mode), err(Debug))]
    pub async fn share_files(
        &self,
        ctx: &UserRootContext,
        input: ShareFiles,
    ) -> Result<(), AppError> {
        input.validate()?;
        ctx.require(AccessMode::SHARE)?;

        if let Some(expires_at) = input.share_expires_at
            && expires_at <= Utc::now()
        {
            return Err(AppError::Unprocessable(
                "Share expiry must be in the future".into(),
            ));
        }

        let mut ids = Vec::with_capacity(input.ids.len());
        let mut seen = HashSet::with_capacity(input.ids.len());

        for id in input.ids {
            if seen.insert(id) {
                ids.push(id);
            }
        }

        self.share
            .set_sharing_subtrees(
                ctx.root.id,
                &ids,
                Some(input.sharing_mode),
                input.share_expires_at,
            )
            .await
    }

    /// Clear the public share on live files and every descendant.
    #[instrument(skip(self, ctx, input), fields(user_id = %ctx.user.id, root_id = %ctx.root.id), err(Debug))]
    pub async fn unshare_files(
        &self,
        ctx: &UserRootContext,
        input: UnshareFiles,
    ) -> Result<(), AppError> {
        input.validate()?;
        ctx.require(AccessMode::SHARE)?;

        let mut ids = Vec::with_capacity(input.ids.len());
        let mut seen = HashSet::with_capacity(input.ids.len());

        for id in input.ids {
            if seen.insert(id) {
                ids.push(id);
            }
        }

        self.share
            .set_sharing_subtrees(ctx.root.id, &ids, None, None)
            .await
    }

    /// Resolve [`UserRootContext`] for `user` on `root_id`.
    ///
    /// Private-root owners and admins get [`AccessMode::FULL`]. Shared members use
    /// their `user_roots.access_mode`. View is implicit whenever this succeeds.
    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn accessible_root(
        &self,
        user: &User,
        root_id: Uuid,
    ) -> Result<UserRootContext, AppError> {
        let Some((root, membership)) = self.folders.access(user.id, root_id).await? else {
            return Err(AppError::ResourceMissing("Folder does not exist".into()));
        };

        let access = if root.owner == Some(user.id) || user.role == Role::Admin {
            AccessMode::FULL
        } else if let Some(mode) = membership {
            mode
        } else {
            return Err(AppError::Forbidden(
                "You do not have access to this folder".into(),
            ));
        };

        Ok(UserRootContext {
            user: user.clone(),
            root,
            access,
        })
    }
}
