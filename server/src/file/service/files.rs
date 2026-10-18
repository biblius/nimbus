use super::super::{
    model::{CreateDirectory, CreateTextFile, FileEntry, FileKind, FileView},
    queue::{ProcessingQueue, ProcessingQueueHandle},
    storage::{Storage, TempFile, UploadWriter},
};
use crate::{
    db::{
        files::{FileRepository, NewFile, ShareRepository, TrashRepository},
        tx::Transactor,
        user_folders::UserFolderRepository,
    },
    error::AppError,
    file::{
        format::{FileType, TextFormat},
        model::{
            CopyFiles, DownloadFiles, FileContentPreview, MAX_ANCESTOR_DEPTH, MoveFiles,
            PreviewSize, RenameFile, RootView, ShareFiles, TrashFiles, UnshareFiles,
        },
        tree::{FileEntryTree, FileTree},
    },
    user::model::User,
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
    folders: UserFolderRepository,
    storage: Storage,
    processing: ProcessingQueueHandle,
    tx: Transactor,
}

impl FileService {
    pub fn new(
        files: FileRepository,
        share: ShareRepository,
        trash: TrashRepository,
        folders: UserFolderRepository,
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
    #[instrument(skip(self, user), fields(user_id = %user.id, root_id = %root_id), err(Debug))]
    pub async fn usage(&self, user: &User, root_id: Uuid) -> Result<i64, AppError> {
        self.accessible(user, root_id).await?;

        if self.files.get_root(root_id).await?.is_none() {
            return Err(AppError::ResourceMissing("Folder does not exist".into()));
        }

        self.files.root_quota_usage(root_id).await
    }

    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn list(
        &self,
        user: &User,
        root_id: Uuid,
        dir_id: Option<Uuid>,
    ) -> Result<Vec<FileView>, AppError> {
        self.accessible(user, root_id).await?;

        self.files.list_files(root_id, dir_id).await
    }

    /// Directories from the root down to `file_id`, including `file_id`.
    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn ancestors(
        &self,
        user: &User,
        root_id: Uuid,
        file_id: Uuid,
    ) -> Result<Vec<FileEntry>, AppError> {
        self.accessible(user, root_id).await?;

        let chain = self.files.list_ancestors(root_id, file_id).await?;

        if chain.is_empty() {
            return Err(AppError::ResourceMissing("File does not exist".into()));
        }

        Ok(chain)
    }

    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn metadata(&self, user: &User, id: Uuid) -> Result<FileEntry, AppError> {
        self.readable(user, id).await
    }

    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn create_directory(
        &self,
        user: &User,
        root: Uuid,
        mut input: CreateDirectory,
    ) -> Result<FileEntry, AppError> {
        input.validify()?;

        let name = self.writable(user, root, &input.name).await?;

        let entry = self
            .files
            .create(NewFile {
                id: Uuid::new_v4(),
                root_id: root,
                created_by: user.id,
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

    #[instrument(skip(self, user), fields(%root, user_id = %user.id, name=%input.name), err(Debug))]
    pub async fn create_text_file(
        &self,
        user: &User,
        root: Uuid,
        mut input: CreateTextFile,
    ) -> Result<FileEntry, AppError> {
        input.validify()?;

        self.accessible(user, root).await?;

        let mut writer = self.start_upload(user, root, input.name).await?;
        let mut file = writer.finish().await?;

        // Keep the entry editable via overwrite even if the name has no text extension.
        if !matches!(file.file_type, FileType::Text(_)) {
            file.file_type = FileType::Text(TextFormat::Txt);
        }

        let mut files = self
            .finish_upload_all(user, root, input.parent, vec![], vec![(file, input.parent)])
            .await?;

        let Some(file) = files.pop() else {
            tracing::error!("finish_upload_all returned no items; this should not be possible!");
            return Err(AppError::Internal);
        };

        Ok(file)
    }

    /// Start an [`UploadWriter`] using the given name.
    ///
    /// Since this is usually called for processing a stream we don't want to
    /// hit the DB every time for accessibility checks, therefore root
    /// accessibility is not checked and is assumed to be accessible.
    ///
    /// Callers should call [`Self::accessible`] beforehand.
    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn start_upload(
        &self,
        user: &User,
        root: Uuid,
        name: String,
    ) -> Result<UploadWriter, AppError> {
        self.storage.writer(name).await
    }

    /// Persist a directory upload. Insert staged dirs and files, then move blobs into storage.
    ///
    /// The [`TempFile`]s should be obtained via [`Self::start_upload`].
    ///
    /// Root accessibility is not checked and is assumed to be accessible.
    ///
    /// Callers should call [`Self::accessible`] beforehand.
    #[instrument(
        skip(self, user, dirs, files),
        fields(user_id = %user.id, root_id = %root_id),
        err(Debug)
    )]
    pub async fn finish_upload_all(
        &self,
        user: &User,
        root_id: Uuid,
        parent: Option<Uuid>,
        dirs: Vec<CreateDirectory>,
        mut files: Vec<(TempFile, Option<Uuid>)>,
    ) -> Result<Vec<FileEntry>, AppError> {
        for dir in dirs.iter() {
            FileEntry::valid_name(&dir.name)?;
        }

        for (file, _) in files.iter() {
            FileEntry::valid_name(&file.name)?;
        }

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
                .get_live(id)
                .await?
                .map(|entry| (entry.sharing_mode, entry.share_expires_at))
                .unwrap_or((None, None)),
            None => (None, None),
        };

        let now = Utc::now();

        let mut entries: Vec<_> = dirs
            .iter()
            .map(|dir| FileEntry {
                id: dir.id,
                root_id,
                created_by: Some(user.id),
                kind: FileKind::Directory,
                name: dir.name.clone(),
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

        entries.extend(files.iter().map(|(file, parent)| FileEntry {
            id: Uuid::new_v4(),
            root_id,
            created_by: Some(user.id),
            kind: FileKind::File,
            name: file.name.clone(),
            parent: *parent,
            file_type: Some(file.file_type.clone()),
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
            if let Err(error) = ProcessingQueue::submit(&self.processing, entry.clone()).await {
                tracing::error!(file = %entry.id, %error, "failed to enqueue file for processing");
            }
        }

        Ok(entries)
    }

    /// Open a writer that will replace an existing file's blob.
    #[instrument(skip(self, user), fields(user_id = %user.id, file_id = %file_id), err(Debug))]
    pub async fn start_overwrite(
        &self,
        user: &User,
        root_id: Uuid,
        file_id: Uuid,
    ) -> Result<(FileEntry, UploadWriter), AppError> {
        let file = self.readable(user, file_id).await?;

        if file.root_id != root_id || file.kind != FileKind::File {
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

        let writer = self.storage.writer(file.name.clone()).await?;

        Ok((file, writer))
    }

    /// Persist new bytes and point the existing file row at them.
    #[instrument(skip(self, user, existing, file), fields(user_id = %user.id, file_id = %existing.id, name = %file.name), err(Debug))]
    pub async fn finish_overwrite(
        &self,
        user: &User,
        existing: FileEntry,
        mut file: TempFile,
    ) -> Result<FileEntry, AppError> {
        let Some(old_hash) = existing.hash.as_deref() else {
            // Should never happen
            return Err(AppError::Unprocessable(
                "Original file is missing hash".into(),
            ));
        };

        let content_changed = old_hash != file.hash.as_str();

        if !content_changed {
            tracing::info!("file contents are the same, skipping overwrite");
            return Ok(existing);
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

        let entry = self
            .files
            .update_content(&mut *tx, existing.id, &file.hash, file.size, file_type)
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

        Ok(entry)
    }

    /// Single-file blob for streaming / range requests. Directories are rejected.
    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn content_for_stream(
        &self,
        user: &User,
        id: Uuid,
    ) -> Result<(String, PathBuf, &'static str), AppError> {
        let file = self.readable(user, id).await?;

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
    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn content_for_download(
        &self,
        user: &User,
        id: Uuid,
    ) -> Result<(String, super::DownloadBody), AppError> {
        let file = self.readable(user, id).await?;

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
    #[instrument(skip(self, user), fields(user_id = %user.id, root_id = %root_id), err(Debug))]
    pub async fn content_for_root_download(
        &self,
        user: &User,
        root_id: Uuid,
    ) -> Result<(String, super::DownloadBody), AppError> {
        self.accessible(user, root_id).await?;

        let root = self
            .files
            .get_root(root_id)
            .await?
            .ok_or_else(|| AppError::ResourceMissing("Folder does not exist".into()))?;

        let children = self.files.list_descendants(root_id, None).await?;

        let blob_paths = super::blob_paths_for(&self.storage, &children).await?;

        let name = format!("{}.zip", root.name);

        let tree = FileTree {
            id: root.id,
            name: root.name,
            kind: FileKind::Directory,
            file_type: None,
            metadata: Default::default(),
            children: FileTree::from_entries(children),
        };

        Ok((name, super::zip_directory_stream(tree, blob_paths)))
    }

    /// Zip the given live files/directories into one streamed archive.
    #[instrument(skip(self, user, input), fields(user_id = %user.id, root_id = %root_id), err(Debug))]
    pub async fn download_dir(
        &self,
        user: &User,
        root_id: Uuid,
        input: DownloadFiles,
    ) -> Result<(String, super::DownloadBody), AppError> {
        input.validate()?;

        self.accessible(user, root_id).await?;

        let mut ids = Vec::with_capacity(input.ids.len());
        let mut selected = HashSet::with_capacity(input.ids.len());

        for id in input.ids {
            if selected.insert(id) {
                ids.push(id);
            }
        }

        let subtree = self.files.list_with_subtrees(root_id, &ids).await?;

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
    #[instrument(skip(self, user), fields(user_id = %user.id, ?size), err(Debug))]
    pub async fn preview(
        &self,
        user: &User,
        id: Uuid,
        size: PreviewSize,
    ) -> Result<FileContentPreview, AppError> {
        let file = self.readable_any(user, id).await?;

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

    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn delete(&self, user: &User, id: Uuid) -> Result<(), AppError> {
        let file = self.readable(user, id).await?;

        self.trash.trash(file.root_id, user.id, &[file.id]).await
    }

    /// Move every listed file in `root_id` to the trash, including descendants.
    #[instrument(skip(self, user, input), fields(user_id = %user.id, root_id = %root_id), err(Debug))]
    pub async fn delete_many(
        &self,
        user: &User,
        root_id: Uuid,
        input: TrashFiles,
    ) -> Result<(), AppError> {
        input.validate()?;

        self.accessible(user, root_id).await?;

        self.trash.trash(root_id, user.id, &input.ids).await
    }

    /// Change the display name of a live file or directory.
    #[instrument(skip(self, user, input), fields(user_id = %user.id, root_id = %root_id, file_id = %id), err(Debug))]
    pub async fn rename(
        &self,
        user: &User,
        root_id: Uuid,
        id: Uuid,
        mut input: RenameFile,
    ) -> Result<(), AppError> {
        input.validify()?;

        let file = self.readable(user, id).await?;

        let name = FileEntry::valid_name(&input.name)?;

        if file.root_id != root_id {
            return Err(AppError::ResourceMissing("File does not exist".into()));
        }

        if file.name == name {
            return Ok(());
        }

        self.files.update_name(id, name).await?;

        Ok(())
    }

    /// Change the parent of live files. Trashed files are rejected.
    #[instrument(skip(self, user, input), fields(user_id = %user.id, root_id = %root_id), err(Debug))]
    pub async fn move_files(
        &self,
        user: &User,
        root_id: Uuid,
        input: MoveFiles,
    ) -> Result<(), AppError> {
        input.validate()?;

        self.accessible(user, root_id).await?;

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
            for entry in &chain {
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
    #[instrument(skip(self, user, input), fields(user_id = %user.id, root_id = %root_id), err(Debug))]
    pub async fn copy_files(
        &self,
        user: &User,
        root_id: Uuid,
        input: CopyFiles,
    ) -> Result<(), AppError> {
        input.validate()?;

        self.accessible(user, root_id).await?;
        self.accessible(user, input.target_root_id).await?;

        if let Some(parent) = input.parent {
            let entry =
                self.files.get_live(parent).await?.ok_or_else(|| {
                    AppError::Unprocessable("Target directory does not exist".into())
                })?;

            if entry.root_id != input.target_root_id || entry.kind != FileKind::Directory {
                return Err(AppError::Unprocessable("Target is not a directory".into()));
            }
        }

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
        user: &User,
        root_id: Uuid,
        parent: Option<Uuid>,
    ) -> Result<FileTree, AppError> {
        self.accessible(user, root_id).await?;

        let files = self.files.list_descendants(root_id, parent).await?;

        Ok(FileTree {
            id: parent.unwrap_or(root_id),
            name: String::new(),
            kind: FileKind::Directory,
            file_type: None,
            metadata: Default::default(),
            children: FileTree::from_entries(files),
        })
    }

    /// Mark live files (and every descendant, for directories) as publicly shared.
    #[instrument(skip(self, user, input), fields(user_id = %user.id, root_id = %root_id, ?input.sharing_mode), err(Debug))]
    pub async fn share_files(
        &self,
        user: &User,
        root_id: Uuid,
        input: ShareFiles,
    ) -> Result<(), AppError> {
        input.validate()?;

        self.accessible(user, root_id).await?;

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
                root_id,
                &ids,
                Some(input.sharing_mode),
                input.share_expires_at,
            )
            .await
    }

    /// Clear the public share on live files and every descendant.
    #[instrument(skip(self, user, input), fields(user_id = %user.id, root_id = %root_id), err(Debug))]
    pub async fn unshare_files(
        &self,
        user: &User,
        root_id: Uuid,
        input: UnshareFiles,
    ) -> Result<(), AppError> {
        input.validate()?;

        self.accessible(user, root_id).await?;

        let mut ids = Vec::with_capacity(input.ids.len());
        let mut seen = HashSet::with_capacity(input.ids.len());

        for id in input.ids {
            if seen.insert(id) {
                ids.push(id);
            }
        }

        self.share
            .set_sharing_subtrees(root_id, &ids, None, None)
            .await
    }

    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    async fn readable(&self, user: &User, id: Uuid) -> Result<FileEntry, AppError> {
        let file = self
            .files
            .get_live(id)
            .await?
            .ok_or_else(|| AppError::ResourceMissing("File does not exist".into()))?;

        self.accessible(user, file.root_id).await?;

        Ok(file)
    }

    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    async fn writable(&self, user: &User, root_id: Uuid, name: &str) -> Result<String, AppError> {
        let name = FileEntry::valid_name(name)?.to_string();

        self.accessible(user, root_id).await?;

        Ok(name)
    }

    /// Resolve a live file, falling back to trash.
    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    async fn readable_any(&self, user: &User, id: Uuid) -> Result<FileEntry, AppError> {
        if let Some(file) = self.files.get_live(id).await? {
            self.accessible(user, file.root_id).await?;
            return Ok(file);
        }

        let file = self
            .trash
            .get_trashed(id)
            .await?
            .ok_or_else(|| AppError::ResourceMissing("File does not exist".into()))?;

        self.accessible(user, file.root_id).await?;

        Ok(file)
    }

    /// See [super::accessible].
    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn accessible(&self, user: &User, root: Uuid) -> Result<(), AppError> {
        super::accessible(&self.folders, user, root).await
    }
}
