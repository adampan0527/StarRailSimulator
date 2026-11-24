use std::sync::Mutex;

use serde::Serialize;
use star_rail_profiles::{
    Error as ProfileError, InventoryLightCone, InventoryRelic, LightCone, Relic,
    RelicImportSummary, SavedTeam, TeamSlots,
};
use tauri::{AppHandle, Manager};

use crate::account_archives::{AccountArchives, AccountList, ImportedAccount};
use crate::character_archive::{
    CharacterArchive, CharacterEdit, CharacterProfile, CharacterSummary, Error, Index,
    RelicOptions, TeamDprReadiness,
};
use crate::desktop_relic_export::{ExportError, ExportOrigin};

#[derive(Default)]
pub struct CharacterState(Mutex<Option<AccountArchives>>);

#[derive(Serialize)]
pub struct CommandError {
    code: &'static str,
}

impl From<Error> for CommandError {
    fn from(error: Error) -> Self {
        let code = match &error {
            Error::MissingCatalog => "catalog_missing",
            Error::Catalog(_) | Error::CatalogSize => "catalog_invalid",
            Error::Profile(ProfileError::Version { .. }) => "catalog_mismatch",
            Error::TeamName => "invalid_team_name",
            Error::AccountSelection => "invalid_account_selection",
            Error::AccountRemark => "invalid_account_remark",
            Error::AccountUidMissing => "archiver_uid_missing",
            Error::Dpr(_) => {
                eprintln!("固定靶场计算失败：{error}");
                "dpr_failed"
            }
            Error::RelicSnapshot(ExportError::InvalidSnapshot) => "invalid_relic_snapshot",
            Error::RelicSnapshot(ExportError::UnsupportedFormat) => "unsupported_relic_snapshot",
            Error::Profile(ProfileError::Invalid {
                field: "archiver.uid",
                ..
            }) => "archiver_account_mismatch",
            Error::Profile(ProfileError::Invalid {
                field: "eidolon" | "affinitive_eidolon",
                ..
            }) => "invalid_eidolon",
            Error::Profile(ProfileError::Invalid {
                field: "inventory.occupied",
                ..
            }) => "relic_occupied",
            Error::Profile(ProfileError::Invalid {
                field: "inventory.equipped",
                ..
            }) => "relic_equipped",
            Error::Profile(ProfileError::Invalid {
                field: "team.id" | "team.members",
                ..
            })
            | Error::Profile(ProfileError::NotFound { kind: "team", .. }) => "invalid_team",
            Error::Profile(ProfileError::Invalid { field, .. }) if field.starts_with("relic.") => {
                "invalid_relic"
            }
            Error::Profile(ProfileError::Invalid {
                field: "light_cone.occupied",
                ..
            }) => "light_cone_occupied",
            Error::Profile(ProfileError::Invalid {
                field: "light_cone.equipped",
                ..
            }) => "light_cone_equipped",
            Error::Profile(ProfileError::Invalid { field, .. })
                if field.starts_with("light_cone.") =>
            {
                "invalid_light_cone"
            }
            Error::Profile(ProfileError::NotFound {
                kind: "light cone" | "inventory light cone",
                ..
            }) => "invalid_light_cone",
            Error::Profile(ProfileError::NotFound { kind: "relic", .. }) => "invalid_relic",
            Error::Profile(ProfileError::Invalid { .. } | ProfileError::NotFound { .. }) => {
                "invalid_profile"
            }
            _ => {
                eprintln!("角色档案操作失败：{error}");
                "storage_failed"
            }
        };
        Self { code }
    }
}

async fn with_state<T: Send + 'static>(
    app: AppHandle,
    operation: impl FnOnce(&mut AccountArchives) -> Result<T, Error> + Send + 'static,
) -> Result<T, CommandError> {
    let unavailable = || CommandError {
        code: "storage_failed",
    };
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<CharacterState>();
        let mut loaded = state.0.lock().map_err(|_| unavailable())?;
        let archives = match loaded.as_mut() {
            Some(archives) => archives,
            None => {
                let path = app.path().app_local_data_dir().map_err(|_| unavailable())?;
                loaded.insert(AccountArchives::open(&path.join("characters"))?)
            }
        };
        Ok(operation(archives)?)
    })
    .await
    .map_err(|_| unavailable())?
}

async fn with_archive<T: Send + 'static>(
    app: AppHandle,
    scope: Option<String>,
    operation: impl FnOnce(&mut CharacterArchive) -> Result<T, Error> + Send + 'static,
) -> Result<T, CommandError> {
    with_state(app, move |archives| {
        archives.check_scope(scope.as_deref())?;
        operation(archives.active_archive()?)
    })
    .await
}

#[tauri::command]
pub(crate) async fn account_list(app: AppHandle) -> Result<AccountList, CommandError> {
    with_state(app, |archives| Ok(archives.list())).await
}

#[tauri::command]
pub(crate) async fn account_select(app: AppHandle, uid: u32) -> Result<AccountList, CommandError> {
    with_state(app, move |archives| archives.select(uid)).await
}

