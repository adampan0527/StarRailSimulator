use std::{
    ffi::OsString,
    fs, io,
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
};

use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewWindow};
use windows_sys::Win32::UI::Controls::Dialogs::{
    CommDlgExtendedError, GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST,
    OPENFILENAMEW,
};

const SETTINGS: &str = "archiver-path.json";
const BUNDLED_DIRECTORY: &str = "archiver-v0.20.0";
const BUNDLED_FILENAME: &str = "reliquary-archiver-pktmon-x64.exe";
const BUNDLED_LICENSE: &[u8] = include_bytes!("../third-party/reliquary-archiver/LICENSE");
#[cfg(bundled_archiver)]
const BUNDLED_PROGRAM: Option<&[u8]> = Some(include_bytes!(
    "../../.archiver/reliquary-archiver-pktmon-x64.exe"
));
#[cfg(not(bundled_archiver))]
const BUNDLED_PROGRAM: Option<&[u8]> = None;

#[derive(Default)]
pub(crate) struct ArchiverLauncher(Mutex<Option<Child>>);

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LaunchError {
    DialogFailed,
    InvalidProgram,
    LaunchFailed,
    BundledUnavailable,
    SettingsUnavailable,
}

#[tauri::command]
pub(crate) async fn archiver_open(app: AppHandle) -> Result<(), LaunchError> {
    tauri::async_runtime::spawn_blocking(move || {
        let directory = app
            .path()
            .app_local_data_dir()
            .map_err(|_| LaunchError::LaunchFailed)?;
        app.state::<ArchiverLauncher>().open(&directory)
    })
    .await
    .map_err(|_| LaunchError::LaunchFailed)?
}

#[tauri::command]
pub(crate) async fn archiver_choose(window: WebviewWindow) -> Result<bool, LaunchError> {
    let owner = window.hwnd().map_err(|_| LaunchError::DialogFailed)?.0 as isize;
    let app = window.app_handle().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let directory = app
            .path()
            .app_local_data_dir()
            .map_err(|_| LaunchError::SettingsUnavailable)?;
        let Some(path) = choose_program(owner)? else {
            return Ok(false);
        };
        if !is_archiver(&path) {
            return Err(LaunchError::InvalidProgram);
        }
        let bytes = serde_json::to_vec(&path).map_err(|_| LaunchError::SettingsUnavailable)?;
        fs::create_dir_all(&directory)
            .and_then(|()| replace_file(&directory.join(SETTINGS), &bytes))
            .map_err(|_| LaunchError::SettingsUnavailable)?;
        Ok(true)
    })
    .await
    .map_err(|_| LaunchError::DialogFailed)?
}

impl ArchiverLauncher {
    fn open(&self, directory: &Path) -> Result<(), LaunchError> {
        let mut launched = self.0.lock().map_err(|_| LaunchError::LaunchFailed)?;
        if let Some(child) = launched.as_mut()
            && child
                .try_wait()
                .map_err(|_| LaunchError::LaunchFailed)?
                .is_none()
        {
            return Ok(());
        }
        let (path, bundled) = match configured_program(directory) {
            Some(path) => (path, false),
            None => (install_bundled_program(directory)?, true),
        };
        let parent = path.parent().ok_or(LaunchError::InvalidProgram)?;
        let mut command = Command::new(&path);
        command.current_dir(parent);
        if bundled {
            // PktMon treats EOF as consent to show the Windows elevation prompt.
            command.stdin(Stdio::null());
        }
        *launched = Some(command.spawn().map_err(|_| LaunchError::LaunchFailed)?);
        Ok(())
    }
}

fn configured_program(directory: &Path) -> Option<PathBuf> {
    let json = fs::read(directory.join(SETTINGS)).ok()?;
    serde_json::from_slice::<PathBuf>(&json)
        .ok()
        .filter(|path| is_archiver(path))
}

fn install_bundled_program(directory: &Path) -> Result<PathBuf, LaunchError> {
    let bytes = BUNDLED_PROGRAM.ok_or(LaunchError::BundledUnavailable)?;
    let directory = directory.join(BUNDLED_DIRECTORY);
    let program = directory.join(BUNDLED_FILENAME);
    fs::create_dir_all(&directory)
        .and_then(|()| install_file(&directory.join("LICENSE"), BUNDLED_LICENSE))
        .and_then(|()| install_file(&program, bytes))
        .map_err(|_| LaunchError::BundledUnavailable)?;
    Ok(program)
}

fn install_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let matches = || fs::read(path).is_ok_and(|installed| installed == bytes);
    if matches() {
        return Ok(());
    }
    // Another simulator instance may finish installing and launch the same file first.
    replace_file(path, bytes).or_else(|error| if matches() { Ok(()) } else { Err(error) })
}

fn replace_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let pending = path.with_extension(format!("{}.tmp", std::process::id()));
    let result = fs::write(&pending, bytes).and_then(|()| fs::rename(&pending, path));
    if result.is_err() {
        let _ = fs::remove_file(pending);
    }
    result
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workbench_tests::TestData;

    #[test]
    fn reads_only_existing_archiver_overrides() {
        let data = TestData::in_directory(Path::new(env!("CARGO_MANIFEST_DIR")));
        let directory = data.path();
        let settings = directory.join(SETTINGS);
        let program = directory.join("reliquary-archiver-custom.exe");
        replace_file(&settings, &serde_json::to_vec(&program).unwrap()).unwrap();
        assert!(configured_program(directory).is_none());

        fs::write(&program, b"fixture").unwrap();
        assert_eq!(configured_program(directory), Some(program));

        replace_file(&settings, b"invalid JSON").unwrap();
        assert!(configured_program(directory).is_none());
    }

    #[cfg(bundled_archiver)]
    #[test]
    fn installs_and_repairs_the_bundled_program_with_its_license() {
        let data = TestData::in_directory(Path::new(env!("CARGO_MANIFEST_DIR")));
        let directory = data.path();
        assert!(configured_program(directory).is_none());
        let program = install_bundled_program(directory).unwrap();
        assert_eq!(
            program,
            directory.join(BUNDLED_DIRECTORY).join(BUNDLED_FILENAME)
        );
        assert_eq!(
            fs::read(program.with_file_name("LICENSE")).unwrap(),
            BUNDLED_LICENSE
        );
        let expected = BUNDLED_PROGRAM.unwrap();
        assert_eq!(fs::read(&program).unwrap(), expected);
        fs::write(&program, b"incomplete copy").unwrap();
        assert_eq!(install_bundled_program(directory).unwrap(), program);
        assert_eq!(fs::read(&program).unwrap(), expected);
    }
}
