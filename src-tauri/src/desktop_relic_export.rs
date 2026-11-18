use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Manager, Url};

mod inventory;
pub(crate) use inventory::inventory_snapshot;

const MAX_SNAPSHOT_BYTES: usize = 8 * 1024 * 1024;
const ARCHIVER_SOURCE: &str = "reliquary_archiver";
const ARCHIVER_VERSION: u32 = 4;
const RELIC_DOCUMENT_FORMAT: &str = "star-rail-simulator.relic-export";
const RELIC_DOCUMENT_VERSION: u32 = 1;

#[derive(Deserialize)]
#[serde(tag = "event", content = "data")]
enum ExportEvent {
    InitialScan(SourceExport),
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExportOrigin {
    Live,
    File,
}

#[derive(Deserialize)]
struct SourceExport {
    source: String,
    version: u32,
    relics: Vec<Relic>,
}

#[derive(Deserialize)]
struct ExportFile {
    format: Option<String>,
    source_version: Option<u32>,
    #[serde(flatten)]
    export: SourceExport,
}

#[derive(Serialize)]
struct RelicDocument<'a> {
    format: &'static str,
    version: u32,
    source: &'a str,
    source_version: u32,
    relics: &'a [Relic],
}

#[derive(Deserialize, Serialize)]
struct Relic {
    #[serde(flatten)]
    identifier: RelicIdentifier,
    name: String,
    slot: String,
    rarity: u32,
    level: u32,
    mainstat: String,
    substats: Vec<Substat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reroll_substats: Option<Vec<Substat>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    preview_substats: Option<Vec<Substat>>,
    location: String,
    lock: bool,
    discard: bool,
    _uid: String,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum RelicIdentifier {
    Set { set_id: String },
    Legacy { id: String },
}

#[derive(Deserialize, Serialize)]
struct Substat {
    key: String,
    value: f64,
    count: u32,
    step: u32,
}

#[derive(Serialize)]
pub(crate) struct SavedRelicExport {
    file_url: String,
    relic_count: usize,
}

#[derive(Debug, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExportError {
    #[error("invalid relic snapshot")]
    InvalidSnapshot,
    #[error("unsupported relic snapshot format")]
    UnsupportedFormat,
    #[error("relic export storage unavailable")]
    StorageUnavailable,
}

#[tauri::command]
pub(crate) async fn relic_export_save(
    app: tauri::AppHandle,
    snapshot: String,
    origin: ExportOrigin,
) -> Result<SavedRelicExport, ExportError> {
    tauri::async_runtime::spawn_blocking(move || {
        let directory = app
            .path()
            .app_local_data_dir()
            .map_err(|_| ExportError::StorageUnavailable)?
            .join("relic-exports");
        save_snapshot(&directory, &snapshot, origin)
    })
    .await
    .map_err(|_| ExportError::StorageUnavailable)?
}

fn parse_snapshot(snapshot: &str, origin: ExportOrigin) -> Result<SourceExport, ExportError> {
    if snapshot.len() > MAX_SNAPSHOT_BYTES {
        return Err(ExportError::InvalidSnapshot);
    }
    let export = match origin {
        ExportOrigin::Live => {
            let ExportEvent::InitialScan(export) =
                serde_json::from_str(snapshot).map_err(|_| ExportError::InvalidSnapshot)?;
            export
        }
        ExportOrigin::File => {
            let ExportFile {
                format,
                source_version,
                mut export,
            } = serde_json::from_str(snapshot).map_err(|_| ExportError::InvalidSnapshot)?;
            if let Some(format) = format {
                if format != RELIC_DOCUMENT_FORMAT || export.version != RELIC_DOCUMENT_VERSION {
                    return Err(ExportError::UnsupportedFormat);
                }
                export.version = source_version.ok_or(ExportError::InvalidSnapshot)?;
            }
            export
        }
    };
    if export.source != ARCHIVER_SOURCE || export.version != ARCHIVER_VERSION {
        return Err(ExportError::UnsupportedFormat);
    }
    Ok(export)
}

