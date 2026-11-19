use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use star_rail_profiles::{AccountInventory, Catalog, ImportedLightCone, LightCone, Loadout};

use crate::character_archive::Error;

use super::{ExportError, ExportOrigin, inventory::convert_relics, parse_snapshot};

#[derive(Deserialize)]
pub(super) struct AccountData {
    #[serde(default)]
    characters: Vec<Character>,
    #[serde(default)]
    light_cones: Vec<Cone>,
    metadata: Option<Metadata>,
    gacha: Option<Gacha>,
    materials: Option<Vec<Value>>,
    recon_relics: Option<Vec<u32>>,
}

#[derive(Deserialize)]
struct Metadata {
    uid: Option<u32>,
    trailblazer: Option<String>,
}

#[derive(Deserialize)]
struct Gacha {
    stellar_jade: u32,
    // The upstream JSON spells "oneiric" this way.
    oneric_shards: u32,
}

#[derive(Serialize)]
pub(crate) struct AccountSummary {
    uid: Option<u32>,
    trailblazer: Option<String>,
    characters: usize,
    light_cones: usize,
    materials: Option<usize>,
    stellar_jade: Option<u32>,
    oneiric_shards: Option<u32>,
    recon_relics: Option<usize>,
}

impl AccountData {
    fn uid(&self) -> Option<u32> {
        self.metadata.as_ref().and_then(|metadata| metadata.uid)
    }

    pub(super) fn summary(&self) -> AccountSummary {
        AccountSummary {
            uid: self.uid(),
            trailblazer: self
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.trailblazer.clone()),
            characters: self.characters.len(),
            light_cones: self.light_cones.len(),
            materials: self.materials.as_ref().map(Vec::len),
            stellar_jade: self.gacha.as_ref().map(|gacha| gacha.stellar_jade),
            oneiric_shards: self.gacha.as_ref().map(|gacha| gacha.oneric_shards),
            recon_relics: self.recon_relics.as_ref().map(Vec::len),
        }
    }
}

#[derive(Deserialize)]
struct Character {
    id: String,
    level: u16,
    ascension: u8,
    eidolon: u8,
    ability_version: Option<u32>,
    skills: BTreeMap<String, u16>,
    traces: BTreeMap<String, bool>,
    memosprite: Option<BTreeMap<String, u16>>,
    affinitive_eidolons: Option<Vec<u32>>,
}

#[derive(Deserialize)]
struct Cone {
    id: String,
    level: u16,
    ascension: u8,
    superimposition: u8,
    location: String,
    _uid: String,
}

// Called after validating the envelope and size through parse_snapshot. Retain the original
// inventory object so Archiver fields that the simulator doesn't model survive saving.
pub(super) fn read_account(
    snapshot: &str,
    origin: ExportOrigin,
) -> Result<Option<(Value, AccountData)>, ExportError> {
    let mut raw: Value =
        serde_json::from_str(snapshot).map_err(|_| ExportError::InvalidSnapshot)?;
    if matches!(origin, ExportOrigin::Live) {
        raw = raw["data"].take();
    }
    let fields = [
        "characters",
        "light_cones",
        "metadata",
        "gacha",
        "materials",
        "recon_relics",
    ];
    if raw.get("format").is_some() || !fields.iter().any(|field| raw.get(field).is_some()) {
        return Ok(None);
    }
    let account = AccountData::deserialize(&raw).map_err(|_| ExportError::InvalidSnapshot)?;
    Ok(Some((raw, account)))
}

