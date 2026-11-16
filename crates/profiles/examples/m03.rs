use std::{
    error::Error,
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use star_rail_profiles::{Archive, Catalog, FrozenTeam, Team};

const ACHERON: &str = "character.1308";
const AHA: &str = "character.1511";
const CONE: &str = "cone.23024";

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [command, catalog] if command == "audit" => audit(&read_catalog(catalog)?),
        [command, catalog, character] if command == "show" => {
            let data = read_catalog(catalog)?;
            let form = data.character(character)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "character": form,
                    "level_80": form.base_attributes(80, 6)?,
                    "default_loadout": data.default_loadout(character)?,
                }))?
            );
            Ok(())
        }
        [command, catalog, db, old] if command == "demo-write" => {
            demo_write(read_catalog(catalog)?, db.as_ref(), old.as_ref())
        }
        [command, catalog, db, old] if command == "demo-reopen" => {
            demo_reopen(read_catalog(catalog)?, db.as_ref(), old.as_ref())
        }
        _ => Err("usage: m03 audit <catalog> | show <catalog> <form> | demo-write/demo-reopen <catalog> <db> <old-snapshot>".into()),
    }
}

fn read_catalog(path: &str) -> Result<Catalog, Box<dyn Error>> {
    Ok(Catalog::from_json(&fs::read_to_string(path)?)?)
}

fn audit(catalog: &Catalog) -> Result<(), Box<dyn Error>> {
    let mut variants = 0;
    let mut skills = 0;
    let mut traces = 0;
    let mut affinitive = 0;
    for (id, character) in &catalog.characters {
        character.base_attributes(80, 6)?;
        for (variant_id, variant) in &character.variants {
            for (level, ascension) in [
                (1, 0),
                (20, 1),
                (30, 2),
                (40, 3),
                (50, 4),
                (60, 5),
                (70, 6),
                (80, 6),
            ] {
                let config = catalog.default_loadout_at(id, variant_id, level, ascension)?;
                catalog.validate_loadout(id, &config)?;
            }
            variants += 1;
            skills += variant.skills.len();
            traces += variant.traces.len();
        }
        if character.affinitive_eidolon.is_some() {
            let mut config = catalog.default_loadout(id)?;
            assert_eq!(config.config.eidolon, 0, "{id}");
            config.affinitive_eidolon = true;
            catalog.validate_loadout(id, &config)?;
            affinitive += 1;
        }
    }
    for (id, cone) in &catalog.light_cones {
        cone.base_attributes(80, 6)?;
        let default = catalog.default_light_cone(id)?;
        assert_eq!((default.level, default.superimposition), (80, 1), "{id}");
        for rank in 1..=5 {
            cone.rank(rank)?;
        }
    }
    if catalog.game_version == "4.7.0" {
        audit_release_samples(catalog)?;
    }
    println!(
        "{}: {} forms, {variants} kits, {skills} skill entries, {traces} trace entries, {affinitive} affinitive eidolons, {} cones × 5, {} relics",
        catalog.data_version,
        catalog.characters.len(),
        catalog.light_cones.len(),
        catalog.relics.len()
    );
    Ok(())
}

