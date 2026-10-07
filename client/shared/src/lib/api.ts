import type { FileEntry, Folders, LoginResult, UserView } from "./types";

export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly body: unknown,
  ) {
    super(`HTTP ${status}`);
  }
}

export interface NimbusClient {
  login(username: string, password: string, remember: boolean): Promise<LoginResult>;
  loginOtp(token: string, otpCode: string): Promise<LoginResult>;
  me(): Promise<UserView>;
  logout(): Promise<void>;
  changePassword(currentPassword: string, newPassword: string): Promise<void>;
  resetPassword(token: string, newPassword: string): Promise<void>;
  folders(): Promise<Folders>;
  list(parent: string): Promise<FileEntry[]>;
  createDirectory(parent: string, name: string): Promise<FileEntry>;
  /** Upload one file. `onProgress` reports bytes sent so far and the total body size. */
  upload(parent: string, file: File, onProgress?: UploadProgress): Promise<FileEntry>;
  remove(id: string): Promise<void>;
  contentUrl(id: string): string;
}

export type UploadProgress = (loaded: number, total: number) => void;

export function createClient(baseUrl: string): NimbusClient {
  const root = baseUrl.replace(/\/$/, "");

  async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
    const headers = new Headers(init.headers);
    headers.set("X-NIMBUS-CSRF", "1");
    if (init.body && !(init.body instanceof FormData) && !headers.has("Content-Type")) {
      headers.set("Content-Type", "application/json");
    }

    const response = await fetch(`${root}${path}`, {
      ...init,
      headers,
      credentials: "include",
    });

    if (!response.ok) {
      let body: unknown = null;
      try {
        body = await response.json();
      } catch {
        body = null;
      }
      throw new ApiError(response.status, body);
    }

    if (response.status === 204) {
      return undefined as T;
    }
    return (await response.json()) as T;
  }

  return {
    login: (username, password, remember) =>
      request<LoginResult>("/auth/login", {
        method: "POST",
        body: JSON.stringify({ username, password, remember }),
      }),
    loginOtp: (token, otpCode) =>
      request<LoginResult>("/auth/login/2fa", {
        method: "POST",
        body: JSON.stringify({ token, otpCode }),
      }),
    me: () => request<UserView>("/auth/me"),
    logout: () => request<void>("/auth/logout", { method: "POST" }),
    changePassword: (currentPassword, newPassword) =>
      request<void>("/auth/password", {
        method: "POST",
        body: JSON.stringify({ currentPassword, newPassword }),
      }),
    resetPassword: (token, newPassword) =>
      request<void>(`/auth/reset/${token}`, {
        method: "POST",
        body: JSON.stringify({ newPassword }),
      }),
    folders: () => request<Folders>("/folders"),
    list: (parent) => request<FileEntry[]>(`/files?parent=${parent}`),
    createDirectory: (parent, name) =>
      request<FileEntry>("/files/directories", {
        method: "POST",
        body: JSON.stringify({ parent, name }),
      }),
    upload: (parent, file, onProgress) => {
      const body = new FormData();
      body.set("parent", parent);
      body.set("file", file);

      // fetch has no upload progress events, so this one call uses XMLHttpRequest.
      return new Promise<FileEntry>((resolve, reject) => {
        const xhr = new XMLHttpRequest();
        xhr.open("POST", `${root}/files`);
        xhr.withCredentials = true;
        xhr.setRequestHeader("X-NIMBUS-CSRF", "1");

        xhr.upload.onprogress = (event) => {
          if (event.lengthComputable) onProgress?.(event.loaded, event.total);
        };

        xhr.onload = () => {
          let payload: unknown = null;
          try {
            payload = xhr.responseText ? JSON.parse(xhr.responseText) : null;
          } catch {
            payload = null;
          }
          if (xhr.status >= 200 && xhr.status < 300 && Array.isArray(payload) && payload[0]) {
            onProgress?.(file.size, file.size);
            resolve(payload[0] as FileEntry);
          } else {
            reject(new ApiError(xhr.status, payload));
          }
        };
        xhr.onerror = () => reject(new TypeError("network error"));
        xhr.onabort = () => reject(new DOMException("upload aborted", "AbortError"));

        xhr.send(body);
      });
    },
    remove: (id) => request<void>(`/files/${id}`, { method: "DELETE" }),
    contentUrl: (id) => `${root}/files/${id}/content`,
  };
}
