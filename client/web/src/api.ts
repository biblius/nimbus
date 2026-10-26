import type {
  ActiveSessionView,
  Activity,
  CreateUserInput,
  DeletionEvent,
  DeletionResult,
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
  Root,
  RootData,
  SelfUserData,
  ShareFile,
  UserData,
  UserPasskeyView,
  UserView,
} from "@nimbus/shared/types";
import type {
  AuthenticatePublicKeyCredentialJSON,
  RegisterPublicKeyCredentialJSON,
} from "@nimbus/shared/webauthn";

import {
  ApiError,
  ValidationError,
  OTP_TOKEN_HEADER,
  isAuthChallenge,
  parseError,
  type ErrorBody,
  type UploadProgress,
} from "@nimbus/shared/api-errors";
import type { NimbusApi } from "@nimbus/shared/nimbus-api";

export type { UploadProgress } from "@nimbus/shared/api-errors";

export type NimbusClientOptions = {
  /** Called when a response indicates the session is no longer valid. */
  onUnauthorized?: () => void;
};

export class NimbusClient implements NimbusApi {
  private readonly root: string;
  private readonly onUnauthorized?: () => void;

  constructor(baseUrl: string, options: NimbusClientOptions = {}) {
    this.root = baseUrl.replace(/\/$/, "");
    this.onUnauthorized = options.onUnauthorized;
  }

  /** Parse an error response and notify the host when the session has expired. */
  private httpError(
    status: number,
    payload: ErrorBody | null,
    path: string,
    method = "GET",
  ): Error {
    if (status === 401 && !isAuthChallenge(path, method)) {
      this.onUnauthorized?.();
    }
    return parseError(status, payload);
  }

  login(
    username: string,
    password: string,
    options: { remember?: boolean; permanent?: boolean } = {},
  ): Promise<LoginResult> {
    return this.request<LoginResult>("/auth/login", {
      method: "POST",
      body: JSON.stringify({
        username,
        password,
        remember: options.remember ?? false,
        permanent: options.permanent ?? false,
      }),
    });
  }

  loginOtp(token: string, otpCode: string): Promise<LoginResult> {
    return this.request<LoginResult>("/auth/login/otp", {
      method: "POST",
      body: JSON.stringify({ token, otpCode }),
    });
  }

  me(): Promise<UserView> {
    return this.request<UserView>("/auth/me");
  }

  updateMe(input: SelfUserData): Promise<UserView> {
    return this.request<UserView>("/auth/me", {
      method: "PATCH",
      body: JSON.stringify(input),
    });
  }

  logout(): Promise<void> {
    return this.request<void>("/auth/logout", { method: "POST" });
  }

  refreshSession(): Promise<LoginResult> {
    return this.request<LoginResult>("/auth/session/refresh", {
      method: "POST",
    });
  }

  listSessions(): Promise<ActiveSessionView[]> {
    return this.request<ActiveSessionView[]>("/auth/sessions");
  }

  expireSession(id: string): Promise<void> {
    return this.request<void>(`/auth/sessions/${encodeURIComponent(id)}`, {
      method: "DELETE",
    });
  }

  expireOtherSessions(): Promise<void> {
    return this.request<void>("/auth/sessions", { method: "DELETE" });
  }

  changePassword(currentPassword: string, newPassword: string): Promise<void> {
    return this.request<void>("/auth/password", {
      method: "POST",
      body: JSON.stringify({ currentPassword, newPassword }),
    });
  }

  async startOtpSetup(): Promise<OtpSetup> {
    const response = await fetch(`${this.root}/auth/otp/setup`, {
      method: "POST",
      credentials: "include",
    });

    if (!response.ok) {
      let body: ErrorBody | null = null;
      try {
        body = (await response.json()) as ErrorBody;
      } catch {
        body = null;
      }
      throw this.httpError(response.status, body, "/auth/otp/setup");
    }

    const token = response.headers.get(OTP_TOKEN_HEADER);
    if (!token) {
      throw new ApiError(500, {
        message: "Missing OTP setup token",
        code: 0,
        body: null,
      });
    }

    return {
      qrSvg: await response.text(),
      token,
    };
  }