#[tauri::command]
pub(crate) async fn account_rename(
    app: AppHandle,
    uid: u32,
    remark: String,
) -> Result<AccountList, CommandError> {
    with_state(app, move |archives| archives.rename(uid, &remark)).await
}

#[tauri::command]
pub async fn character_index(app: AppHandle, scope: Option<String>) -> Result<Index, CommandError> {
    with_archive(app, scope, |archive| archive.index()).await
}

#[tauri::command]
pub async fn character_dpr_readiness(
    app: AppHandle,
    scope: Option<String>,
    team_id: String,
) -> Result<TeamDprReadiness, CommandError> {
    with_archive(app, scope, move |archive| archive.dpr_readiness(&team_id)).await
}

#[tauri::command]
pub async fn character_roster(
    app: AppHandle,
    scope: Option<String>,
) -> Result<Vec<CharacterSummary>, CommandError> {
    with_archive(app, scope, |archive| archive.roster()).await
}

#[tauri::command]
pub async fn character_import_catalog(
    app: AppHandle,
    scope: Option<String>,
    json: String,
) -> Result<Index, CommandError> {
    with_state(app, move |archives| {
        archives.check_scope(scope.as_deref())?;
        archives.import_catalog(&json)
    })
    .await
}

#[tauri::command]
pub async fn character_profile(
    app: AppHandle,
    scope: Option<String>,
    character_id: String,
) -> Result<CharacterProfile, CommandError> {
    with_archive(app, scope, move |archive| archive.profile(&character_id)).await
}

#[tauri::command]
pub async fn character_save(
    app: AppHandle,
    scope: Option<String>,
    character_id: String,
    edit: CharacterEdit,
) -> Result<
    (
        CharacterProfile,
        Vec<InventoryRelic>,
        Vec<InventoryLightCone>,
    ),
    CommandError,
> {
    with_archive(app, scope, move |archive| {
        let profile = archive.save(&character_id, edit)?;
        Ok((
            profile,
            archive.inventory()?,
            archive.light_cone_inventory()?,
        ))
    })
    .await
}

#[tauri::command]
pub async fn character_save_team(
    app: AppHandle,
    scope: Option<String>,
    id: Option<String>,
    name: String,
    slots: TeamSlots,
) -> Result<SavedTeam, CommandError> {
    with_archive(app, scope, move |archive| {
        archive.save_team(id.as_deref(), &name, &slots)
    })
    .await
}

#[tauri::command]
pub async fn relic_inventory(
    app: AppHandle,
    scope: Option<String>,
) -> Result<Vec<InventoryRelic>, CommandError> {
    with_archive(app, scope, |archive| archive.inventory()).await
}

#[tauri::command]
pub async fn relic_inventory_import(
    app: AppHandle,
    scope: Option<String>,
    snapshot: String,
) -> Result<(RelicImportSummary, Vec<InventoryRelic>), CommandError> {
    with_archive(app, scope, move |archive| {
        let summary = archive.import_relics(&snapshot)?;
        Ok((summary, archive.inventory()?))
    })
    .await
}

#[tauri::command]
pub(crate) async fn archiver_import(
    app: AppHandle,
    scope: Option<String>,
    snapshot: String,
    origin: ExportOrigin,
) -> Result<ImportedAccount, CommandError> {
    with_state(app, move |archives| {
        archives.check_scope(scope.as_deref())?;
        archives.import(&snapshot, origin)
    })
    .await
}

#[tauri::command]
pub async fn light_cone_inventory(
    app: AppHandle,
    scope: Option<String>,
) -> Result<Vec<InventoryLightCone>, CommandError> {
    with_archive(app, scope, |archive| archive.light_cone_inventory()).await
}

#[tauri::command]
pub async fn light_cone_save(
    app: AppHandle,
    scope: Option<String>,
    id: Option<u32>,
    cone: LightCone,
) -> Result<Vec<InventoryLightCone>, CommandError> {
    with_archive(app, scope, move |archive| {
        archive.save_light_cone(id, &cone)
    })
    .await
}

#[tauri::command]
pub async fn light_cone_delete(
    app: AppHandle,
    scope: Option<String>,
    id: u32,
) -> Result<Vec<InventoryLightCone>, CommandError> {
    with_archive(app, scope, move |archive| archive.delete_light_cone(id)).await
}

#[tauri::command]
pub async fn relic_save(
    app: AppHandle,
    scope: Option<String>,
    id: Option<u32>,
    relic: Relic,
) -> Result<Vec<InventoryRelic>, CommandError> {
    with_archive(app, scope, move |archive| archive.save_relic(id, &relic)).await
}

#[tauri::command]
pub async fn relic_delete(
    app: AppHandle,
    scope: Option<String>,
    id: u32,
) -> Result<Vec<InventoryRelic>, CommandError> {
    with_archive(app, scope, move |archive| archive.delete_relic(id)).await
}

#[tauri::command]
pub async fn character_relic_options(
    app: AppHandle,
    scope: Option<String>,
    relic_id: String,
    level: u16,
) -> Result<RelicOptions, CommandError> {
    with_archive(app, scope, move |archive| {
        archive.relic_options(&relic_id, level)
    })
    .await
}
