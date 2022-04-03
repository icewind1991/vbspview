use crate::bsp::map_coords;
use crate::Error;
use std::fs;
use std::path::Path;
use tf_demo_parser::demo::parser::gamestateanalyser::{GameState, GameStateAnalyser};
use tf_demo_parser::{Demo, DemoParser};
use three_d::{vec3, Vec3};
use tracing::{error, info};

pub struct DemoInfo {
    pub map: String,
    pub positions: Vec<(Vec3, f32, f32)>,
    pub start_tick: u32,
    pub time_per_tick: f64,
}

impl DemoInfo {
    pub fn new(demo_path: impl AsRef<Path>, name: &str) -> Result<Self, Error> {
        let file = fs::read(demo_path)?;
        let demo = Demo::new(&file);
        let parser = DemoParser::new_with_analyser(demo.get_stream(), GameStateAnalyser::new());
        let (header, mut ticker) = parser.ticker()?;

        let mut positions = Vec::with_capacity(header.ticks as usize);
        let mut user_id = None;
        let mut start_tick = 0;

        while let Some(tick) = ticker.next()? {
            let state: &GameState = tick.state;
            if user_id.is_none() {
                if let Some(found) = state
                    .players
                    .iter()
                    .enumerate()
                    .filter_map(|(i, player)| Some((i, player.info.as_ref()?)))
                    .find_map(|(i, player)| {
                        player.name.to_ascii_lowercase().contains(name).then(|| i)
                    })
                {
                    info!(user_id = found, "found user");
                    start_tick = tick.tick;
                    user_id = Some(found);
                }
            }
            if let Some(user_id) = user_id {
                let player = &state.players[user_id];
                let coords = map_coords(player.position);
                positions.push((
                    vec3(coords[0], coords[1], coords[2]),
                    player.view_angle,
                    player.pitch_angle,
                ))
            }
        }
        if user_id.is_none() {
            let found = ticker
                .into_state()
                .players
                .into_iter()
                .filter_map(|player| Some(player.info?.name))
                .collect::<Vec<_>>();
            error!(
                "User {} not found in demo, found: {}",
                name,
                found.join(", ")
            );
            return Err("Failed to find user in demo".into());
        }
        Ok(DemoInfo {
            map: header.map,
            positions,
            start_tick,
            time_per_tick: ticker.parser_state().demo_meta.interval_per_tick as f64,
        })
    }
}
