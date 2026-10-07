export type Role = "admin" | "user";

export interface UserView {
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
  user: UserView;
}

export type LoginResult =
  | { status: "session"; session: SessionView }
  | { status: "twoFactorRequired"; token: string };

export interface Root {
  id: string;
  name: string;
  createdAt: string;
  updatedAt: string;
}

export interface FolderSpace {
  root: Root;
  primary: boolean;
  quotaBytes: number;
}

export interface FileEntry {
  id: string;
  rootId: string;
  createdBy: string;
  kind: "file" | "directory";
  name: string;
  parent: string | null;
  family: string | null;
  format: string | null;
  hash: string | null;
  sizeBytes: number;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
}

export interface Folders {
  privateFolder: FolderSpace;
  shared: FolderSpace[];
}