  finishOtpSetup(token: string, otpCode: string): Promise<void> {
    return this.request<void>("/auth/otp/setup/confirm", {
      method: "POST",
      body: JSON.stringify({ token, otpCode }),
    });
  }

  disableOtp(otpCode: string): Promise<void> {
    return this.request<void>("/auth/otp/disable", {
      method: "POST",
      body: JSON.stringify({ otpCode }),
    });
  }

  listPasskeys(): Promise<UserPasskeyView[]> {
    return this.request<UserPasskeyView[]>("/auth/passkeys");
  }

  deletePasskey(id: string): Promise<void> {
    return this.request<void>(`/auth/passkeys/${encodeURIComponent(id)}`, {
      method: "DELETE",
    });
  }

  startPasskeySetup(): Promise<PasskeySetupChallenge> {
    return this.request<PasskeySetupChallenge>("/auth/passkey/setup", {
      method: "POST",
    });
  }

  finishPasskeySetup(
    token: string,
    credential: RegisterPublicKeyCredentialJSON,
  ): Promise<void> {
    return this.request<void>("/auth/passkey/setup/confirm", {
      method: "POST",
      body: JSON.stringify({ token, credential }),
    });
  }

  /** Start a discoverable (usernameless) passkey login ceremony. */
  startPasskeyLogin(
    options: {
      token?: string;
      remember?: boolean;
      permanent?: boolean;
    } = {},
  ): Promise<PasskeyLoginChallenge> {
    return this.request<PasskeyLoginChallenge>("/auth/login/passkey", {
      method: "POST",
      body: JSON.stringify({
        token: options.token,
        remember: options.remember ?? false,
        permanent: options.permanent ?? false,
      }),
    });
  }

  finishPasskeyLogin(
    token: string,
    credential: AuthenticatePublicKeyCredentialJSON,
  ): Promise<LoginResult> {
    return this.request<LoginResult>("/auth/login/passkey/confirm", {
      method: "POST",
      body: JSON.stringify({ token, credential }),
    });
  }

  /**
   * Browser half of native passkey login: verify the assertion without creating
   * a session. Returns the pickup token the desktop app should exchange.
   */
  confirmNativePasskeyLogin(
    token: string,
    credential: AuthenticatePublicKeyCredentialJSON,
  ): Promise<string> {
    return this.request<string>("/auth/login/passkey/confirm/native", {
      method: "POST",
      body: JSON.stringify({ token, credential }),
    });
  }

  /**
   * Desktop half of native passkey login: exchange the pickup token for a
   * session cookie. Returns 404 until the browser step has completed.
   */
  finishNativeLogin(token: string): Promise<LoginResult> {
    return this.request<LoginResult>("/auth/login/native", {
      method: "POST",
      body: JSON.stringify({ token }),
    });
  }

  resetPassword(token: string, newPassword: string): Promise<void> {
    return this.request<void>(`/auth/reset/${token}`, {
      method: "POST",
      body: JSON.stringify({ newPassword }),
    });
  }

  requestPasswordReset(email: string): Promise<void> {
    return this.request<void>("/auth/reset", {
      method: "POST",
      body: JSON.stringify({ email }),
    });
  }

  roots(): Promise<Root[]> {
    return this.request<Root[]>("/roots");
  }

  list(rootId: string, parent: string | null): Promise<FileView[]> {
    const query = parent ? `?parent=${encodeURIComponent(parent)}` : "";
    return this.request<FileView[]>(
      `/roots/${encodeURIComponent(rootId)}${query}`,
    );
  }

  /** Name full-text search within a root, ordered by rank. */
  search(rootId: string, q: string): Promise<FileView[]> {
    const query = `?q=${encodeURIComponent(q)}`;
    return this.request<FileView[]>(
      `/roots/${encodeURIComponent(rootId)}/search${query}`,
    );
  }

  /**
   * All directories in a root as `id → FileEntry` for client-side path crumbs.
   */
  descendants(rootId: string): Promise<Record<string, FileEntry>> {
    return this.request<Record<string, FileEntry>>(
      `/roots/${encodeURIComponent(rootId)}/descendants`,
    );
  }

