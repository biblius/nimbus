import type { NimbusClient } from "../../api";
import type { UserView } from "../../types";

/** Bumped each time an admin opens a root from the file browser. */
export interface EditRequest {
  id: string;
  token: number;
}

export interface AdminContext {
  client: NimbusClient;
  editRequest: EditRequest | null;
  currentUserId: string;
  onAccountChanged: (account: UserView) => void;
  onAccountDeleted: (id: string) => void;
}

export interface AdminSidebarEntry {
  id: string;
  label: string;
}

/**
 * Sidebar entries for the admin panel.
 * Add a category here, then render it from the matching branch in AdminPanel.
 */
export const ADMIN_SIDEBAR_ENTRIES: AdminSidebarEntry[] = [
  { id: "users", label: "Users" },
  { id: "folders", label: "Folders" },
];