fn save_snapshot(
    directory: &Path,
    snapshot: &str,
    origin: ExportOrigin,
) -> Result<SavedRelicExport, ExportError> {
    let export = parse_snapshot(snapshot, origin)?;
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ExportError::StorageUnavailable)?;
    let filename = format!("relics-{}-{}", elapsed.as_nanos(), std::process::id());
    let path = directory.join(format!("{filename}.json"));
    let pending_path = directory.join(format!("{filename}.tmp"));
    let file_url = Url::from_file_path(&path).map_err(|()| ExportError::StorageUnavailable)?;
    fs::create_dir_all(directory).map_err(|_| ExportError::StorageUnavailable)?;
    let mut pending = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&pending_path)
        .map_err(|_| ExportError::StorageUnavailable)?;

    let document = RelicDocument {
        format: RELIC_DOCUMENT_FORMAT,
        version: RELIC_DOCUMENT_VERSION,
        source: &export.source,
        source_version: export.version,
        relics: &export.relics,
    };
    let written = serde_json::to_writer(&mut pending, &document)
        .and_then(|()| pending.sync_all().map_err(serde_json::Error::io));
    drop(pending);
    if written.is_err() || fs::rename(&pending_path, &path).is_err() {
        let _ = fs::remove_file(&pending_path);
        return Err(ExportError::StorageUnavailable);
    }
    Ok(SavedRelicExport {
        file_url: file_url.into(),
        relic_count: export.relics.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_full_snapshots_only() {
        assert!(parse_snapshot(
            r#"{"event":"InitialScan","data":{"source":"reliquary_archiver","version":4,"relics":[]}}"#,
            ExportOrigin::Live
        )
        .is_ok());
        assert!(matches!(
            parse_snapshot(r#"{"event":"UpdateRelics","data":[]}"#, ExportOrigin::Live),
            Err(ExportError::InvalidSnapshot)
        ));
        assert!(matches!(
            parse_snapshot(
                r#"{"event":"InitialScan","data":{"source":"reliquary_archiver","version":5,"relics":[]}}"#,
                ExportOrigin::Live
            ),
            Err(ExportError::UnsupportedFormat)
        ));
    }

    #[test]
    fn accepts_archiver_json_when_live_export_is_unavailable() {
        let file = r#"{"source":"reliquary_archiver","version":4,"relics":[]}"#;
        assert!(parse_snapshot(file, ExportOrigin::File).is_ok());
        assert!(matches!(
            parse_snapshot(file, ExportOrigin::Live),
            Err(ExportError::InvalidSnapshot)
        ));
        assert!(matches!(
            parse_snapshot(
                r#"{"source":"reliquary_archiver","version":4}"#,
                ExportOrigin::File
            ),
            Err(ExportError::InvalidSnapshot)
        ));
    }

    #[test]
    fn accepts_archiver_set_id_in_files_and_live_scans() {
        let relic = serde_json::json!({
            "set_id": "102",
            "name": "Musketeer of Wild Wheat",
            "slot": "Feet",
            "rarity": 5,
            "level": 15,
            "mainstat": "ATK_",
            "substats": [{ "key": "SPD", "value": 2.6, "count": 1, "step": 2 }],
            "preview_substats": [],
            "location": "1001",
            "lock": true,
            "discard": false,
            "_uid": "fixture-relic-1"
        });
        let mut legacy_relic = relic.clone();
        let fields = legacy_relic.as_object_mut().unwrap();
        fields.remove("set_id");
        fields.insert("id".into(), serde_json::json!("61011"));
        let expected = serde_json::json!([relic, legacy_relic]);
        let export = serde_json::json!({
            "source": "reliquary_archiver",
            "version": 4,
            "relics": &expected
        });
        let live = serde_json::json!({ "event": "InitialScan", "data": &export });

        for (snapshot, origin) in [
            (export.to_string(), ExportOrigin::File),
            (live.to_string(), ExportOrigin::Live),
        ] {
            let export = parse_snapshot(&snapshot, origin).unwrap();
            assert_eq!(serde_json::to_value(&export.relics).unwrap(), expected);
        }
    }

    #[test]
    fn reopens_saved_relic_document_from_file() {
        let saved = serde_json::json!({
            "format": "star-rail-simulator.relic-export",
            "version": 1,
            "source": "reliquary_archiver",
            "source_version": 4,
            "relics": [{
                "set_id": "102",
                "name": "Musketeer of Wild Wheat",
                "slot": "Feet",
                "rarity": 5,
                "level": 15,
                "mainstat": "ATK_",
                "substats": [{ "key": "SPD", "value": 2.6, "count": 1, "step": 2 }],
                "location": "1001",
                "lock": true,
                "discard": false,
                "_uid": "fixture-relic-1"
            }]
        });
        let export = parse_snapshot(&saved.to_string(), ExportOrigin::File).unwrap();
        assert_eq!(export.version, 4);
        assert_eq!(
            serde_json::to_value(&export.relics).unwrap(),
            saved["relics"]
        );

        let live = serde_json::json!({ "event": "InitialScan", "data": &saved });
        assert!(matches!(
            parse_snapshot(&live.to_string(), ExportOrigin::Live),
            Err(ExportError::UnsupportedFormat)
        ));
    }

    #[test]
    fn rejects_unknown_or_incomplete_saved_relic_documents() {
        for file in [
            r#"{"format":"other","version":4,"source":"reliquary_archiver","relics":[]}"#,
            r#"{"format":"star-rail-simulator.relic-export","version":2,"source":"reliquary_archiver","source_version":4,"relics":[]}"#,
            r#"{"format":"star-rail-simulator.relic-export","version":1,"source":"reliquary_archiver","source_version":5,"relics":[]}"#,
        ] {
            assert!(matches!(
                parse_snapshot(file, ExportOrigin::File),
                Err(ExportError::UnsupportedFormat)
            ));
        }
        assert!(matches!(
            parse_snapshot(
                r#"{"format":"star-rail-simulator.relic-export","version":1,"source":"reliquary_archiver","relics":[]}"#,
                ExportOrigin::File
            ),
            Err(ExportError::InvalidSnapshot)
        ));
    }
}