  /**
   * Directories from the root down to `fileId`, including `fileId`.
   * The first entry sits in the root. The last entry is `fileId`.
   */
  ancestors(rootId: string, fileId: string): Promise<FileEntry[]> {
    return this.request<FileEntry[]>(
      `/roots/${encodeURIComponent(rootId)}/files/${encodeURIComponent(fileId)}/ancestors`,
    );
  }

  createTextFile(
    rootId: string,
    parent: string | null,
    name: string,
  ): Promise<FileEntry> {
    return this.request<FileEntry>(
      `/roots/${encodeURIComponent(rootId)}/files`,
      {
        method: "POST",
        body: JSON.stringify({ parent, name }),
      },
    );
  }

  createDirectory(
    rootId: string,
    parent: string | null,
    name: string,
  ): Promise<FileEntry> {
    return this.request<FileEntry>(
      `/roots/${encodeURIComponent(rootId)}/directories`,
      {
        method: "POST",
        body: JSON.stringify({ parent, name }),
      },
    );
  }

  /** Upload one file. `onProgress` reports bytes sent so far and the total body size. */
  upload(
    rootId: string,
    parent: string | null,
    file: File,
    onProgress?: UploadProgress,
    signal?: AbortSignal,
  ): Promise<FileEntry> {
    const body = new FormData();

    if (parent) {
      body.set("parent", parent);
    }

    body.set("file", file);

    return this.uploadForm<FileEntry>(
      `/roots/${encodeURIComponent(rootId)}`,
      body,
      onProgress,
      signal,
      (payload) => {
        if (Array.isArray(payload) && payload[0]) {
          onProgress?.(file.size, file.size);
          return payload[0] as FileEntry;
        }
        return null;
      },
    );
  }

  /**
   * Upload a directory tree. Each file's `webkitRelativePath` is sent as the
   * multipart filename so the server can recreate folders.
   */
  uploadDir(
    rootId: string,
    parent: string | null,
    files: File[],
    onProgress?: UploadProgress,
    signal?: AbortSignal,
  ): Promise<FileEntry[]> {
    const body = new FormData();

    if (parent) {
      body.set("parent", parent);
    }

    let total = 0;

    for (const file of files) {
      total += file.size;
      body.append("file", file, file.webkitRelativePath || file.name);
    }

    return this.uploadForm<FileEntry[]>(
      `/roots/${encodeURIComponent(rootId)}/upload/dir`,
      body,
      onProgress,
      signal,
      (payload) => {
        if (Array.isArray(payload)) {
          onProgress?.(total, total);
          return payload as FileEntry[];
        }
        return null;
      },
    );
  }

  /** Shared XHR upload used by file and directory uploads for progress + abort. */
  private uploadForm<T>(
    path: string,
    body: FormData,
    onProgress: UploadProgress | undefined,
    signal: AbortSignal | undefined,
    parse: (payload: unknown) => T | null,
  ): Promise<T> {
    // fetch has no upload progress events, so these calls use XMLHttpRequest.
    return new Promise<T>((resolve, reject) => {
      if (signal?.aborted) {
        reject(new DOMException("upload aborted", "AbortError"));
        return;
      }

      const xhr = new XMLHttpRequest();
      let settled = false;

      const succeed = (value: T) => {
        if (settled) {
          return;
        }
        settled = true;
        signal?.removeEventListener("abort", onAbort);
        resolve(value);
      };

      const fail = (error: unknown) => {
        if (settled) {
          return;
        }
        settled = true;
        signal?.removeEventListener("abort", onAbort);
        reject(error);
      };

      const onAbort = () => {
        xhr.abort();
        fail(new DOMException("upload aborted", "AbortError"));
      };

      signal?.addEventListener("abort", onAbort);

      xhr.open("POST", `${this.root}${path}`);

      xhr.withCredentials = true;

      xhr.upload.onprogress = (event) => {
        if (event.lengthComputable) {
          onProgress?.(event.loaded, event.total);
        }
      };

      xhr.onload = () => {
        let payload: ErrorBody | null = null;

        try {
          payload = xhr.responseText
            ? (JSON.parse(xhr.responseText) as ErrorBody)
            : null;
        } catch {
          payload = null;
        }

        if (xhr.status >= 200 && xhr.status < 300) {
          const value = parse(payload);
          if (value !== null) {
            succeed(value);
            return;
          }
        }

        fail(this.httpError(xhr.status, payload, path));
      };

      xhr.onerror = () => fail(new TypeError("network error"));

      xhr.onabort = () =>
        fail(new DOMException("upload aborted", "AbortError"));

      xhr.send(body);
    });
  }

