use std::collections::{BTreeMap, BTreeSet};

use star_rail_profiles::{
    Affix, Catalog, Error, ImportedRelic, Relic as InventoryRelic, RelicRolls, RelicSlot,
    RelicSource,
};

use crate::character_archive::Error as ArchiveError;

use super::{ExportOrigin, Relic, RelicIdentifier, Substat, parse_snapshot};

pub(crate) fn inventory_snapshot(
    snapshot: &str,
    catalog: &Catalog,
) -> Result<Vec<ImportedRelic>, ArchiveError> {
    let export = parse_snapshot(snapshot, ExportOrigin::File)?;
    let mut seen = BTreeSet::new();
    export
        .relics
        .into_iter()
        .map(|relic| {
            if relic._uid.trim().is_empty() || !seen.insert(relic._uid.clone()) {
                return Err(invalid("missing or repeated source instance ID").into());
            }
            Ok(convert(relic, catalog)?)
        })
        .collect()
}

fn convert(relic: Relic, catalog: &Catalog) -> Result<ImportedRelic, Error> {
    let slot = match relic.slot.as_str() {
        "Head" => RelicSlot::Head,
        "Hands" => RelicSlot::Hands,
        "Body" => RelicSlot::Body,
        "Feet" => RelicSlot::Feet,
        "Planar Sphere" => RelicSlot::PlanarSphere,
        "Link Rope" => RelicSlot::LinkRope,
        _ => return Err(invalid("unknown slot")),
    };
    let (id, data) = match &relic.identifier {
        RelicIdentifier::Set { set_id } => {
            let set = format!("set.{set_id}");
            let mut matching = catalog.relics.iter().filter(|(_, data)| {
                data.set_id == set && data.slot == slot && u32::from(data.rarity) == relic.rarity
            });
            let entry = matching
                .next()
                .ok_or_else(|| invalid("unknown relic set"))?;
            if matching.next().is_some() {
                return Err(invalid("ambiguous relic set"));
            }
            entry
        }
        RelicIdentifier::Legacy { id } => catalog
            .relics
            .get_key_value(&format!("relic.{id}"))
            .filter(|(_, data)| data.slot == slot && u32::from(data.rarity) == relic.rarity)
            .ok_or_else(|| invalid("unknown relic ID"))?,
    };
    let level = u16::try_from(relic.level).map_err(|_| invalid("invalid level"))?;
    let main = match (relic.mainstat.as_str(), slot) {
        ("HP", RelicSlot::Head) => "hp_flat",
        ("ATK", RelicSlot::Hands) => "attack_flat",
        ("HP" | "HP_", _) => "hp_ratio",
        ("ATK" | "ATK_", _) => "attack_ratio",
        ("DEF" | "DEF_", _) => "defense_ratio",
        (key, _) => property(key).ok_or_else(|| invalid("unknown main stat"))?,
    };
    let group = &catalog.sub_affixes[&data.sub_group];
    let mut substats = BTreeMap::new();
    let mut rolls = BTreeMap::new();
    for stat in &relic.substats {
        let (name, value) = convert_substat(stat, group)?;
        if substats.insert(name.into(), value).is_some() {
            return Err(invalid("repeated substat"));
        }
        rolls.insert(
            name.into(),
            RelicRolls {
                count: stat.count,
                step: stat.step,
            },
        );
    }
    let preview_substats = relic
        .preview_substats
        .as_ref()
        .map(|preview| {
            let mut result = BTreeMap::new();
            for stat in preview {
                let (name, value) = convert_substat(stat, group)?;
                if name == main
                    || substats.contains_key(name)
                    || result.insert(name.into(), value).is_some()
                {
                    return Err(invalid("repeated preview substat"));
                }
            }
            Ok(result)
        })
        .transpose()?;
    Ok(ImportedRelic {
        source_uid: relic._uid,
        relic: InventoryRelic {
            data_id: id.clone(),
            level,
            main_stat: main.into(),
            substats,
        },
        source: RelicSource {
            location: relic.location,
            rolls,
            preview_substats,
        },
    })
}

fn convert_substat(
    stat: &Substat,
    group: &BTreeMap<String, Affix>,
) -> Result<(&'static str, u32), Error> {
    let name = property(&stat.key).ok_or_else(|| invalid("unknown substat"))?;
    let affix = group
        .get(name)
        .ok_or_else(|| invalid("unsupported substat"))?;
    Ok((name, substat_micros(stat, *affix)?))
}

fn substat_micros(stat: &Substat, affix: Affix) -> Result<u32, Error> {
    if stat.count == 0 || u64::from(stat.step) > u64::from(affix.step_count) * u64::from(stat.count)
    {
        return Err(invalid("invalid substat rolls"));
    }
    let value = affix.base * f64::from(stat.count) + affix.step * f64::from(stat.step);
    let source_value = if stat.key.ends_with('_') {
        stat.value / 100.0
    } else {
        stat.value
    };
    // Archiver computes stat values in f32. Recover the catalog value from the roll counts.
    let tolerance = value.abs() * f64::from(f32::EPSILON) * 4.0;
    let micros = (value * 1_000_000.0).round();
    if (value - source_value).abs() > tolerance || !(1.0..=f64::from(u32::MAX)).contains(&micros) {
        return Err(invalid("substat value does not match its rolls"));
    }
    Ok(micros as u32)
}

fn property(key: &str) -> Option<&'static str> {
    Some(match key {
        "HP" => "hp_flat",
        "ATK" => "attack_flat",
        "DEF" => "defense_flat",
        "HP_" => "hp_ratio",
        "ATK_" => "attack_ratio",
        "DEF_" => "defense_ratio",
        "SPD" => "speed",
        "CRIT Rate" | "CRIT Rate_" => "crit_rate",
        "CRIT DMG" | "CRIT DMG_" => "crit_damage",
        "Effect Hit Rate" | "Effect Hit Rate_" => "statusprobabilitybase",
        "Effect RES" | "Effect RES_" => "statusresistancebase",
        "Break Effect" | "Break Effect_" => "breakdamageaddedratiobase",
        "Energy Regeneration Rate" => "spratiobase",
        "Outgoing Healing Boost" => "healratiobase",
        "Physical DMG Boost" => "physicaladdedratio",
        "Fire DMG Boost" => "fireaddedratio",
        "Ice DMG Boost" => "iceaddedratio",
        "Lightning DMG Boost" => "thunderaddedratio",
        "Wind DMG Boost" => "windaddedratio",
        "Quantum DMG Boost" => "quantumaddedratio",
        "Imaginary DMG Boost" => "imaginaryaddedratio",
        _ => return None,
    })
}

fn invalid(detail: &str) -> Error {
    Error::Invalid {
        field: "relic.import",
        detail: detail.into(),
    }
}
