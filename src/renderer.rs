use crate::control::{Control, DebugToggle};
use crate::ui::DebugType;
use crate::DebugUI;
use three_d::*;

pub struct Renderer<C: Control> {
    gui: DebugUI,
    pub models: Vec<Model<PhysicalMaterial>>,
    ambient_lights: Vec<AmbientLight>,
    directional_lights: Vec<DirectionalLight>,
    pub context: Context,
    control: C,
    debug_toggle: DebugToggle,
    pub camera: Camera,
}

impl<C: Control> Renderer<C> {
    pub fn new(window: &Window, control: C) -> Self {
        let context = window.gl();
        let camera = Camera::new_perspective(
            window.viewport(),
            vec3(9.0, 4.0, 5.0),
            vec3(0.0, 0.0, 0.0),
            vec3(0.0, 1.0, 0.0),
            degrees(60.0),
            0.1,
            45.0,
        );

        let ambient_lights = vec![AmbientLight {
            color: Srgba::WHITE,
            intensity: 0.2,
            ..Default::default()
        }];
        let directional_lights = vec![
            DirectionalLight::new(&context, 1.0, Srgba::WHITE, &vec3(0.0, -1.0, 0.0)),
            DirectionalLight::new(&context, 1.0, Srgba::WHITE, &vec3(0.0, 1.0, 0.0)),
        ];
        // let control = FirstPerson::new(0.1);

        Self {
            models: Vec::new(),
            gui: DebugUI::new(&context),
            ambient_lights,
            directional_lights,
            context,
            control,
            debug_toggle: DebugToggle::new(),
            camera,
        }
    }

    pub fn render(&mut self, mut frame_input: FrameInput) -> FrameOutput {
        let (ui_change, _panel_width) =
            self.gui
                .update(&mut frame_input, &self.camera, &mut self.control);
        let change = frame_input.first_frame || ui_change;
        if change {
            if self.gui.shadows_enabled {
                self.directional_lights[0]
                    .generate_shadow_map(1024, self.models.iter().flat_map(|model| model.iter()));
                self.directional_lights[1]
                    .generate_shadow_map(1024, self.models.iter().flat_map(|model| model.iter()));
            } else {
                self.directional_lights[0].clear_shadow_map();
                self.directional_lights[1].clear_shadow_map();
            }
            self.directional_lights[0].intensity = self.gui.directional_intensity;
            self.directional_lights[1].intensity = self.gui.directional_intensity;
            self.ambient_lights[0].intensity = self.gui.ambient_intensity;
            self.camera
                .set_perspective_projection(degrees(self.gui.fov), 0.1, 45.0);
        }

        let viewport = Viewport {
            x: 0,
            y: 0,
            width: frame_input.viewport.width,
            height: frame_input.viewport.height,
        };
        self.camera.set_viewport(viewport);
        self.control.handle(
            &mut self.camera,
            &mut frame_input.events,
            frame_input.elapsed_time,
            frame_input.accumulated_time,
        );
        self.debug_toggle.handle(
            &mut self.camera,
            &mut frame_input.events,
            frame_input.elapsed_time,
            frame_input.accumulated_time,
        );

        let lights = &[
            &self.ambient_lights[0] as &dyn Light,
            &self.directional_lights[0],
            &self.directional_lights[1],
        ];

        // Light pass
        let target = frame_input.screen();
        target.clear(ClearState::default());

        let geometries = self
            .models
            .iter()
            .enumerate()
            .filter_map(|(i, model)| {
                if !self.gui.show_bsp && i == 0 {
                    None
                } else if !self.gui.show_props && i == 1 {
                    None
                } else {
                    Some(model)
                }
            })
            .flat_map(|model| model.iter());

        match self.gui.debug_type {
            DebugType::Normal => target.render_with_material(
                &NormalMaterial::default(),
                &self.camera,
                geometries.map(|gm| &gm.geometry),
                lights,
            ),
            DebugType::Depth => {
                let depth_material = DepthMaterial {
                    max_distance: Some(self.gui.depth_max),
                    ..DepthMaterial::default()
                };
                target.render_with_material(&depth_material, &self.camera, geometries, lights)
            }
            DebugType::Orm => target.render_with_material(
                &ORMMaterial::default(),
                &self.camera,
                geometries.map(|gm| &gm.geometry),
                lights,
            ),
            DebugType::Position => {
                let position_material = PositionMaterial::default();
                target.render_with_material(
                    &position_material,
                    &self.camera,
                    geometries.map(|gm| &gm.geometry),
                    lights,
                )
            }
            DebugType::Uv => {
                let uv_material = UVMaterial::default();
                target.render_with_material(
                    &uv_material,
                    &self.camera,
                    geometries.map(|gm| &gm.geometry),
                    lights,
                )
            }
            DebugType::Color => target.render_with_material(
                &ColorMaterial::default(),
                &self.camera,
                geometries.map(|gm| &gm.geometry),
                lights,
            ),
            DebugType::None => target.render(&self.camera, geometries, lights),
        };

        if self.debug_toggle.enabled {
            target.write(|| self.gui.render());
        }
        FrameOutput::default()
    }
}