  tree(rootId: string): Promise<FileTree> {
    return this.request<FileTree>(`/roots/${encodeURIComponent(rootId)}/tree`);
  }

  trashTree(rootId: string): Promise<FileTree> {
    return this.request<FileTree>(`/roots/${encodeURIComponent(rootId)}/trash`);
  }

  listActivities(
    rootId: string,
    query: Pick<ListQuery, "page" | "perPage">,
  ): Promise<Page<Activity>> {
    const params = new URLSearchParams({
      page: String(query.page),
      perPage: String(query.perPage),
    });
    return this.request<Page<Activity>>(
      `/roots/${encodeURIComponent(rootId)}/activity?${params}`,
    );
  }

  restore(rootId: string, id: string, parent: string | null): Promise<void> {
    return this.request<void>(
      `/roots/${encodeURIComponent(rootId)}/trash/${encodeURIComponent(id)}/restore`,
      { method: "POST", body: JSON.stringify({ parent }) },
    );
  }

  restoreAll(rootId: string, parent: string | null): Promise<void> {
    return this.request<void>(
      `/roots/${encodeURIComponent(rootId)}/trash/restore`,
      { method: "POST", body: JSON.stringify({ parent }) },
    );
  }

  deleteTrash(rootId: string, id: string): AsyncGenerator<DeletionEvent> {
    return this.events(
      `/roots/${encodeURIComponent(rootId)}/trash/${encodeURIComponent(id)}`,
      { method: "DELETE" },
    );
  }

  move(rootId: string, ids: string[], parent: string | null): Promise<void> {
    return this.request<void>(`/roots/${encodeURIComponent(rootId)}/move`, {
      method: "POST",
      body: JSON.stringify({ ids, parent }),
    });
  }

  rename(rootId: string, id: string, name: string): Promise<void> {
    return this.request<void>(
      `/roots/${encodeURIComponent(rootId)}/files/${encodeURIComponent(id)}`,
      {
        method: "PATCH",
        body: JSON.stringify({ name }),
      },
    );
  }

  copy(
    rootId: string,
    ids: string[],
    targetRootId: string,
    parent: string | null = null,
  ): Promise<void> {
    return this.request<void>(`/roots/${encodeURIComponent(rootId)}/copy`, {
      method: "POST",
      body: JSON.stringify({ ids, targetRootId, parent }),
    });
  }

  trash(rootId: string, ids: string[]): Promise<void> {
    return this.request<void>(`/roots/${encodeURIComponent(rootId)}/files`, {
      method: "DELETE",
      body: JSON.stringify({ ids }),
    });
  }

  /** Mark live files (and descendants, for directories) as publicly shared. */
  shareFiles(
    rootId: string,
    ids: string[],
    input: ShareFile,
  ): Promise<void> {
    return this.request<void>(
      `/roots/${encodeURIComponent(rootId)}/files/share`,
      {
        method: "POST",
        body: JSON.stringify({
          ids,
          sharingMode: input.sharingMode,
          shareExpiresAt: input.shareExpiresAt,
        }),
      },
    );
  }

  /** Clear the public share flag on live files and their descendants. */
  unshareFiles(rootId: string, ids: string[]): Promise<void> {
    return this.request<void>(
      `/roots/${encodeURIComponent(rootId)}/files/share`,
      {
        method: "DELETE",
        body: JSON.stringify({ ids }),
      },
    );
  }

