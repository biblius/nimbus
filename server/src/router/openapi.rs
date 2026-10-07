use utoipa::OpenApi;

use crate::{
    auth::model::{
        GeneratedOtp, LoginCredentials, LoginResult, Otp, PasswordChange, PasswordReset,
        PasswordResetRequest,
    },
    auth::session::SessionView,
    file::model::{CreateDirectory, FileEntry, Root, RootView},
    router::{auth, files, folders, users},
    user::model::{
        CreateSharedFolder, CreateUser, CurrentUserView, FolderQuota, Role, SetQuota, UserView,
    },
};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Nimbus",
        description = "Self-hosted file sharing",
        version = "0.1.0"
    ),
    paths(
        auth::credentials_login,
        auth::otp_login,
        auth::start_password_reset,
        auth::finish_password_reset,
        auth::me,
        auth::logout,
        auth::change_password,
        auth::start_otp_setup,
        auth::finish_otp_setup,
        users::create_user,
        users::list_users,
        users::delete_user,
        folders::list_members,
        folders::create_shared,
        folders::set_quota,
        folders::assign,
        folders::unassign,
        files::roots,
        files::list_files,
        files::create_directory,
        files::upload,
        files::metadata,
        files::download,
        files::delete_file,
    ),
    components(schemas(
        LoginCredentials,
        LoginResult,
        Otp,
        GeneratedOtp,
        PasswordResetRequest,
        PasswordReset,
        PasswordChange,
        SessionView,
        CurrentUserView,
        Role,
        CreateUser,
        CreateSharedFolder,
        FolderQuota,
        Root,
        RootView,
        UserView,
        FileEntry,
        CreateDirectory,
        SetQuota,
    ))
)]
pub struct ApiDoc;
