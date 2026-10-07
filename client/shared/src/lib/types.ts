export type Role = "admin" | "user";

export interface UserViewFull {
  id: string;
  username: string;
  email: string | null;
  role: Role;
  otpEnabled: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface SessionView {
  id: string;
  remoteAddress: string | null;
  userAgent: string | null;
  createdAt: string;
  updatedAt: string;
  expiresAt: string | null;
  firstLogin: boolean;
  user: UserViewFull;
}

export interface UserView {
  id: string;
  username: string;
  email: string | null;
  role: Role;
  createdAt: string;
}

/**
 * Data used when creating or modifying a user.
 * Parameters are not allowed to be null when creating.
 * If any parameter is null when updating, left as is.
 */
export interface UserData {
  username?: string;
  email?: string;
  role: Role;
  password?: string;
}

export type LoginResult =
  | { status: "session"; session: SessionView }
  | { status: "twoFactorRequired"; token: string };

export interface Root {
  id: string;
  name: string;
  primary: boolean;
  quota: number;
  createdAt: string;
  updatedAt: string;
}

/** A root as stored, without the caller's assignment flag. */
export interface Folder {
  id: string;
  name: string;
  quota: number;
  createdAt: string;
  updatedAt: string;
}

export interface CreateUserInput {
  username: string;
  password: string;
  email?: string;
  role: Role;
}

export interface UpdateUserInput {
  username: string;
  email: string | null;
  role: Role;
  password?: string;
}

export type FileFamily =
  "Image" | "Audio" | "Video" | "Text" | "Doc" | "Unknown";

export type FileFormat =
  | "Jpg"
  | "Png"
  | "Wav"
  | "Mp3"
  | "Avi"
  | "Mp4"
  | "Txt"
  | "Json"
  | "Html"
  | "Xml"
  | "Md"
  | "Csv"
  | "Css"
  | "Rtf"
  | "Doc"
  | "Docx"
  | "Odt"
  | "Ods"
  | "Odp"
  | "Pdf"
  | "Xls"
  | "Xlsx"
  | "Ppt"
  | "Pptx"
  | { Other: string };

export interface FileEntry {
  id: string;
  rootId: string;
  createdBy: string;
  kind: "file" | "directory";
  name: string;
  parent: string | null;
  family: FileFamily | null;
  format: FileFormat | null;
  hash: string | null;
  sizeBytes: number;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
}