  /** Public shared file entry (no session required). */
  sharedFile(fileId: string): Promise<SharedFileView> {
    return this.request<SharedFileView>(
      `/files/shared/${encodeURIComponent(fileId)}`,
      { credentials: "omit" },
    );
  }

  /** Children of a publicly shared directory. */
  sharedList(parentId: string): Promise<SharedFileView[]> {
    return this.request<SharedFileView[]>(
      `/files/shared/${encodeURIComponent(parentId)}/children`,
      { credentials: "omit" },
    );
  }

  /** Full-file blob URL (attachment download). Directories zip. */
  downloadUrl(rootId: string, id: string): string {
    return `${this.root}/roots/${encodeURIComponent(rootId)}/files/${encodeURIComponent(id)}/blob/download`;
  }

  /** Zip of every top-level entry in a root. */
  downloadRootUrl(rootId: string): string {
    return `${this.root}/roots/${encodeURIComponent(rootId)}/download`;
  }

  /** Zip of the given live file/directory ids. */
  downloadDirUrl(rootId: string, ids: string[]): string {
    const params = new URLSearchParams({ ids: ids.join(",") });
    return `${this.root}/roots/${encodeURIComponent(rootId)}/files/download?${params}`;
  }

  /** Range-capable blob URL for streaming media (`<video>` / `<audio>` src). */
  contentUrl(rootId: string, id: string): string {
    return `${this.root}/roots/${encodeURIComponent(rootId)}/files/${encodeURIComponent(id)}/blob/chunk`;
  }

  /** Inline preview content (original blob or derived `previewBlob`). */
  contentPreviewUrl(rootId: string, id: string): string {
    return `${this.root}/roots/${encodeURIComponent(rootId)}/files/${encodeURIComponent(id)}/preview`;
  }

  /** Public shared attachment download URL. */
  sharedDownloadUrl(fileId: string): string {
    return `${this.root}/files/shared/${encodeURIComponent(fileId)}/blob/download`;
  }

  /** Public shared Range-capable blob URL for media. */
  sharedContentUrl(fileId: string): string {
    return `${this.root}/files/shared/${encodeURIComponent(fileId)}/blob/chunk`;
  }

  /** Public shared inline preview content. */
  sharedContentPreviewUrl(fileId: string): string {
    return `${this.root}/files/shared/${encodeURIComponent(fileId)}/preview`;
  }

  /** Fetch a file's bytes (same auth as download). */
  async content(rootId: string, id: string): Promise<Blob> {
    const path = `/roots/${encodeURIComponent(rootId)}/files/${encodeURIComponent(id)}/blob/download`;
    const response = await fetch(`${this.root}${path}`, {
      credentials: "include",
    });

    if (!response.ok) {
      let body: ErrorBody | null = null;
      try {
        body = (await response.json()) as ErrorBody;
      } catch {
        body = null;
      }
      throw this.httpError(response.status, body, path);
    }

    return response.blob();
  }

  /** Fetch inline preview content (original blob or derived `previewBlob`). */
  async contentPreview(rootId: string, id: string): Promise<Blob> {
    const path = `/roots/${encodeURIComponent(rootId)}/files/${encodeURIComponent(id)}/preview`;
    const response = await fetch(`${this.root}${path}`, {
      credentials: "include",
    });

    if (!response.ok) {
      let body: ErrorBody | null = null;
      try {
        body = (await response.json()) as ErrorBody;
      } catch {
        body = null;
      }
      throw this.httpError(response.status, body, path);
    }

    return response.blob();
  }

  /** Replace an existing file's blob (multipart `file` field). */
  overwrite(
    rootId: string,
    id: string,
    content: Blob,
    name: string,
  ): Promise<void> {
    const body = new FormData();
    body.set("file", content, name);

    return this.request<void>(
      `/roots/${encodeURIComponent(rootId)}/files/${encodeURIComponent(id)}/overwrite`,
      { method: "PUT", body },
    );
  }

  /**
   * Kick off a browser-managed download (streams to disk).
   *
   * Prefer this over `fetch` + `blob()` for large/zip payloads: buffering the
   * whole response in JS waits until the server finishes before any save UI.
   */
  private triggerDownload(url: string, name: string): void {
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = name;
    anchor.rel = "noopener";
    document.body.appendChild(anchor);
    anchor.click();
    anchor.remove();
  }

