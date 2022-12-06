use crate::bsp::{map_coords, UNIT_SCALE};
use crate::wrapping::Wrapping;
use crate::Error;
use splines::{Interpolation, Key};
use std::fs;
use std::path::Path;
use tf_demo_parser::demo::data::{DemoTick, UserInfo};
use tf_demo_parser::demo::header::Header;
use tf_demo_parser::demo::message::packetentities::EntityId;
use tf_demo_parser::demo::message::Message;
use tf_demo_parser::demo::packet::message::MessagePacketMeta;
use tf_demo_parser::demo::packet::stringtable::StringTableEntry;
use tf_demo_parser::demo::parser::MessageHandler;
use tf_demo_parser::demo::sendprop::SendPropIdentifier;
use tf_demo_parser::demo::vector::{Vector, VectorXY};
use tf_demo_parser::{Demo, DemoParser, MessageType, ParserState, ReadResult, Stream};
use three_d::{vec3, Vec3};

pub struct DemoInfo {
    pub ticks: u32,
    pub map: String,
    pub positions: Positions,
    pub start_tick: DemoTick,
    pub time_per_tick: f64,
}

impl DemoInfo {
    pub fn new(demo_path: impl AsRef<Path>, name: &str) -> Result<Self, Error> {
        let file = fs::read(demo_path)?;
        let demo = Demo::new(&file);
        let parser =
            DemoParser::new_with_analyser(demo.get_stream(), PovAnalyzer::new(name.into()));
        let (header, (positions, start_tick, interval_per_tick)) = parser.parse()?;

        Ok(DemoInfo {
            ticks: header.ticks,
            map: header.map,
            positions,
            start_tick,
            time_per_tick: interval_per_tick as f64,
        })
    }
}

#[derive(Default)]
pub struct Positions {
    pub positions: Vec<Key<f32, Vec3>>,
    pub pitch: Vec<Key<f32, Wrapping<-180, 180>>>,
    pub yaw: Vec<Key<f32, Wrapping<-180, 180>>>,
}

struct PovAnalyzer {
    last_position: Vector,
    view_offset: f32,
    positions: Positions,
    name: String,
    player: Option<EntityId>,
    start_tick: DemoTick,
    pov_name: String,
    is_pov: bool,
    last_tick: DemoTick,
    last_pov_tick: DemoTick,
}

impl MessageHandler for PovAnalyzer {
    type Output = (Positions, DemoTick, f32);

    fn does_handle(message_type: MessageType) -> bool {
        matches!(message_type, MessageType::PacketEntities)
    }

    fn handle_header(&mut self, header: &Header) {
        self.pov_name = header.nick.clone();
        if self.name.is_empty() {
            self.name = self.pov_name.to_ascii_lowercase();
        }
    }

