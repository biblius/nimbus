import { createRouter } from "sv-router";

export const { navigate, route, isActive } = createRouter({
  "/login": () => import("./pages/LoginPage.svelte"),
  "/reset-password": () => import("./pages/ResetPasswordPage.svelte"),
  "/reset-password/:token": () =>
    import("./pages/ResetPasswordTokenPage.svelte"),

  // Pages that need a signed-in user. SessionApp loads the user and renders the header.
  "/": {
    "/": () => import("./pages/HomeRedirect.svelte"),
    "/files": {
      "/": () => import("./pages/FilesOutlet.svelte"),
      "/:root_id": () => import("./pages/FilesOutlet.svelte"),
      "/:root_id/dir/:parent_id": () => import("./pages/FilesOutlet.svelte"),
      layout: () => import("./pages/FilesPage.svelte"),
    },
    "/admin": {
      "/users": () => import("./pages/admin/UsersPage.svelte"),
      "/folders": {
        "/private": {
          "/": () => import("./pages/admin/PrivateFoldersPage.svelte"),
          "/:folderId": () => import("./pages/admin/FolderPage.svelte"),
        },
        "/shared": {
          "/": () => import("./pages/admin/SharedFoldersPage.svelte"),
          "/:folderId": () => import("./pages/admin/FolderPage.svelte"),
        },
      },
      layout: () => import("./pages/admin/AdminLayout.svelte"),
    },
    layout: () => import("./components/SessionApp.svelte"),
  },

  "*": () => import("./pages/NotFoundPage.svelte"),
});
