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
  primary: boolean;
  quota: number;
  createdAt: string;
  updatedAt: string;
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
