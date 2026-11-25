use std::path::Path;

use serde::Serialize;
use star_rail_battle_core::BattleError;
use tauri::{AppHandle, Manager};

use crate::workbench::{BattleView, Error, ReplaySummary, archived_battles, archived_replay};

#[derive(Serialize)]
pub struct CommandError {
    code: &'static str,
}

impl From<Error> for CommandError {
    fn from(error: Error) -> Self {
        let code = match error {
            Error::ReplayNotFound(_) => "replay_not_found",
            Error::ReplayVersion(_)
            | Error::Battle(
                BattleError::UnsupportedSchema(_) | BattleError::UnsupportedVersion { .. },
            ) => "incompatible_version",
            _ => {
                eprintln!("战斗记录读取失败：{error}");
                "storage_failed"
            }
        };
        Self { code }
    }
}

async fn with_history<T: Send + 'static>(
    app: AppHandle,
    operation: impl FnOnce(&Path) -> Result<T, Error> + Send + 'static,
) -> Result<T, CommandError> {
    let unavailable = || CommandError {
        code: "storage_failed",
    };
    tauri::async_runtime::spawn_blocking(move || {
        let path = app.path().app_local_data_dir().map_err(|_| unavailable())?;
        operation(&path.join("training")).map_err(CommandError::from)
    })
    .await
    .map_err(|_| unavailable())?
}

#[tauri::command]
pub async fn battle_history(app: AppHandle) -> Result<Vec<ReplaySummary>, CommandError> {
    with_history(app, archived_battles).await
}

#[tauri::command]
pub async fn battle_replay(app: AppHandle, session_id: u32) -> Result<BattleView, CommandError> {
    with_history(app, move |path| archived_replay(path, session_id)).await
}
