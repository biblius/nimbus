export type Role = "admin" | "user";

export type SessionView = {
  id: string;
  remoteAddress: string | null;
  userAgent: string | null;
  createdAt: string;
  updatedAt: string;
  expiresAt: string | null;
  firstLogin: boolean;
  user: UserViewFull;
};

export type UserViewFull = UserView & {
  otpEnabled: boolean;
  updatedAt: string;
};

export type UserView = {
  id: string;
  username: string;
  email: string | null;
  role: Role;
  createdAt: string;
};

/**
 * Data used when creating or modifying a user.
 * Parameters are not allowed to be null when creating.
 * If any parameter is null when updating, left as is.
 */
export type UserData = {
  username?: string;
  email?: string;
  role: Role;
  password?: string;
};

export type LoginResult =
  | { status: "session"; session: SessionView }
  | { status: "twoFactorRequired"; token: string };

export type Root = {
  id: string;
  name: string;
  quota: number;
  /** Set for a user's private root. Null for a shared root. */
  owner: string | null;
  createdAt: string;
  updatedAt: string;
};

/** A user assigned to a shared root. */
export type Member = {
  id: string;
  username: string;
  email: string | null;
  role: Role;
  createdAt: string;
};

export type RootData = {
  name: string;
  quotaBytes: number;
  members: string[];
};

export type CreateUserInput = {
  username: string;
  password: string;
  email?: string;
  role: Role;
};

export type UpdateUserInput = {
  username: string;
  email: string | null;
  role: Role;
  password?: string;
};

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

export type FileEntry = {
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
};
