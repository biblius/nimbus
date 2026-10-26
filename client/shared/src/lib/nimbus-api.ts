import type {
  ActiveSessionView,
  Activity,
  CreateUserInput,
  DeletionEvent,
  FileEntry,
  FileTree,
  FileView,
  LoginResult,
  SharedFileView,
  Member,
  ListQuery,
  OtpSetup,
  Page,
  PasskeyLoginChallenge,
  PasskeySetupChallenge,
  NimbusConfigUpdate,
  NimbusConfiguration,
  Root,
  RootData,
  SelfUserData,
  ShareFile,
  UserData,
  UserPasskeyView,
  UserView,
} from "./types";
import type {
  AuthenticatePublicKeyCredentialJSON,
  RegisterPublicKeyCredentialJSON,
} from "./webauthn";
import type { UploadProgress } from "./api-errors";

/**
 * Shared API surface used by both clients.
 * Platform extras live on `NimbusWebApi` / `NimbusNativeApi` in each app.
 */
export interface NimbusApi {
  login(
      username: string,
      password: string,
      options?: { remember?: boolean; permanent?: boolean },
    ): Promise<LoginResult>;

  loginOtp(token: string, otpCode: string): Promise<LoginResult>;

  me(): Promise<UserView>;

  updateMe(input: SelfUserData): Promise<UserView>;

  logout(): Promise<void>;

  /** Rotate the current session credential; returns the new session id. */
  refreshSession(): Promise<LoginResult>;

  listSessions(): Promise<ActiveSessionView[]>;

  expireSession(id: string): Promise<void>;

  expireOtherSessions(): Promise<void>;

  changePassword(currentPassword: string, newPassword: string): Promise<void>;

  startOtpSetup(): Promise<OtpSetup>;

  finishOtpSetup(token: string, otpCode: string): Promise<void>;

  disableOtp(otpCode: string): Promise<void>;

  listPasskeys(): Promise<UserPasskeyView[]>;

  deletePasskey(id: string): Promise<void>;

  startPasskeySetup(): Promise<PasskeySetupChallenge>;

  finishPasskeySetup(
      token: string,
      credential: RegisterPublicKeyCredentialJSON,
    ): Promise<void>;

  startPasskeyLogin(
      options?: {
        token?: string;
        remember?: boolean;
        permanent?: boolean;
      },
    ): Promise<PasskeyLoginChallenge>;

  finishPasskeyLogin(
      token: string,
      credential: AuthenticatePublicKeyCredentialJSON,
    ): Promise<LoginResult>;

  confirmNativePasskeyLogin(
      token: string,
      credential: AuthenticatePublicKeyCredentialJSON,
    ): Promise<string>;

  finishNativeLogin(token: string): Promise<LoginResult>;

  resetPassword(token: string, newPassword: string): Promise<void>;

  requestPasswordReset(email: string): Promise<void>;

  roots(): Promise<Root[]>;

  list(rootId: string, parent: string | null): Promise<FileView[]>;

  search(rootId: string, q: string): Promise<FileView[]>;

  descendants(rootId: string): Promise<Record<string, FileEntry>>;

  ancestors(rootId: string, fileId: string): Promise<FileEntry[]>;

  createTextFile(
      rootId: string,
      parent: string | null,
      name: string,
    ): Promise<FileEntry>;

  createDirectory(
      rootId: string,
      parent: string | null,
      name: string,
    ): Promise<FileEntry>;

  upload(
      rootId: string,
      parent: string | null,
      file: File,
      onProgress?: UploadProgress,
      signal?: AbortSignal,
    ): Promise<FileEntry>;

  uploadDir(
      rootId: string,
      parent: string | null,
      files: File[],
      onProgress?: UploadProgress,
      signal?: AbortSignal,
    ): Promise<FileEntry[]>;

  tree(rootId: string): Promise<FileTree>;

  trashTree(rootId: string): Promise<FileTree>;

  listActivities(
      rootId: string,
      query: Pick<ListQuery, "page" | "perPage">,
    ): Promise<Page<Activity>>;

  restore(rootId: string, id: string, parent: string | null): Promise<void>;

  restoreAll(rootId: string, parent: string | null): Promise<void>;

  deleteTrash(rootId: string, id: string): AsyncGenerator<DeletionEvent>;

  move(rootId: string, ids: string[], parent: string | null): Promise<void>;

  rename(rootId: string, id: string, name: string): Promise<void>;

  copy(
      rootId: string,
      ids: string[],
      targetRootId: string,
      parent: string | null,
    ): Promise<void>;

  trash(rootId: string, ids: string[]): Promise<void>;

  shareFiles(
      rootId: string,
      ids: string[],
      input: ShareFile,
    ): Promise<void>;

  unshareFiles(rootId: string, ids: string[]): Promise<void>;

  sharedFile(fileId: string): Promise<SharedFileView>;

  sharedList(parentId: string): Promise<SharedFileView[]>;

  downloadUrl(rootId: string, id: string): string;

  downloadRootUrl(rootId: string): string;

  downloadDirUrl(rootId: string, ids: string[]): string;

  contentUrl(rootId: string, id: string): string;

  contentPreviewUrl(rootId: string, id: string): string;

  sharedDownloadUrl(fileId: string): string;

  sharedContentUrl(fileId: string): string;

  sharedContentPreviewUrl(fileId: string): string;

  content(rootId: string, id: string): Promise<Blob>;

  contentPreview(rootId: string, id: string): Promise<Blob>;

  overwrite(
      rootId: string,
      id: string,
      content: Blob,
      name: string,
    ): Promise<void>;

  download(rootId: string, id: string, name: string): Promise<void>;

  downloadRoot(rootId: string, name: string): Promise<void>;

  downloadDir(
      rootId: string,
      ids: string[],
      name?: string,
    ): Promise<void>;

  sharedContent(fileId: string): Promise<Blob>;

  sharedContentPreview(fileId: string): Promise<Blob>;

  sharedDownload(fileId: string, name: string): Promise<void>;

  icon(
      rootId: string,
      id: string,
      size?: 64 | 256,
    ): Promise<Blob>;

  sharedIcon(fileId: string, size?: 64 | 256): Promise<Blob>;

  listUsers(query: ListQuery): Promise<Page<UserView>>;

  createUser(input: CreateUserInput): Promise<UserView>;

  updateUser(id: string, input: UserData): Promise<UserView>;

  deleteUser(id: string): AsyncGenerator<DeletionEvent>;

  listPrivateFolders(): Promise<Root[]>;

  listSharedFolders(): Promise<Root[]>;

  getFolder(id: string): Promise<Root>;

  createFolder(input: RootData): Promise<Root>;

  updateFolder(id: string, input: RootData): Promise<Root>;

  deleteFolder(id: string): AsyncGenerator<DeletionEvent>;

  listAllMembers(id: string): Promise<Member[]>;

  listMembers(id: string, query: ListQuery): Promise<Page<Member>>;

  folderUsage(id: string): Promise<number>;

  getConfig(): Promise<NimbusConfiguration>;

  updateConfig(input: NimbusConfigUpdate): Promise<NimbusConfiguration>;

}