    fn handle_message(&mut self, message: &Message, tick: DemoTick, _state: &ParserState) {
        if tick > self.last_tick {
            self.last_tick = tick;
            const NON_LOCAL_ORIGIN: SendPropIdentifier =
                SendPropIdentifier::new("DT_TFNonLocalPlayerExclusive", "m_vecOrigin");
            const NON_LOCAL_ORIGIN_Z: SendPropIdentifier =
                SendPropIdentifier::new("DT_TFNonLocalPlayerExclusive", "m_vecOrigin[2]");
            const NON_LOCAL_PITCH_ANGLES: SendPropIdentifier =
                SendPropIdentifier::new("DT_TFNonLocalPlayerExclusive", "m_angEyeAngles[0]");
            const NON_LOCAL_YAW_ANGLES: SendPropIdentifier =
                SendPropIdentifier::new("DT_TFNonLocalPlayerExclusive", "m_angEyeAngles[1]");
            const VIEW_OFFSET: SendPropIdentifier =
                SendPropIdentifier::new("DT_LocalPlayerExclusive", "m_vecViewOffset[2]");

            let old_pos = self.last_position;
            let old_offset = self.view_offset;

            if let (Message::PacketEntities(message), Some(player_id)) = (message, self.player) {
                if self.start_tick == 0 {
                    self.start_tick = tick;
                }
                for entity in &message.entities {
                    if entity.entity_index == player_id {
                        for prop in &entity.props {
                            match prop.identifier {
                                NON_LOCAL_ORIGIN => {
                                    let pos_xy =
                                        VectorXY::try_from(&prop.value).unwrap_or_default();
                                    self.last_position.x = pos_xy.x;
                                    self.last_position.y = pos_xy.y;
                                }
                                NON_LOCAL_ORIGIN_Z => {
                                    self.last_position.z =
                                        f32::try_from(&prop.value).unwrap_or_default()
                                }
                                NON_LOCAL_PITCH_ANGLES => {
                                    self.positions.pitch.push(Key::new(
                                        u32::from(tick) as f32,
                                        Wrapping(f32::try_from(&prop.value).unwrap_or_default()),
                                        Interpolation::Linear,
                                    ));
                                }
                                NON_LOCAL_YAW_ANGLES => {
                                    self.positions.yaw.push(Key::new(
                                        u32::from(tick) as f32,
                                        Wrapping(f32::try_from(&prop.value).unwrap_or_default()),
                                        Interpolation::Linear,
                                    ));
                                }
                                VIEW_OFFSET => {
                                    self.view_offset =
                                        f32::try_from(&prop.value).unwrap_or_default() * UNIT_SCALE;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }

            if (self.last_position != old_pos || old_offset != self.view_offset) && !self.is_pov {
                let pos = map_coords(<[f32; 3]>::from(self.last_position));
                self.positions.positions.push(Key::new(
                    u32::from(tick) as f32,
                    vec3(pos[0], pos[1] + self.view_offset, pos[2]),
                    Interpolation::CatmullRom,
                ));
            }
        }
    }

    fn handle_string_entry(
        &mut self,
        table: &str,
        index: usize,
        entry: &StringTableEntry,
        _state: &ParserState,
    ) {
        if table == "userinfo" && self.player.is_none() {
            let _ = self.parse_user_info(
                index as u16,
                entry.text.as_ref().map(|s| s.as_ref()),
                entry.extra_data.as_ref().map(|data| data.data.clone()),
            );
        }
    }

    fn handle_packet_meta(
        &mut self,
        tick: DemoTick,
        meta: &MessagePacketMeta,
        _state: &ParserState,
    ) {
        if tick != self.last_pov_tick {
            self.last_pov_tick = tick;
            if self.is_pov {
                self.positions.pitch.push(Key::new(
                    u32::from(tick) as f32,
                    Wrapping(meta.view_angles[0].local_angles.y),
                    Interpolation::Linear,
                ));
                self.positions.yaw.push(Key::new(
                    u32::from(tick) as f32,
                    Wrapping(meta.view_angles[0].local_angles.x),
                    Interpolation::Linear,
                ));
                let pos = map_coords(<[f32; 3]>::from(meta.view_angles[0].origin));
                self.positions.positions.push(Key::new(
                    u32::from(tick) as f32,
                    vec3(pos[0], pos[1] + self.view_offset, pos[2]),
                    Interpolation::CatmullRom,
                ));
            }
        }
    }

    fn into_output(self, state: &ParserState) -> Self::Output {
        (
            self.positions,
            self.start_tick,
            state.demo_meta.interval_per_tick,
        )
    }
}

impl PovAnalyzer {
    pub fn new(name: String) -> Self {
        PovAnalyzer {
            last_position: Vector::default(),
            view_offset: 0.0,
            positions: Positions::default(),
            name,
            player: None,
            start_tick: DemoTick::default(),
            pov_name: String::new(),
            is_pov: false,
            last_tick: DemoTick::default(),
            last_pov_tick: DemoTick::default(),
        }
    }

    fn parse_user_info(
        &mut self,
        index: u16,
        text: Option<&str>,
        data: Option<Stream>,
    ) -> ReadResult<()> {
        if let Some(user_info) = UserInfo::parse_from_string_table(index, text, data)? {
            if user_info
                .player_info
                .name
                .to_ascii_lowercase()
                .contains(&self.name)
            {
                self.is_pov = user_info.player_info.name == self.pov_name;
                self.player = Some(user_info.entity_id);
            }
        }

        Ok(())
    }
}