pub(crate) fn convert_snapshot(
    snapshot: &str,
    origin: ExportOrigin,
    catalog: &Catalog,
) -> Result<AccountInventory, Error> {
    let export = parse_snapshot(snapshot, origin)?;
    let full = read_account(snapshot, origin)?;
    let mut inventory = AccountInventory {
        relics: convert_relics(export.relics, catalog)?,
        ..AccountInventory::default()
    };
    let Some((_, account)) = full else {
        return Ok(inventory);
    };
    inventory.uid = account.uid();
    let sources: BTreeMap<_, _> = catalog
        .characters
        .iter()
        .flat_map(|(id, character)| character.source_ids.iter().map(move |source| (source, id)))
        .collect();
    let mut seen = BTreeSet::new();
    for character in account.characters {
        let Some(id) = sources.get(&character.id) else {
            inventory.skipped_characters += 1;
            continue;
        };
        if !seen.insert(*id) {
            return Err(ExportError::InvalidSnapshot.into());
        }
        if let Some(loadout) = character_loadout(catalog, id, character)? {
            inventory.characters.insert((*id).clone(), loadout);
        } else {
            inventory.skipped_characters += 1;
        }
    }
    let mut cone_ids = BTreeSet::new();
    for cone in account.light_cones {
        if cone._uid.trim().is_empty() || !cone_ids.insert(cone._uid.clone()) {
            return Err(ExportError::InvalidSnapshot.into());
        }
        let data_id = format!("cone.{}", cone.id);
        if !catalog.light_cones.contains_key(&data_id) {
            inventory.skipped_light_cones += 1;
            continue;
        }
        inventory.light_cones.push(ImportedLightCone {
            source_uid: cone._uid,
            owner_id: sources.get(&cone.location).map(|id| (*id).clone()),
            cone: LightCone {
                data_id,
                level: cone.level,
                ascension: cone.ascension,
                superimposition: cone.superimposition,
            },
        });
    }
    Ok(inventory)
}

fn character_loadout(
    catalog: &Catalog,
    id: &str,
    source: Character,
) -> Result<Option<Loadout>, star_rail_profiles::Error> {
    let character = catalog.character(id)?;
    let variant_id = match source.ability_version {
        None => &character.default_variant,
        Some(0) => "base",
        Some(1) => "novaflare",
        Some(_) => return Ok(None),
    };
    // Archiver reports level zero when it hasn't resolved a form's base character yet.
    if source.level == 0 {
        return Ok(None);
    }
    let Some(variant) = character.variants.get(variant_id) else {
        return Ok(None);
    };
    let mut loadout = catalog.default_loadout_at(id, variant_id, source.level, source.ascension)?;
    loadout.config.eidolon = source.eidolon;
    loadout.affinitive_eidolon = character
        .affinitive_eidolon
        .as_ref()
        .is_some_and(|eidolon| {
            eidolon
                .id
                .strip_prefix("affinitive.")
                .and_then(|id| id.parse().ok())
                .is_some_and(|id| {
                    source
                        .affinitive_eidolons
                        .as_ref()
                        .is_some_and(|ids| ids.contains(&id))
                })
        });
    for (id, level) in &mut loadout.config.skill_levels {
        let skills = &source.skills;
        let memosprite = source.memosprite.as_ref();
        let value = match variant.skills[id].kind.as_str() {
            "Normal" => skills.get("basic"),
            "BPSkill" => skills.get("skill"),
            "Ultra" => skills.get("ult"),
            "Talent" => skills.get("talent"),
            "ElationDamage" => skills.get("elation"),
            "MemospriteSkill" => memosprite.and_then(|levels| levels.get("skill")),
            "MemospriteTalent" => memosprite.and_then(|levels| levels.get("talent")),
            _ => None,
        };
        *level = value.copied().unwrap_or(1).max(1);
    }
    loadout
        .config
        .unlocked_trace_ids
        .retain(|id| trace_key(id).is_some_and(|key| source.traces.get(&key) == Some(&true)));
    catalog.validate_loadout(id, &loadout)?;
    Ok(Some(loadout))
}

// These suffixes match the v4 Archiver point anchors in the 4.7 catalog,
// including the Novaflare and Trailblazer variants.
fn trace_key(id: &str) -> Option<String> {
    let suffix: u16 = id.get(id.len().checked_sub(3)?..)?.parse().ok()?;
    match suffix {
        101..=103 => Some(format!("ability_{}", suffix - 100)),
        201..=210 => Some(format!("stat_{}", suffix - 200)),
        501 => Some("special".into()),
        _ => None,
    }
}
