#[derive(serde::Serialize)]
pub(crate) struct DesktopStatus {
    version: &'static str,
}

#[tauri::command]
pub(crate) fn desktop_status() -> DesktopStatus {
    DesktopStatus {
        version: env!("CARGO_PKG_VERSION"),
    }
}
