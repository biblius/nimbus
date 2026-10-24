import { fetch } from "@tauri-apps/plugin-http";
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
  OTP_TOKEN_HEADER,
  isAuthChallenge,
  parseError,
  type ErrorBody,
  type UploadProgress,
} from "@nimbus/shared/api-errors";
import type { NimbusApi } from "@nimbus/shared/nimbus-api";

export {
  ApiError,
  ValidationError,
  isLoginBackoff,
  loginBackoffUnlockAt,
  type ErrorBody,
  type ValidationField,
  type ValidationItem,
  type ValidationSchema,
  type UploadProgress,
} from "@nimbus/shared/api-errors";

export type NimbusClientOptions = {
  /** Called when a response indicates the session is no longer valid. */
  onUnauthorized?: () => void;
  /** Initial Bearer token loaded from SQLite. */
  accessToken?: string | null;
  /** Persist or clear the single access token. */
  onAccessTokenChange?: (token: string | null) => void | Promise<void>;
};

/**
 * Native Tauri API client. Uses `@tauri-apps/plugin-http` and Bearer auth.
 * Same method surface as the web `NimbusClient`.
 */
export class NimbusClient implements NimbusApi {
  private readonly root: string;
  private readonly onUnauthorized?: () => void;
  private readonly onAccessTokenChange?: (
    token: string | null,
  ) => void | Promise<void>;
  private accessToken: string | null;

  constructor(baseUrl: string, options: NimbusClientOptions = {}) {
    this.root = baseUrl.replace(/\/$/, "");
    this.onUnauthorized = options.onUnauthorized;
    this.onAccessTokenChange = options.onAccessTokenChange;
    this.accessToken = options.accessToken ?? null;
  }

  private async setAccessToken(token: string | null): Promise<void> {
    this.accessToken = token;
    await this.onAccessTokenChange?.(token);
  }

  private async rememberSession(result: LoginResult): Promise<LoginResult> {
    if (result.status === "session") {
      await this.setAccessToken(result.sessionId);
    }
    return result;
  }

  /** tauri-http fetch with Bearer (no cookies / CORS). */
  private send(path: string, init: RequestInit = {}): Promise<Response> {
    const headers = new Headers(init.headers);

    if (
      init.body &&
      !(init.body instanceof FormData) &&
      !headers.has("Content-Type")
    ) {
      headers.set("Content-Type", "application/json");
    }

    // credentials: "omit" = public route, no Bearer.
    if (init.credentials !== "omit" && this.accessToken) {
      headers.set("Authorization", `Bearer ${this.accessToken}`);
    }

    return fetch(`${this.root}${path}`, {
      ...init,
      headers,
      credentials: "omit",
    });
  }

  private httpError(
    status: number,
    payload: ErrorBody | null,
    path: string,
    method = "GET",
  ): Error {
    if (status === 401 && !isAuthChallenge(path, method)) {
      void this.setAccessToken(null);
      this.onUnauthorized?.();
    }
    return parseError(status, payload);
  }

  async login(
    username: string,
    password: string,
    options: { remember?: boolean; permanent?: boolean } = {},
  ): Promise<LoginResult> {
    const result = await this.request<LoginResult>("/auth/login", {
      method: "POST",
      body: JSON.stringify({
        username,
        password,
        remember: options.remember ?? false,
        permanent: options.permanent ?? false,
      }),
    });
    return this.rememberSession(result);
  }

