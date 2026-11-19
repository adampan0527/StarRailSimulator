use std::{
    ffi::OsString,
    fs,
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::Mutex,
};

use serde::Serialize;
use tauri::{Manager, WebviewWindow};
use windows_sys::Win32::UI::Controls::Dialogs::{
    CommDlgExtendedError, GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST,
    OPENFILENAMEW,
};

const SETTINGS: &str = "archiver-path.json";

#[derive(Default)]
pub(crate) struct ArchiverLauncher(Mutex<Option<Child>>);

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LaunchResult {
    Opened,
    AlreadyOpen,
    Canceled,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LaunchError {
    DialogFailed,
    InvalidProgram,
    LaunchFailed,
    AlreadyOpen,
}

#[tauri::command]
pub(crate) async fn archiver_open(
    window: WebviewWindow,
    reselect: bool,
) -> Result<LaunchResult, LaunchError> {
    let owner = window.hwnd().map_err(|_| LaunchError::DialogFailed)?.0 as isize;
    let app = window.app_handle().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let settings = app
            .path()
            .app_local_data_dir()
            .map_err(|_| LaunchError::LaunchFailed)?
            .join(SETTINGS);
        app.state::<ArchiverLauncher>()
            .open(&settings, owner, reselect)
    })
    .await
    .map_err(|_| LaunchError::LaunchFailed)?
}

impl ArchiverLauncher {
    fn open(
        &self,
        settings: &Path,
        owner: isize,
        reselect: bool,
    ) -> Result<LaunchResult, LaunchError> {
        let mut launched = self.0.lock().map_err(|_| LaunchError::LaunchFailed)?;
        if let Some(child) = launched.as_mut()
            && child
                .try_wait()
                .map_err(|_| LaunchError::LaunchFailed)?
                .is_none()
        {
            return if reselect {
                Err(LaunchError::AlreadyOpen)
            } else {
                Ok(LaunchResult::AlreadyOpen)
            };
        }
        let configured = (!reselect)
            .then(|| fs::read(settings).ok())
            .flatten()
            .and_then(|json| serde_json::from_slice::<PathBuf>(&json).ok())
            .filter(|path| is_archiver(path));
        let path = match configured {
            Some(path) => path,
            None => match choose_program(owner)? {
                Some(path) => path,
                None => return Ok(LaunchResult::Canceled),
            },
        };
        if !is_archiver(&path) {
            return Err(LaunchError::InvalidProgram);
        }
        let parent = path.parent().ok_or(LaunchError::InvalidProgram)?;
        *launched = Some(
            Command::new(&path)
                .current_dir(parent)
                .spawn()
                .map_err(|_| LaunchError::LaunchFailed)?,
        );
        // A settings write failure only means that the user must choose the program again.
        if let Some(directory) = settings.parent()
            && let Ok(json) = serde_json::to_vec(&path)
            && fs::create_dir_all(directory).is_ok()
        {
            let _ = fs::write(settings, json);
        }
        Ok(LaunchResult::Opened)
    }
}

fn is_archiver(path: &Path) -> bool {
    path.is_absolute()
        && path.is_file()
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                let name = name.to_ascii_lowercase();
                name == "reliquary-archiver.exe"
                    || (name.starts_with("reliquary-archiver-") && name.ends_with(".exe"))
            })
}

fn choose_program(owner: isize) -> Result<Option<PathBuf>, LaunchError> {
    let mut file = vec![0_u16; 32_768];
    let filter: Vec<_> = "Reliquary Archiver\0reliquary-archiver*.exe\0\0"
        .encode_utf16()
        .collect();
    let title: Vec<_> = "选择 Reliquary Archiver 程序\0".encode_utf16().collect();
    let mut dialog = OPENFILENAMEW {
        lStructSize: size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner as _,
        lpstrFilter: filter.as_ptr(),
        lpstrTitle: title.as_ptr(),
        lpstrFile: file.as_mut_ptr(),
        nMaxFile: file.len() as u32,
        Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        ..OPENFILENAMEW::default()
    };
    // SAFETY: The title and filter are NUL-terminated. All buffers outlive the synchronous
    // call, and the only writable buffer is allocated for nMaxFile UTF-16 code units.
    if unsafe { GetOpenFileNameW(&mut dialog) } != 0 {
        let end = file
            .iter()
            .position(|unit| *unit == 0)
            .ok_or(LaunchError::DialogFailed)?;
        return Ok(Some(PathBuf::from(OsString::from_wide(&file[..end]))));
    }
    // SAFETY: Queries the result of the common dialog on the same thread.
    if unsafe { CommDlgExtendedError() } == 0 {
        Ok(None)
    } else {
        Err(LaunchError::DialogFailed)
    }
}