  async download(rootId: string, id: string, name: string): Promise<void> {
    this.triggerDownload(this.downloadUrl(rootId, id), name);
  }

  /** Download the whole root as a zip archive. */
  async downloadRoot(rootId: string, name: string): Promise<void> {
    this.triggerDownload(this.downloadRootUrl(rootId), name);
  }

  /** Download selected live files/directories as one zip archive. */
  async downloadDir(
    rootId: string,
    ids: string[],
    name = "download.zip",
  ): Promise<void> {
    this.triggerDownload(this.downloadDirUrl(rootId, ids), name);
  }

  /** Fetch publicly shared file bytes for inline preview (allowed in view mode). */
  async sharedContent(fileId: string): Promise<Blob> {
    const path = `/files/shared/${encodeURIComponent(fileId)}/blob/chunk`;
    const response = await fetch(`${this.root}${path}`, {
      credentials: "omit",
    });

    if (!response.ok) {
      let body: ErrorBody | null = null;
      try {
        body = (await response.json()) as ErrorBody;
      } catch {
        body = null;
      }
      throw this.httpError(response.status, body, path);
    }

    return response.blob();
  }

  /** Fetch publicly shared inline preview content. */
  async sharedContentPreview(fileId: string): Promise<Blob> {
    const path = `/files/shared/${encodeURIComponent(fileId)}/preview`;
    const response = await fetch(`${this.root}${path}`, {
      credentials: "omit",
    });

    if (!response.ok) {
      let body: ErrorBody | null = null;
      try {
        body = (await response.json()) as ErrorBody;
      } catch {
        body = null;
      }
      throw this.httpError(response.status, body, path);
    }

    return response.blob();
  }

  /** Attachment download for a publicly shared file (download mode only). */
  async sharedDownload(fileId: string, name: string): Promise<void> {
    this.triggerDownload(this.sharedDownloadUrl(fileId), name);
  }

  /** Fetch a file's WebP icon as a blob (`64` list or `256` large tile). */
  async icon(
    rootId: string,
    id: string,
    size: 64 | 256 = 64,
  ): Promise<Blob> {
    const path = `/roots/${encodeURIComponent(rootId)}/files/${encodeURIComponent(id)}/icon?size=${size}`;
    const response = await fetch(`${this.root}${path}`, {
      credentials: "include",
    });

    if (!response.ok) {
      let body: ErrorBody | null = null;
      try {
        body = (await response.json()) as ErrorBody;
      } catch {
        body = null;
      }
      throw this.httpError(response.status, body, path);
    }

    return response.blob();
  }

  /** Fetch a publicly shared file's WebP icon. */
  async sharedIcon(fileId: string, size: 64 | 256 = 64): Promise<Blob> {
    const path = `/files/shared/${encodeURIComponent(fileId)}/icon?size=${size}`;
    const response = await fetch(`${this.root}${path}`, {
      credentials: "omit",
    });

    if (!response.ok) {
      let body: ErrorBody | null = null;
      try {
        body = (await response.json()) as ErrorBody;
      } catch {
        body = null;
      }
      throw this.httpError(response.status, body, path);
    }

    return response.blob();
  }

  listUsers(query: ListQuery): Promise<Page<UserView>> {
    return this.request<Page<UserView>>(
      `/admin/users?${this.listParams(query)}`,
    );
  }

  createUser(input: CreateUserInput): Promise<UserView> {
    return this.request<UserView>("/admin/users", {
      method: "POST",
      body: JSON.stringify(input),
    });
  }

  updateUser(id: string, input: UserData): Promise<UserView> {
    return this.request<UserView>(`/admin/users/${encodeURIComponent(id)}`, {
      method: "PATCH",
      body: JSON.stringify(input),
    });
  }

  deleteUser(id: string): AsyncGenerator<DeletionEvent> {
    return this.events(`/admin/users/${encodeURIComponent(id)}`, {
      method: "DELETE",
    });
  }

