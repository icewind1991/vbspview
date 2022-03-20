use crate::{DebugUI, FirstPerson};
use three_d::*;

pub struct Renderer {
    gui: DebugUI,
    pub models: Vec<Model<PhysicalMaterial>>,
    lights: Lights,
    pub context: Context,
    pipeline: ForwardPipeline,
    control: FirstPerson,
    camera: Camera,
}

impl Renderer {
    pub fn new(window: &Window) -> ThreeDResult<Self> {
        let context = window.gl().unwrap();
        let forward_pipeline = ForwardPipeline::new(&context).unwrap();
        let camera = Camera::new_perspective(
            &context,
            window.viewport().unwrap(),
            vec3(9.0, 4.0, 5.0),
            vec3(0.0, 0.0, 0.0),
            vec3(0.0, 1.0, 0.0),
            degrees(90.0),
            0.1,
            30.0,
        )?;

        let lights = Lights {
            ambient: Some(AmbientLight {
                color: Color::WHITE,
                intensity: 0.2,
                ..Default::default()
            }),
            directional: vec![
                DirectionalLight::new(&context, 1.0, Color::WHITE, &vec3(0.0, -1.0, 0.0))?,
                DirectionalLight::new(&context, 1.0, Color::WHITE, &vec3(0.0, 1.0, 0.0))?,
            ],
            ..Default::default()
        };
        let control = FirstPerson::new(0.1);

        Ok(Self {
            models: Vec::new(),
            gui: DebugUI::new(&context)?,
            pipeline: forward_pipeline,
            lights,
            context,
            control,
            camera,
        })
    }

    pub fn render(&mut self, mut frame_input: FrameInput) -> ThreeDResult<FrameOutput> {
        let (ui_change, _panel_width) = self.gui.update(&mut frame_input, &self.camera)?;
        let change = frame_input.first_frame || ui_change;
        if change {
            if self.gui.shadows_enabled {
                self.lights.directional[0]
                    .generate_shadow_map(4.0, 1024, 1024, &self.models)
                    .unwrap();
                self.lights.directional[1]
                    .generate_shadow_map(4.0, 1024, 1024, &self.models)
                    .unwrap();
            } else {
                self.lights.directional[0].clear_shadow_map();
                self.lights.directional[1].clear_shadow_map();
            }
            self.lights.directional[0].set_intensity(self.gui.directional_intensity);
            self.lights.directional[1].set_intensity(self.gui.directional_intensity);
            self.lights.ambient.as_mut().unwrap().intensity = self.gui.ambient_intensity;
            self.camera
                .set_perspective_projection(
                    degrees(self.gui.fov),
                    self.camera.z_near(),
                    self.camera.z_far(),
                )
                .unwrap();
        }

        let viewport = Viewport {
            x: 0,
            y: 0,
            width: frame_input.viewport.width,
            height: frame_input.viewport.height,
        };
        self.camera.set_viewport(viewport).unwrap();
        self.control
            .handle_events(&mut self.camera, &mut frame_input.events)
            .unwrap();

        // Light pass
        Screen::write(&self.context, ClearState::default(), || {
            match self.gui.debug_type {
                DebugType::NORMAL => {
                    for model in &self.models {
                        model.render_with_material(
                            &NormalMaterial::from_physical_material(&model.material),
                            &self.camera,
                            &self.lights,
                        )?;
                    }
                }
                DebugType::DEPTH => {
                    let depth_material = DepthMaterial {
                        max_distance: Some(self.gui.depth_max),
                        ..DepthMaterial::default()
                    };
                    for model in &self.models {
                        model.render_with_material(&depth_material, &self.camera, &self.lights)?;
                    }
                }
                DebugType::ORM => {
                    for model in &self.models {
                        model.render_with_material(
                            &ORMMaterial::from_physical_material(&model.material),
                            &self.camera,
                            &self.lights,
                        )?;
                    }
                }
                DebugType::POSITION => {
                    for model in &self.models {
                        let position_material = PositionMaterial::default();
                        model.render_with_material(
                            &position_material,
                            &self.camera,
                            &self.lights,
                        )?;
                    }
                }
                DebugType::UV => {
                    for model in &self.models {
                        let uv_material = UVMaterial::default();
                        model.render_with_material(&uv_material, &self.camera, &self.lights)?;
                    }
                }
                DebugType::COLOR => {
                    for model in &self.models {
                        model.render_with_material(
                            &ColorMaterial::from_physical_material(&model.material),
                            &self.camera,
                            &self.lights,
                        )?;
                    }
                }
                DebugType::NONE => {
                    self.pipeline
                        .render_pass(&self.camera, &self.models, &self.lights)?
                }
            };
            if self.control.debug {
                self.gui.render()?;
            }
            Ok(())
        })?;
        Ok(FrameOutput::default())
    }
}
