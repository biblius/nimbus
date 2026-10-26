use tauri::{
    webview::{
        cookie::{time::Duration, SameSite},
        Cookie, Url,
    },
    AppHandle, Manager,
};

/// Must match server `auth::session::SESSION_COOKIE_NAME`.
const SESSION_COOKIE_NAME: &str = "nimbus_session";

/// Persist the Nimbus session cookie in the webview jar so `<video>` / `<audio>`
/// can authenticate chunked blob streams (Bearer is not sent by media elements).
///
/// Always `Secure` (matches the server). `SameSite=None` so the Tauri shell can
/// attach the cookie on cross-origin media requests to the API host.
/// Soup's cookie manager requires a host string; that scopes storage to the API
/// host (host-only style), not a subdomain `Domain=` attribute.
#[tauri::command]
pub async fn set_session_cookie(
    app: AppHandle,
    origin: String,
    value: String,
) -> Result<(), String> {
    // Cookie jar APIs must not run on the async command worker (deadlocks on
    // WebView2; GTK main-iteration must stay on the UI thread via the runtime queue).
    tauri::async_runtime::spawn_blocking(move || set_session_cookie_blocking(app, origin, value))
        .await
        .map_err(|error| error.to_string())?
}

fn set_session_cookie_blocking(
    app: AppHandle,
    origin: String,
    value: String,
) -> Result<(), String> {
    let url = Url::parse(&origin).map_err(|error| error.to_string())?;
    let host = url
        .host_str()
        .ok_or_else(|| "origin has no host".to_string())?
        .to_string();

    let cookie = Cookie::build((SESSION_COOKIE_NAME, value))
        .domain(host)
        .path("/")
        .http_only(true);

    #[cfg(debug_assertions)]
    let cookie = cookie.secure(false).same_site(SameSite::Lax);

    #[cfg(not(debug_assertions))]
    let cookie = cookie.secure(true).same_site(SameSite::None);

    let cookie = cookie.max_age(Duration::days(400)).build();

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main webview not found".to_string())?;

    // Runtime `set_cookie` is fire-and-forget (errors only logged). Queue it,
    // then confirm via `cookies_for_url`, which waits on the main-thread queue.
    window
        .set_cookie(cookie)
        .map_err(|error| error.to_string())?;

    let stored = window
        .cookies_for_url(url)
        .map_err(|error| error.to_string())?;

    if !stored
        .iter()
        .any(|cookie| cookie.name() == SESSION_COOKIE_NAME)
    {
        return Err(format!(
            "webview rejected session cookie for {origin} (Secure + SameSite=None)"
        ));
    }

    Ok(())
}

#[tauri::command]
pub async fn clear_session_cookie(app: AppHandle, origin: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || clear_session_cookie_blocking(app, origin))
        .await
        .map_err(|error| error.to_string())?
}

fn clear_session_cookie_blocking(app: AppHandle, origin: String) -> Result<(), String> {
    let url = Url::parse(&origin).map_err(|error| error.to_string())?;
    let host = url
        .host_str()
        .ok_or_else(|| "origin has no host".to_string())?
        .to_string();

    let cookie = Cookie::build((SESSION_COOKIE_NAME, ""))
        .domain(host)
        .path("/")
        .build();

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main webview not found".to_string())?;

    window
        .delete_cookie(cookie)
        .map_err(|error| error.to_string())
}
