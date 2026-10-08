import type {
  CreateUserInput,
  FileEntry,
  Folder,
  LoginResult,
  UserView,
  Root,
  UserViewFull,
  UserData,
} from "./types";

/** JSON body of every `AppError` response. */
export interface ErrorBody<T = unknown> {
  message: string | null;
  code: number;
  body: T | null;
}

/** One `validify::ValidationError::Field` inside an `ErrorBody`. */
export interface ValidationField {
  field: string | null;
  code: string;
  params: Record<string, unknown>;
  message: string | null;
  location: string;
}

/** One `validify::ValidationError::Schema` inside an `ErrorBody`. */
export interface ValidationSchema {
  code: string;
  message: string | null;
  location: string;
}

/** `validify::ValidationErrors`, serialized as an externally tagged enum array. */
export type ValidationItem =
  { Field: ValidationField } | { Schema: ValidationSchema };

export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly body: ErrorBody | null,
  ) {
    super(body?.message ?? `HTTP ${status}`);
  }
}

/** `AppError::Validation`: code 4 with a list of field errors. */
export class ValidationError extends Error {
  constructor(readonly body: ErrorBody<ValidationItem[]>) {
    super(body.message ?? "Validation error");
  }

  /**
   * Returns the validation error for the given field, if any.
   *
   */
  field(name: string): ValidationField | undefined {
    for (const item of this.body.body ?? []) {
      if ("Field" in item && item.Field.field === name) return item.Field;
    }
  }
}

/** `AppError::Validation`. `Unprocessable` uses the same code with no field list. */
const VALIDATION_CODE = 4;

function parseError(status: number, payload: ErrorBody | null): Error {
  if (payload?.code === VALIDATION_CODE && payload.body) {
    return new ValidationError(payload as ErrorBody<ValidationItem[]>);
  }
  return new ApiError(status, payload);
}

export type UploadProgress = (loaded: number, total: number) => void;

export class NimbusClient {
  readonly #root: string;

  constructor(baseUrl: string) {
    this.#root = baseUrl.replace(/\/$/, "");
  }

  login(
    username: string,
    password: string,
    remember: boolean,
  ): Promise<LoginResult> {
    return this.request<LoginResult>("/auth/login", {
      method: "POST",
      body: JSON.stringify({ username, password, remember }),
    });
  }

  loginOtp(token: string, otpCode: string): Promise<LoginResult> {
    return this.request<LoginResult>("/auth/login/2fa", {
      method: "POST",
      body: JSON.stringify({ token, otpCode }),
    });
  }

  me(): Promise<UserViewFull> {
    return this.request<UserViewFull>("/auth/me");
  }

  logout(): Promise<void> {
    return this.request<void>("/auth/logout", { method: "POST" });
  }

  changePassword(currentPassword: string, newPassword: string): Promise<void> {
    return this.request<void>("/auth/password", {
      method: "POST",
      body: JSON.stringify({ currentPassword, newPassword }),
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

  list(rootId: string, parent: string | null): Promise<FileEntry[]> {
    const query = parent ? `?parent=${encodeURIComponent(parent)}` : "";
    return this.request<FileEntry[]>(
      `/roots/${encodeURIComponent(rootId)}${query}`,
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
  ): Promise<FileEntry> {
    const body = new FormData();

    if (parent) body.set("parent", parent);

    body.set("file", file);

    // fetch has no upload progress events, so this one call uses XMLHttpRequest.
    return new Promise<FileEntry>((resolve, reject) => {
      const xhr = new XMLHttpRequest();

      xhr.open("POST", `${this.#root}/roots/${encodeURIComponent(rootId)}`);

      xhr.withCredentials = true;

      xhr.setRequestHeader("X-NIMBUS-CSRF", "1");

      xhr.upload.onprogress = (event) => {
        if (event.lengthComputable) onProgress?.(event.loaded, event.total);
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

        if (
          xhr.status >= 200 &&
          xhr.status < 300 &&
          Array.isArray(payload) &&
          payload[0]
        ) {
          onProgress?.(file.size, file.size);
          resolve(payload[0] as FileEntry);
        } else {
          reject(parseError(xhr.status, payload));
        }
      };

      xhr.onerror = () => reject(new TypeError("network error"));

      xhr.onabort = () =>
        reject(new DOMException("upload aborted", "AbortError"));

      xhr.send(body);
    });
  }

  remove(rootId: string, id: string): Promise<void> {
    return this.request<void>(
      `/roots/${encodeURIComponent(rootId)}/files/${encodeURIComponent(id)}`,
      { method: "DELETE" },
    );
  }

  async download(rootId: string, id: string, name: string): Promise<void> {
    const response = await fetch(
      `${this.#root}/roots/${encodeURIComponent(rootId)}/files/${encodeURIComponent(id)}/download`,
      {
        credentials: "include",
        headers: { "X-NIMBUS-CSRF": "1" },
      },
    );
    if (!response.ok) {
      let body: ErrorBody | null = null;
      try {
        body = (await response.json()) as ErrorBody;
      } catch {
        body = null;
      }
      throw parseError(response.status, body);
    }

    const url = URL.createObjectURL(await response.blob());
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = name;
    anchor.click();
    URL.revokeObjectURL(url);
  }

  listUsers(): Promise<UserViewFull[]> {
    return this.request<UserViewFull[]>("/users");
  }

  createUser(input: CreateUserInput): Promise<UserViewFull> {
    return this.request<UserViewFull>("/users", {
      method: "POST",
      body: JSON.stringify(input),
    });
  }

  updateUser(id: string, input: UserData): Promise<UserView> {
    return this.request<UserView>(`/users/${encodeURIComponent(id)}`, {
      method: "PATCH",
      body: JSON.stringify(input),
    });
  }

  deleteUser(id: string): Promise<void> {
    return this.request<void>(`/users/${encodeURIComponent(id)}`, {
      method: "DELETE",
    });
  }

  listFolders(): Promise<Folder[]> {
    return this.request<Folder[]>("/admin/roots");
  }

  updateFolder(id: string, name: string, quotaBytes: number): Promise<Folder> {
    return this.request<Folder>(`/roots/${encodeURIComponent(id)}`, {
      method: "PUT",
      body: JSON.stringify({ name, quotaBytes }),
    });
  }

  async request<T>(path: string, init: RequestInit = {}): Promise<T> {
    const headers = new Headers(init.headers);

    headers.set("X-NIMBUS-CSRF", "1");

    if (
      init.body &&
      !(init.body instanceof FormData) &&
      !headers.has("Content-Type")
    ) {
      headers.set("Content-Type", "application/json");
    }

    const response = await fetch(`${this.#root}${path}`, {
      ...init,
      headers,
      credentials: "include",
    });

    if (!response.ok) {
      let body: ErrorBody | null = null;
      try {
        body = (await response.json()) as ErrorBody;
      } catch {
        body = null;
      }
      throw parseError(response.status, body);
    }

    if (response.status === 204) {
      return undefined as T;
    }

    return (await response.json()) as T;
  }
}
