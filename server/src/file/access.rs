use crate::{
    auth::session::Session, db::Db, error::AppError, file::model::Root, state::AppState,
    user::model::User,
};
use axum::extract::{FromRequestParts, Path};
use bitflags::bitflags;
use serde::Deserialize;
use sqlx::{Database, Decode, Encode, Type, encode::IsNull, error::BoxDynError};
use uuid::Uuid;

bitflags! {
    /// Permission bits for a user's membership in a shared root.
    /// Private roots are exempt from this.
    ///
    /// View access is implicit for any member.
    /// Private roots and admins are treated as having every defined flag.
    /// Unused high bits are reserved for future permissions.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct AccessMode: i64 {
        /// Upload, move, rename, edit, trash, restore, and empty trash.
        const WRITE = 1 << 0;
        /// Create and clear public file shares.
        const SHARE = 1 << 1;
    }
}

impl AccessMode {
    /// Default membership grants: write and share.
    pub const DEFAULT: Self = Self::all();

    /// Every currently defined permission bit (private owners and admins).
    pub const FULL: Self = Self::all();
}

impl Type<Db> for AccessMode {
    fn type_info() -> <Db as Database>::TypeInfo {
        <i64 as Type<Db>>::type_info()
    }

    fn compatible(ty: &<Db as Database>::TypeInfo) -> bool {
        <i64 as Type<Db>>::compatible(ty)
    }
}

impl<'q> Encode<'q, Db> for AccessMode {
    fn encode_by_ref(
        &self,
        buf: &mut <Db as Database>::ArgumentBuffer,
    ) -> Result<IsNull, BoxDynError> {
        <i64 as Encode<'q, Db>>::encode_by_ref(&self.bits(), buf)
    }
}

impl<'r> Decode<'r, Db> for AccessMode {
    fn decode(value: <Db as Database>::ValueRef<'r>) -> Result<Self, BoxDynError> {
        let bits = <i64 as Decode<'r, Db>>::decode(value)?;
        Ok(Self::from_bits_truncate(bits))
    }
}

/// Authenticated user plus the root they are operating on, with resolved permissions.
///
/// Constructed by the axum extractor (or [`crate::file::service::FileService::user_root`])
/// after verifying the user may access the root. View is always granted when this exists.
#[derive(Debug, Clone)]
pub struct UserRootContext {
    pub user: User,
    pub root: Root,
    pub access: AccessMode,
}

impl UserRootContext {
    pub fn require(&self, flags: AccessMode) -> Result<(), AppError> {
        if self.access.contains(flags) {
            Ok(())
        } else {
            Err(AppError::Forbidden(
                "You do not have permission to perform this action".into(),
            ))
        }
    }
}

#[derive(Debug, Deserialize)]
struct RootIdPath {
    root_id: Uuid,
}

impl FromRequestParts<AppState> for UserRootContext {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, state).await?;
        let Path(RootIdPath { root_id }) = Path::from_request_parts(parts, state)
            .await
            .map_err(|_| AppError::Unprocessable("Missing root id".into()))?;

        state
            .services
            .files
            .accessible_root(&session.user, root_id)
            .await
    }
}