fn audit_release_samples(catalog: &Catalog) -> Result<(), Box<dyn Error>> {
    let march = catalog.character("march_7th.preservation")?;
    let promotion = march.base_attributes(80, 6)?;
    // Final ascension HP is 489.6 with 7.2 growth for each of the first 79 levels.
    assert!((promotion["hp"] - 1058.4).abs() < 0.000_001);
    assert_eq!(promotion["spd"], 101.0);
    let mut acheron = catalog.default_loadout(ACHERON)?;
    acheron.affinitive_eidolon = true;
    assert_eq!(
        catalog.effective_skill_level(ACHERON, &acheron, "skill.130803")?,
        10
    );
    acheron.config.eidolon = 3;
    assert_eq!(
        catalog.effective_skill_level(ACHERON, &acheron, "skill.130803")?,
        12
    );
    assert_eq!(acheron.config.skill_levels["skill.130803"], 10);
    let crit = catalog.light_cone("cone.20000")?;
    for (rank, expected) in (1..=5).zip([0.12, 0.15, 0.18, 0.21, 0.24]) {
        assert_eq!(crit.rank(rank)?.parameters, [expected, 3.0]);
    }
    assert_eq!(catalog.character(AHA)?.resource.capacity, Some(8));
    assert!(
        catalog
            .character("character.1212")?
            .variants
            .contains_key("novaflare")
    );
    assert!(
        catalog.character("character.1407")?.variants["base"]
            .servant
            .is_some()
    );
    Ok(())
}

fn demo_write(catalog: Catalog, db: &Path, old: &Path) -> Result<(), Box<dyn Error>> {
    // The demo uses fresh files to avoid overwriting a user's database or earlier evidence.
    if db.exists() || old.exists() {
        return Err("choose a new database and snapshot path for demo-write".into());
    }
    OpenOptions::new().write(true).create_new(true).open(db)?;
    let mut archive = Archive::open(db, catalog)?;
    archive.ensure_profile(ACHERON)?;
    archive.ensure_profile(AHA)?;
    let mut draft = archive.catalog().default_loadout(ACHERON)?;
    draft.config.light_cone = Some(archive.catalog().default_light_cone(CONE)?);
    archive.save_preset(ACHERON, "叠影一", &draft, true)?;
    for (id, members) in [("alpha", vec![ACHERON, AHA]), ("beta", vec![AHA, ACHERON])] {
        archive.save_team(&Team {
            id: id.into(),
            name: id.into(),
            character_form_ids: members.into_iter().map(String::from).collect(),
        })?;
    }
    let encoded = serde_json::to_string_pretty(&archive.freeze_team("alpha")?)?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(old)?
        .write_all(encoded.as_bytes())?;
    if let Some(cone) = &mut draft.config.light_cone {
        cone.superimposition = 5;
    }
    draft.affinitive_eidolon = true;
    archive.save_preset(ACHERON, "叠影五", &draft, true)?;
    println!("Saved two teams and froze the earlier S1 configuration");
    Ok(())
}

fn demo_reopen(catalog: Catalog, db: &Path, old: &Path) -> Result<(), Box<dyn Error>> {
    if !db.is_file() {
        return Err("existing demo database required".into());
    }
    let archive = Archive::open(db, catalog)?;
    let before = FrozenTeam::from_json(&fs::read_to_string(old)?, archive.catalog())?;
    assert_eq!(archive.profile(ACHERON)?.current_preset, "叠影五");
    assert_snapshot(&before, 1, false)?;
    for id in ["alpha", "beta"] {
        assert_snapshot(&archive.freeze_team(id)?, 5, true)?;
    }
    assert_eq!(archive.team("alpha")?.character_form_ids, [ACHERON, AHA]);
    assert_eq!(archive.team("beta")?.character_form_ids, [AHA, ACHERON]);
    println!(
        "Reopened both teams at E0/S5 with Affinitive Eidolon enabled. Saved E0/S1 snapshot retained."
    );
    Ok(())
}

fn assert_snapshot(snapshot: &FrozenTeam, rank: u8, special: bool) -> Result<(), Box<dyn Error>> {
    let character = snapshot
        .members
        .iter()
        .find(|member| member.character_form_id == ACHERON)
        .ok_or("Acheron missing from demo team")?;
    let cone = character
        .loadout
        .config
        .light_cone
        .as_ref()
        .ok_or("demo light cone missing")?;
    if cone.superimposition != rank
        || character.loadout.affinitive_eidolon != special
        || character.loadout.config.eidolon != 0
    {
        return Err("unexpected demo snapshot configuration".into());
    }
    Ok(())
}