  async loginOtp(token: string, otpCode: string): Promise<LoginResult> {
    const result = await this.request<LoginResult>("/auth/login/otp", {
      method: "POST",
      body: JSON.stringify({ token, otpCode }),
    });
    return this.rememberSession(result);
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

  async logout(): Promise<void> {
    try {
      await this.request<void>("/auth/logout", { method: "POST" });
    } finally {
      await this.setAccessToken(null);
    }
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
    const response = await this.send("/auth/otp/setup", {
      method: "POST",
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

  deletePasskey(id: number): Promise<void> {
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

  async finishPasskeyLogin(
    token: string,
    credential: AuthenticatePublicKeyCredentialJSON,
  ): Promise<LoginResult> {
    const result = await this.request<LoginResult>(
      "/auth/login/passkey/confirm",
      {
        method: "POST",
        body: JSON.stringify({ token, credential }),
      },
    );
    return this.rememberSession(result);
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
   * Bearer session. Returns 404 until the browser step has completed.
   */
  async finishNativeLogin(token: string): Promise<LoginResult> {
    const result = await this.request<LoginResult>("/auth/login/native", {
      method: "POST",
      body: JSON.stringify({ token }),
    });
    return this.rememberSession(result);
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

  /** Multipart upload via tauri-http (no XHR / CORS). */
  private async uploadForm<T>(
    path: string,
    body: FormData,
    _onProgress: UploadProgress | undefined,
    signal: AbortSignal | undefined,
    parse: (payload: unknown) => T | null,
  ): Promise<T> {
    if (signal?.aborted) {
      throw new DOMException("upload aborted", "AbortError");
    }

    const response = await this.send(path, {
      method: "POST",
      body,
      signal,
    });

    let payload: ErrorBody | null = null;
    try {
      payload =
        response.status === 204
          ? null
          : ((await response.json()) as ErrorBody);
    } catch {
      payload = null;
    }

    if (!response.ok) {
      throw this.httpError(response.status, payload, path);
    }

    const value = parse(payload);
    if (value === null) {
      throw this.httpError(response.status, payload, path);
    }
    return value;
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
    const response = await this.send(path);

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

  /** Authenticated download via tauri-http + blob object URL. */
  private async triggerDownload(
    path: string,
    name: string,
    init: RequestInit = {},
  ): Promise<void> {
    const response = await this.send(path, init);

    if (!response.ok) {
      let body: ErrorBody | null = null;
      try {
        body = (await response.json()) as ErrorBody;
      } catch {
        body = null;
      }
      throw this.httpError(response.status, body, path);
    }

    const blob = await response.blob();
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = name;
    anchor.rel = "noopener";
    document.body.appendChild(anchor);
    anchor.click();
    anchor.remove();
    URL.revokeObjectURL(url);
  }

  async download(rootId: string, id: string, name: string): Promise<void> {
    await this.triggerDownload(
      `/roots/${encodeURIComponent(rootId)}/files/${encodeURIComponent(id)}/blob/download`,
      name,
    );
  }

  async downloadRoot(rootId: string, name: string): Promise<void> {
    await this.triggerDownload(
      `/roots/${encodeURIComponent(rootId)}/download`,
      name,
    );
  }

  async downloadDir(
    rootId: string,
    ids: string[],
    name = "download.zip",
  ): Promise<void> {
    const params = new URLSearchParams({ ids: ids.join(",") });
    await this.triggerDownload(
      `/roots/${encodeURIComponent(rootId)}/files/download?${params}`,
      name,
    );
  }

  async sharedContent(fileId: string): Promise<Blob> {
    const path = `/files/shared/${encodeURIComponent(fileId)}/blob/chunk`;
    const response = await this.send(path, { credentials: "omit" });

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

  async sharedDownload(fileId: string, name: string): Promise<void> {
    await this.triggerDownload(
      `/files/shared/${encodeURIComponent(fileId)}/blob/download`,
      name,
      { credentials: "omit" },
    );
  }

  async icon(
    rootId: string,
    id: string,
    size: 64 | 256 = 64,
  ): Promise<Blob> {
    const path = `/roots/${encodeURIComponent(rootId)}/files/${encodeURIComponent(id)}/icon?size=${size}`;
    const response = await this.send(path);

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

  async sharedIcon(fileId: string, size: 64 | 256 = 64): Promise<Blob> {
    const path = `/files/shared/${encodeURIComponent(fileId)}/icon?size=${size}`;
    const response = await this.send(path, { credentials: "omit" });

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
    const response = await this.send(path, init);

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
    const response = await this.send(path, init);

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
