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

/** One page from a paginated list. */
export type Page<T> = {
  total: number;
  items: T[];
};

/** Column and text for one search. */
export type ListSearch = {
  column: string;
  q: string;
};

/** Column and direction for one sort. */
export type ListSort = {
  column: string;
  direction: "asc" | "desc";
};

/** Query for a paginated, searchable, sortable list. */
export type ListQuery = {
  page: number;
  perPage: number;
  search?: ListSearch;
  sort?: ListSort;
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

/**
 * File types from the API. The single key is the family and the value is the format variant
 * (`"Png"`, `"SevenZ"`). `other` holds the raw extension instead of a variant.
 */
export type FileType =
  | {
      app:
        | "Wasm"
        | "Elf"
        | "Exe"
        | "Dll"
        | "Class"
        | "Bc"
        | "Mach"
        | "Dex"
        | "Dey"
        | "Der"
        | "Obj"
        | "Pem"
        | "Qcow2";
    }
  | { book: "Epub" | "Mobi" }
  | {
      image:
        | "Jpg"
        | "Jp2"
        | "Png"
        | "Gif"
        | "Webp"
        | "Cr2"
        | "Tif"
        | "Bmp"
        | "Jxr"
        | "Psd"
        | "Ico"
        | "Heif"
        | "Avif"
        | "Jxl"
        | "Ora"
        | "Djvu"
        | "Dwg";
    }
  | {
      video:
        "Mp4" | "M4v" | "Mkv" | "Webm" | "Mov" | "Avi" | "Wmv" | "Mpg" | "Flv";
    }
  | {
      audio:
        | "Midi"
        | "Mp3"
        | "M4a"
        | "Opus"
        | "Ogg"
        | "Flac"
        | "Wav"
        | "Amr"
        | "Aac"
        | "Aiff"
        | "Dsf"
        | "Ape";
    }
  | { font: "Woff" | "Woff2" | "Ttf" | "Otf" }
  | {
      doc:
        | "Doc"
        | "Docx"
        | "Xls"
        | "Xlsx"
        | "Ppt"
        | "Pptx"
        | "Odt"
        | "Ods"
        | "Odp";
    }
  | {
      archive:
        | "Zip"
        | "Tar"
        | "Par2"
        | "Rar"
        | "Gz"
        | "Bz2"
        | "Bz3"
        | "SevenZ"
        | "Xz"
        | "Pdf"
        | "Swf"
        | "Rtf"
        | "Eot"
        | "Ps"
        | "Sqlite"
        | "Nes"
        | "Crx"
        | "Cab"
        | "Deb"
        | "Ar"
        | "Z"
        | "Lz"
        | "Rpm"
        | "Dcm"
        | "Zst"
        | "Lz4"
        | "Msi"
        | "Cpio";
    }
  | { text: "Html" | "Xml" | "Sh" | "Txt" | "Json" | "Md" | "Csv" | "Css" }
  | { other: string };

export type FileTree = {
  id: string;
  name: string;
  kind: "file" | "directory";
  /** Detected format for files; null for directories. */
  fileType?: FileType | null;
  /** Content hash of the generated preview blob, when one exists. */
  preview?: string | null;
  children: FileTree[];
};

export type FileEntry = {
  id: string;
  rootId: string;
  createdBy: string | null;
  kind: "file" | "directory";
  name: string;
  parent: string | null;
  fileType: FileType | null;
  hash: string | null;
  sizeBytes: number;
  preview: string | null;
  createdAt: string;
  updatedAt: string;
};

/** Preview fields mirrored from `files.preview` for listings. */
export type FileMetadata = {
  preview: string | null;
};

/** Directory listing row: file entry with optional joined metadata. */
export type FileView = {
  file: FileEntry;
  metadata: FileMetadata | null;
};

/** One file the deletion worker finished attempting. */
export type DeletionResult = {
  requestId: string;
  fileId: string;
  name: string;
  success: boolean;
};

/** Messages on the deletion SSE channel. */
export type DeletionEvent =
  | { type: "progress"; result: DeletionResult }
  | { type: "end" };
