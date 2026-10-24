import type { LoginBackoffBody } from "./types";

/** JSON body of every `AppError` response. */
export type ErrorBody<T = unknown> = {
  message: string | null;
  code: number;
  body: T | null;
};

/** One `validify::ValidationError::Field` inside an `ErrorBody`. */
export type ValidationField = {
  field: string | null;
  code: string;
  params: Record<string, unknown>;
  message: string | null;
  location: string;
};

/** One `validify::ValidationError::Schema` inside an `ErrorBody`. */
export type ValidationSchema = {
  code: string;
  message: string | null;
  location: string;
};

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

/** `AppError::Auth` (including login backoff). */
const AUTH_CODE = 1;

type LoginBackoffError = ApiError & {
  body: ErrorBody<LoginBackoffBody> & { body: LoginBackoffBody };
};

/**
 * True when `error` is a 401 login backoff with a parseable `unlockAt`.
 * Distinguishes `AuthFailure::LoginBackoff` from other auth 401s (same code).
 */
export function isLoginBackoff(error: unknown): error is LoginBackoffError {
  if (!(error instanceof ApiError) || error.status !== 401) {
    return false;
  }

  const payload = error.body;
  if (!payload || payload.code !== AUTH_CODE || payload.body == null) {
    return false;
  }

  const unlockAt = (payload.body as LoginBackoffBody).unlockAt;
  return typeof unlockAt === "string" && unlockAt.length > 0;
}

/** Parsed `unlockAt` from a login-backoff `ApiError`, or `null` if missing/invalid. */
export function loginBackoffUnlockAt(error: unknown): Date | null {
  if (!isLoginBackoff(error)) {
    return null;
  }

  const date = new Date(error.body.body.unlockAt);
  return Number.isNaN(date.getTime()) ? null : date;
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
      if ("Field" in item && item.Field.field === name) {
        return item.Field;
      }
    }
  }
}

export type UploadProgress = (loaded: number, total: number) => void;

/** `AppError::Validation`. `Unprocessable` uses the same code with no field list. */
export const VALIDATION_CODE = 4;

export const OTP_TOKEN_HEADER = "x-otp-token";

/**
 * Paths where 401 means bad credentials / OTP / current password - not a dead session.
 * These must not trigger a redirect to login.
 */
export const AUTH_CHALLENGE_PATHS = new Set([
  "/auth/login",
  "/auth/login/otp",
  "/auth/login/native",
  "/auth/login/passkey",
  "/auth/login/passkey/confirm",
  "/auth/login/passkey/confirm/native",
  "/auth/password",
  "/auth/otp/setup/confirm",
  "/auth/otp/disable",
]);

/** PATCH /auth/me returns 401 when the current password is wrong. */
export function isAuthChallenge(path: string, method: string): boolean {
  if (AUTH_CHALLENGE_PATHS.has(path)) {
    return true;
  }
  return path === "/auth/me" && method.toUpperCase() === "PATCH";
}

export function parseError(status: number, payload: ErrorBody | null): Error {
  if (payload?.code === VALIDATION_CODE && payload.body) {
    return new ValidationError(payload as ErrorBody<ValidationItem[]>);
  }
  return new ApiError(status, payload);
}
