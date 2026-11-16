use std::{env, error::Error, fs, hint::black_box, io, time::Instant};

use serde_json::json;
use star_rail_battle_core::{Battle, LogMode, Outcome, PreparedBattle, Session};

const SHARED: &str = include_str!("../../../contracts/battle-session.v1.example.json");
const FULL_COMMANDS: &str = include_str!("../fixtures/full-replay.commands.json");

fn fixture() -> Result<Session, Box<dyn Error>> {
    let mut session = Session::from_json(SHARED)?;
    session.commands = serde_json::from_str(FULL_COMMANDS)?;
    Ok(session)
}

fn replay(path: Option<&String>) -> Result<(), Box<dyn Error>> {
    let session = match path {
        Some(path) => Session::from_json(&fs::read_to_string(path)?)?,
        None => fixture()?,
    };
    let setup = PreparedBattle::new(&session.input)?;
    let battle = Battle::replay(&setup, &session.commands, LogMode::Detailed)?;
    let report = json!({ "snapshot": battle.snapshot(), "events": battle.events() });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn benchmark(mode: &str, iterations: u32, wait_for_peak: bool) -> Result<(), Box<dyn Error>> {
    if cfg!(debug_assertions) {
        return Err("build the benchmark in Release mode".into());
    }
    if iterations == 0 {
        return Err("iterations must be positive".into());
    }
    let log = match mode {
        "off" => LogMode::Off,
        "detailed" => LogMode::Detailed,
        _ => return Err("log mode must be off or detailed".into()),
    };
    let session = fixture()?;
    let setup = PreparedBattle::new(&session.input)?;
    let simulate = || -> Result<(), Box<dyn Error>> {
        let battle = Battle::replay(
            black_box(&setup),
            black_box(&session.commands),
            black_box(log),
        )?;
        if black_box(&battle).outcome() != Outcome::Victory {
            return Err("the benchmark fixture did not complete with a victory".into());
        }
        Ok(())
    };
    for _ in 0..1_000 {
        simulate()?;
    }
    let start = Instant::now();
    for _ in 0..iterations {
        simulate()?;
    }
    let elapsed = start.elapsed();
    let seconds = elapsed.as_secs_f64();
    println!(
        "{}",
        json!({
            "log_mode": mode, "iterations": iterations, "warmup": 1_000, "workers": 1,
            "elapsed_ns": elapsed.as_nanos() as u64,
            "battles_per_second": f64::from(iterations) / seconds,
            "average_us": seconds * 1_000_000.0 / f64::from(iterations),
        })
    );
    if wait_for_peak {
        // The Windows caller reads the process peak after the timed loop and then releases us.
        io::stdin().read_line(&mut String::new())?;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    match args.as_slice() {
        [] => replay(None),
        [verb] if verb == "replay" => replay(None),
        [verb, path] if verb == "replay" => replay(Some(path)),
        [verb, mode, count] if verb == "benchmark" => benchmark(mode, count.parse()?, false),
        [verb, mode, count, flag] if verb == "benchmark" && flag == "--wait-for-peak" => {
            benchmark(mode, count.parse()?, true)
        }
        _ => Err(
            "usage: m02 replay [session.json] | m02 benchmark <off|detailed> <iterations>".into(),
        ),
    }
}