  listPrivateFolders(): Promise<Root[]> {
    return this.request<Root[]>("/admin/roots/private");
  }

  listSharedFolders(): Promise<Root[]> {
    return this.request<Root[]>("/admin/roots/shared");
  }

  getFolder(id: string): Promise<Root> {
    return this.request<Root>(`/admin/roots/${encodeURIComponent(id)}`);
  }

  createFolder(input: RootData): Promise<Root> {
    return this.request<Root>("/admin/roots/shared", {
      method: "POST",
      body: JSON.stringify(input),
    });
  }

  updateFolder(id: string, input: RootData): Promise<Root> {
    return this.request<Root>(`/admin/roots/${encodeURIComponent(id)}`, {
      method: "PUT",
      body: JSON.stringify(input),
    });
  }

  deleteFolder(id: string): AsyncGenerator<DeletionEvent> {
    return this.events(`/admin/roots/${encodeURIComponent(id)}`, {
      method: "DELETE",
    });
  }

  listAllMembers(id: string): Promise<Member[]> {
    return this.request<Member[]>(`/roots/${encodeURIComponent(id)}/users/all`);
  }

  listMembers(id: string, query: ListQuery): Promise<Page<Member>> {
    return this.request<Page<Member>>(
      `/roots/${encodeURIComponent(id)}/users?${this.listParams(query)}`,
    );
  }

  private listParams(query: ListQuery): URLSearchParams {
    const params = new URLSearchParams({
      page: String(query.page),
      perPage: String(query.perPage),
    });

    if (query.search?.q) {
      params.set("q", query.search.q);
      params.set("column", query.search.column);
    }

    if (query.sort) {
      params.set("sort.column", query.sort.column);
      params.set("sort.direction", query.sort.direction);
    }

    return params;
  }

  folderUsage(id: string): Promise<number> {
    return this.request<number>(`/roots/${encodeURIComponent(id)}/usage`);
  }

  private async *events(
    path: string,
    init: RequestInit,
  ): AsyncGenerator<DeletionEvent> {
    const response = await fetch(`${this.root}${path}`, {
      ...init,
      credentials: "include",
    });

    if (!response.ok) {
      let body: ErrorBody | null = null;
      try {
        body = (await response.json()) as ErrorBody;
      } catch {
        body = null;
      }
      throw this.httpError(response.status, body, path);
    }

    let failed = false;

    if (response.body) {
      const reader = response.body.getReader();
      const decoder = new TextDecoder();
      let buffer = "";

      for (;;) {
        const { done, value } = await reader.read();

        if (done) {
          break;
        }

        buffer += decoder.decode(value, { stream: true });

        const parts = buffer.split("\n\n");

        buffer = parts.pop() ?? "";

        for (const part of parts) {
          const data = part
            .split("\n")
            .filter((line) => line.startsWith("data:"))
            .map((line) => line.slice("data:".length).trim())
            .join("\n");

          // Empty frames, or axum keep-alives (`data: keep-alive`), are not JSON results.
          if (!data || data === "keep-alive") {
            continue;
          }

          const result = JSON.parse(data) as DeletionResult;

          if (!result.success) {
            failed = true;
          }

          yield { type: "progress", result };
        }
      }
    }

    yield { type: "end" };

    if (failed) {
      throw new ApiError(500, null);
    }
  }

  async request<T>(path: string, init: RequestInit = {}): Promise<T> {
    const headers = new Headers(init.headers);

    if (
      init.body &&
      !(init.body instanceof FormData) &&
      !headers.has("Content-Type")
    ) {
      headers.set("Content-Type", "application/json");
    }

    const response = await fetch(`${this.root}${path}`, {
      ...init,
      headers,
      credentials: init.credentials ?? "include",
    });

    if (!response.ok) {
      let body: ErrorBody | null = null;
      try {
        body = (await response.json()) as ErrorBody;
      } catch {
        body = null;
      }
      throw this.httpError(
        response.status,
        body,
        path,
        init.method ?? "GET",
      );
    }

    if (response.status === 204) {
      return undefined as T;
    }

    return (await response.json()) as T;
  }
}
