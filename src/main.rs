use miette::Diagnostic;
use std::env::args;
use std::fs;
use thiserror::Error;
use three_d::*;
use vbsp::{Bsp, Handle};

#[derive(Debug, Error, Diagnostic)]
enum Error {
    #[error(transparent)]
    Three(#[from] Box<dyn std::error::Error>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Bsp(#[from] vbsp::BspError),
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error("{0}")]
    Other(&'static str),
}

#[derive(Debug, Eq, PartialEq)]
enum Pipeline {
    Forward,
    Deferred,
}

fn main() -> Result<(), Error> {
    miette::set_panic_hook();

    let mut args = args();
    let bin = args.next().unwrap();
    let file = match args.next() {
        Some(file) => file,
        None => {
            eprintln!("usage: {} <file.bsp>", bin);
            return Ok(());
        }
    };

    let window = Window::new(WindowSettings {
        title: file.clone(),
        max_size: Some((1280, 720)),
        ..Default::default()
    })?;

    let data = fs::read(&file)?;
    let bsp = Bsp::read(&data)?;
    let world_model = bsp.models().next().ok_or(Error::Other("No world model"))?;

    let context = window.gl().unwrap();

    let mut cpu_mesh = model_to_mesh(world_model);
    cpu_mesh.compute_normals();
    let forward_pipeline = ForwardPipeline::new(&context).unwrap();
    let mut deferred_pipeline = DeferredPipeline::new(&context).unwrap();
    let mut camera = Camera::new_perspective(
        &context,
        window.viewport().unwrap(),
        vec3(2.0, 2.0, 5.0),
        vec3(0.0, 0.0, 0.0),
        vec3(0.0, 1.0, 0.0),
        degrees(45.0),
        0.1,
        30.0,
    )
    .unwrap();
    let mut control = OrbitControl::new(*camera.target(), 1.0, 100.0);
    let mut gui = three_d::GUI::new(&context).unwrap();

    let material = PhysicalMaterial {
        albedo: Color {
            r: 128,
            g: 128,
            b: 128,
            a: 255,
        },
        ..Default::default()
    };

    let model = Model::new_with_material(&context, &cpu_mesh, material)?;

    let mut lights = Lights {
        ambient: Some(AmbientLight {
            color: Color::WHITE,
            intensity: 0.2,
            ..Default::default()
        }),
        directional: vec![DirectionalLight::new(
            &context,
            1.0,
            Color::WHITE,
            &vec3(0.0, -1.0, 0.0),
        )
        .unwrap()],
        ..Default::default()
    };

    // main loop
    let mut shadows_enabled = true;
    let mut directional_intensity = lights.directional[0].intensity();

    let mut current_pipeline = Pipeline::Forward;

    window.render_loop(move |mut frame_input| {
        let mut change = frame_input.first_frame;
        let mut panel_width = frame_input.viewport.width;
        change |= gui
            .update(&mut frame_input, |gui_context| {
                use three_d::egui::*;
                SidePanel::left("side_panel").show(gui_context, |ui| {
                    ui.heading("Debug Panel");

                    ui.label("Light options");
                    ui.add(
                        Slider::new(&mut lights.ambient.as_mut().unwrap().intensity, 0.0..=1.0)
                            .text("Ambient intensity"),
                    );
                    ui.add(
                        Slider::new(&mut directional_intensity, 0.0..=1.0)
                            .text("Directional intensity"),
                    );
                    lights.directional[0].set_intensity(directional_intensity);
                    if ui.checkbox(&mut shadows_enabled, "Shadows").clicked() {
                        if !shadows_enabled {
                            lights.directional[0].clear_shadow_map();
                        }
                    }

                    ui.label("Lighting model");
                    ui.radio_value(&mut lights.lighting_model, LightingModel::Phong, "Phong");
                    ui.radio_value(&mut lights.lighting_model, LightingModel::Blinn, "Blinn");
                    ui.radio_value(
                        &mut lights.lighting_model,
                        LightingModel::Cook(
                            NormalDistributionFunction::Blinn,
                            GeometryFunction::SmithSchlickGGX,
                        ),
                        "Cook (Blinn)",
                    );
                    ui.radio_value(
                        &mut lights.lighting_model,
                        LightingModel::Cook(
                            NormalDistributionFunction::Beckmann,
                            GeometryFunction::SmithSchlickGGX,
                        ),
                        "Cook (Beckmann)",
                    );
                    ui.radio_value(
                        &mut lights.lighting_model,
                        LightingModel::Cook(
                            NormalDistributionFunction::TrowbridgeReitzGGX,
                            GeometryFunction::SmithSchlickGGX,
                        ),
                        "Cook (Trowbridge-Reitz GGX)",
                    );

                    ui.label("Pipeline");
                    ui.radio_value(&mut current_pipeline, Pipeline::Forward, "Forward");
                    ui.radio_value(&mut current_pipeline, Pipeline::Deferred, "Deferred");
                    ui.label("Debug options");
                    ui.radio_value(&mut deferred_pipeline.debug_type, DebugType::NONE, "None");
                    ui.radio_value(
                        &mut deferred_pipeline.debug_type,
                        DebugType::POSITION,
                        "Position",
                    );
                    ui.radio_value(
                        &mut deferred_pipeline.debug_type,
                        DebugType::NORMAL,
                        "Normal",
                    );
                    ui.radio_value(&mut deferred_pipeline.debug_type, DebugType::COLOR, "Color");
                    ui.radio_value(&mut deferred_pipeline.debug_type, DebugType::UV, "UV");
                    ui.radio_value(&mut deferred_pipeline.debug_type, DebugType::DEPTH, "Depth");
                    ui.radio_value(&mut deferred_pipeline.debug_type, DebugType::ORM, "ORM");
                });
                panel_width = gui_context.used_size().x as u32;
            })
            .unwrap();

        let viewport = Viewport {
            x: panel_width as i32,
            y: 0,
            width: frame_input.viewport.width - panel_width,
            height: frame_input.viewport.height,
        };
        change |= camera.set_viewport(viewport).unwrap();
        change |= control
            .handle_events(&mut camera, &mut frame_input.events)
            .unwrap();

        // Draw
        {
            if shadows_enabled {
                lights.directional[0]
                    .generate_shadow_map(4.0, 1024, 1024, &[&model])
                    .unwrap();
            }

            // Geometry pass
            if change && current_pipeline == Pipeline::Deferred {
                deferred_pipeline
                    .render_pass(
                        &camera,
                        &[(
                            &model,
                            DeferredPhysicalMaterial::from_physical_material(&model.material),
                        )],
                    )
                    .unwrap();
            }

            // Light pass
            Screen::write(&context, ClearState::default(), || {
                match current_pipeline {
                    Pipeline::Forward => {
                        match deferred_pipeline.debug_type {
                            DebugType::NORMAL => {
                                model.render_with_material(
                                    &NormalMaterial::from_physical_material(&model.material),
                                    &camera,
                                    &lights,
                                )?;
                            }
                            DebugType::DEPTH => {
                                let depth_material = DepthMaterial::default();
                                model.render_with_material(&depth_material, &camera, &lights)?;
                            }
                            DebugType::ORM => {
                                model.render_with_material(
                                    &ORMMaterial::from_physical_material(&model.material),
                                    &camera,
                                    &lights,
                                )?;
                            }
                            DebugType::POSITION => {
                                let position_material = PositionMaterial::default();
                                model.render_with_material(&position_material, &camera, &lights)?;
                            }
                            DebugType::UV => {
                                let uv_material = UVMaterial::default();
                                model.render_with_material(&uv_material, &camera, &lights)?;
                            }
                            DebugType::COLOR => {
                                model.render_with_material(
                                    &ColorMaterial::from_physical_material(&model.material),
                                    &camera,
                                    &lights,
                                )?;
                            }
                            DebugType::NONE => {
                                forward_pipeline.render_pass(&camera, &[&model], &lights)?
                            }
                        };
                    }
                    Pipeline::Deferred => {
                        deferred_pipeline.lighting_pass(&camera, &lights)?;
                    }
                }
                gui.render()?;
                Ok(())
            })
            .unwrap();
        }

        FrameOutput::default()
    })?;

    Ok(())
}

fn model_to_mesh(model: Handle<vbsp::data::Model>) -> CPUMesh {
    let size = [
        model.maxs.z - model.mins.z,
        model.maxs.y - model.mins.y,
        model.maxs.x - model.mins.x,
    ]
    .into_iter()
    .max_by(|a, b| a.partial_cmp(b).unwrap())
    .unwrap()
        / 10.0;
    let positions: Vec<f32> = model
        .faces()
        .filter(|face| face.is_visible())
        .flat_map(|face| face.triangulate())
        .flat_map(|triangle| triangle.into_iter())
        .flat_map(|vertex| [vertex.x, vertex.z, vertex.y])
        .map(|c| c / size)
        .collect();

    CPUMesh {
        positions,
        ..Default::default()
    }
}
