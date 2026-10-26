import { createRouter, type Routes } from "sv-router";

/** Routes shared by web and native. Public share pages are web-only extras. */
export const baseRoutes = {
  "/login": () => import("./pages/LoginPage.svelte"),
  "/login/passkey": () => import("./pages/PasskeyBridgePage.svelte"),
  "/reset-password": () => import("./pages/ResetPasswordPage.svelte"),
  "/reset-password/:token": () =>
    import("./pages/ResetPasswordTokenPage.svelte"),

  // Pages that need a signed-in user. SessionApp loads the user and renders the header.
  "/": {
    "/": () => import("./pages/HomeRedirect.svelte"),
    "/files": {
      "/": () => import("./pages/FilesOutlet.svelte"),
      "/trash": () => import("./pages/FilesOutlet.svelte"),
      "/:root_id": () => import("./pages/FilesOutlet.svelte"),
      "/:root_id/activity": () => import("./pages/FilesOutlet.svelte"),
      "/:root_id/dir/:parent_id": () => import("./pages/FilesOutlet.svelte"),
      layout: () => import("./pages/FilesPage.svelte"),
    },
    "/admin": {
      "/users": () => import("./pages/admin/UsersPage.svelte"),
      "/settings": () => import("./pages/admin/SettingsPage.svelte"),
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
    "/settings": () => import("./pages/SettingsPage.svelte"),
    layout: () => import("./components/SessionApp.svelte"),
  },

  "*": () => import("./pages/NotFoundPage.svelte"),
} satisfies Routes;

/** Open enough for web extras (public share routes) while keeping base typed. */
type RouterHandles = ReturnType<typeof createRouter<Routes>>;

export let navigate: RouterHandles["navigate"];
export let route: RouterHandles["route"];
export let isActive: RouterHandles["isActive"];
export let p: RouterHandles["p"];

let initialized = false;

/**
 * Create the app router once. Web passes public share routes; native does not.
 * Must run before `<Router />` mounts.
 */
export function initRouter(extraRoutes: Routes = {}): void {
  if (initialized) {
    throw new Error("initRouter called more than once");
  }
  initialized = true;

  const api = createRouter({
    ...baseRoutes,
    ...extraRoutes,
  });

  navigate = api.navigate;
  route = api.route;
  isActive = api.isActive;
  p = api.p;
}
