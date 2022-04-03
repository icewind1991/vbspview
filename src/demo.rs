use crate::bsp::{map_coords, UNIT_SCALE};
use crate::Error;
use std::fs;
use std::path::Path;
use tf_demo_parser::demo::data::UserInfo;
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
    pub map: String,
    pub positions: Vec<(Vec3, [f32; 2])>,
    pub start_tick: u32,
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
            map: header.map,
            positions,
            start_tick,
            time_per_tick: interval_per_tick as f64,
        })
    }
}

struct PovAnalyzer {
    last_position: Vector,
    last_angles: [f32; 2],
    view_offset: f32,
    positions: Vec<(Vec3, [f32; 2])>,
    name: String,
    player: Option<EntityId>,
    start_tick: u32,
    last_tick: u32,
    pov_name: String,
    is_pov: bool,
}

impl MessageHandler for PovAnalyzer {
    type Output = (Vec<(Vec3, [f32; 2])>, u32, f32);

    fn does_handle(message_type: MessageType) -> bool {
        matches!(message_type, MessageType::PacketEntities)
    }

    fn handle_header(&mut self, header: &Header) {
        self.pov_name = header.nick.clone();
        if self.name.is_empty() {
            self.name = self.pov_name.clone();
        }
    }

    fn handle_packet_meta(&mut self, meta: &MessagePacketMeta) {
        if self.is_pov {
            self.last_angles = [
                meta.view_angles.local_angles.1.x,
                meta.view_angles.local_angles.1.y,
            ];
            self.last_position = meta.view_angles.origin.1
        }
    }

    fn handle_message(&mut self, message: &Message, tick: u32) {
        const LOCAL_ORIGIN: SendPropIdentifier =
            SendPropIdentifier::new("DT_TFLocalPlayerExclusive", "m_vecOrigin");
        const NON_LOCAL_ORIGIN: SendPropIdentifier =
            SendPropIdentifier::new("DT_TFNonLocalPlayerExclusive", "m_vecOrigin");
        const LOCAL_ORIGIN_Z: SendPropIdentifier =
            SendPropIdentifier::new("DT_TFLocalPlayerExclusive", "m_vecOrigin[2]");
        const NON_LOCAL_ORIGIN_Z: SendPropIdentifier =
            SendPropIdentifier::new("DT_TFNonLocalPlayerExclusive", "m_vecOrigin[2]");
        const LOCAL_YAW_ANGLES: SendPropIdentifier =
            SendPropIdentifier::new("DT_TFLocalPlayerExclusive", "m_angEyeAngles[1]");
        const NON_LOCAL_YAW_ANGLES: SendPropIdentifier =
            SendPropIdentifier::new("DT_TFNonLocalPlayerExclusive", "m_angEyeAngles[1]");
        const LOCAL_PITCH_ANGLES: SendPropIdentifier =
            SendPropIdentifier::new("DT_TFLocalPlayerExclusive", "m_angEyeAngles[0]");
        const NON_LOCAL_PITCH_ANGLES: SendPropIdentifier =
            SendPropIdentifier::new("DT_TFNonLocalPlayerExclusive", "m_angEyeAngles[0]");
        const VIEW_OFFSET: SendPropIdentifier =
            SendPropIdentifier::new("DT_LocalPlayerExclusive", "m_vecViewOffset[2]");

        if let (Message::PacketEntities(message), Some(player_id)) = (message, self.player) {
            if self.start_tick == 0 {
                self.start_tick = tick;
            }
            for entity in &message.entities {
                if entity.entity_index == player_id {
                    for prop in &entity.props {
                        match prop.identifier {
                            LOCAL_ORIGIN | NON_LOCAL_ORIGIN => {
                                let pos_xy = VectorXY::try_from(&prop.value).unwrap_or_default();
                                self.last_position.x = pos_xy.x;
                                self.last_position.y = pos_xy.y;
                            }
                            LOCAL_ORIGIN_Z | NON_LOCAL_ORIGIN_Z => {
                                self.last_position.z =
                                    f32::try_from(&prop.value).unwrap_or_default()
                            }
                            LOCAL_YAW_ANGLES | NON_LOCAL_YAW_ANGLES => {
                                self.last_angles[1] = f32::try_from(&prop.value).unwrap_or_default()
                            }
                            LOCAL_PITCH_ANGLES | NON_LOCAL_PITCH_ANGLES => {
                                self.last_angles[0] = f32::try_from(&prop.value).unwrap_or_default()
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

        if tick > self.last_tick {
            self.last_tick = tick;
            let pos = map_coords(self.last_position);
            self.positions.push((
                vec3(pos[0], pos[1] + self.view_offset, pos[2]),
                self.last_angles,
            ));
        }
    }

    fn handle_string_entry(&mut self, table: &str, _index: usize, entry: &StringTableEntry) {
        if table == "userinfo" && self.player.is_none() {
            let _ = self.parse_user_info(
                entry.text.as_ref().map(|s| s.as_ref()),
                entry.extra_data.as_ref().map(|data| data.data.clone()),
            );
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
            last_angles: [0.0, 0.0],
            view_offset: 0.0,
            positions: vec![],
            name,
            player: None,
            start_tick: 0,
            last_tick: 0,
            pov_name: String::new(),
            is_pov: false,
        }
    }

    fn parse_user_info(&mut self, text: Option<&str>, data: Option<Stream>) -> ReadResult<()> {
        if let Some(user_info) = UserInfo::parse_from_string_table(text, data)? {
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
