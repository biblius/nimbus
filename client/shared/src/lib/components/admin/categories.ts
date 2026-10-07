import type { Component } from "svelte";
import type { NimbusClient } from "../../api";
import type { UserView } from "../../types";
import FoldersPage from "./FoldersPage.svelte";
import UsersPage from "./UsersPage.svelte";

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
  component: Component<{ context: AdminContext }>;
}

/**
 * Sidebar entries for the admin panel.
 * Add a category by appending a page component here.
 */
export const ADMIN_SIDEBAR_ENTRIES: AdminSidebarEntry[] = [
  { id: "users", label: "Users", component: UsersPage },
  { id: "folders", label: "Folders", component: FoldersPage },
];
