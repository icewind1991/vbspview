use crate::Control;
use three_d::egui::*;
use three_d::{Camera, Context, FrameInput, GUI};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum DebugType {
    POSITION,
    NORMAL,
    COLOR,
    DEPTH,
    ORM,
    NONE,
}

pub struct DebugUI {
    ui: GUI,
    pub shadows_enabled: bool,
    pub directional_intensity: f32,
    pub ambient_intensity: f32,
    pub depth_max: f32,
    pub fov: f32,
    pub debug_type: DebugType,
}

impl DebugUI {
    pub fn new(context: &Context) -> Self {
        DebugUI {
            ui: three_d::GUI::new(context),
            shadows_enabled: false,
            directional_intensity: 1.0,
            ambient_intensity: 0.2,
            depth_max: 30.0,
            fov: 60.0,
            debug_type: DebugType::NORMAL,
        }
    }

    pub fn update<C: Control>(
        &mut self,
        frame_input: &mut FrameInput,
        camera: &Camera,
        control: &mut C,
    ) -> (bool, u32) {
        let mut panel_width = 0;
        let change = self.ui.update(
            &mut frame_input.events,
            frame_input.accumulated_time,
            frame_input.viewport,
            frame_input.device_pixel_ratio,
            |gui_context| {
                SidePanel::left("side_panel").show(gui_context, |ui| {
                    ui.heading("Debug Panel");
                    ui.label("  toggle panel with <`>");

                    ui.label("Light options");
                    ui.add(
                        Slider::new(&mut self.ambient_intensity, 0.0..=1.0)
                            .text("Ambient intensity"),
                    );
                    ui.add(
                        Slider::new(&mut self.directional_intensity, 0.0..=1.0)
                            .text("Directional intensity"),
                    );
                    ui.checkbox(&mut self.shadows_enabled, "Shadows");

                    ui.label("Debug options");
                    ui.radio_value(&mut self.debug_type, DebugType::NONE, "None");
                    ui.radio_value(&mut self.debug_type, DebugType::POSITION, "Position");
                    ui.radio_value(&mut self.debug_type, DebugType::NORMAL, "Normal");
                    ui.radio_value(&mut self.debug_type, DebugType::COLOR, "Color");
                    ui.radio_value(&mut self.debug_type, DebugType::DEPTH, "Depth");
                    ui.radio_value(&mut self.debug_type, DebugType::ORM, "ORM");

                    ui.label("View options");
                    ui.add(Slider::new(&mut self.depth_max, 1.0..=30.0).text("Depth max"));
                    ui.add(Slider::new(&mut self.fov, 45.0..=90.0).text("FOV"));

                    ui.label("Position");
                    ui.add(Label::new(format!("\tx: {}", camera.position().x)));
                    ui.add(Label::new(format!("\ty: {}", camera.position().y)));
                    ui.add(Label::new(format!("\tz: {}", camera.position().z)));

                    control.ui(ui);
                });
                panel_width = gui_context.used_size().x as u32;
            },
        );
        control.post_ui(frame_input.accumulated_time);
        (change, panel_width)
    }

    pub fn render(&mut self) -> () {
        self.ui.render()
    }
}
