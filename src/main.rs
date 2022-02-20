mod camera;

use camera::FirstPerson;
use itertools::Either;
use std::env::args;
use std::fs;
use thiserror::Error;
use three_d::*;
use vbsp::{Bsp, Handle};

#[derive(Debug, Error)]
enum Error {
    #[error(transparent)]
    Three(#[from] Box<dyn std::error::Error>),
    #[error(transparent)]
    Bsp(#[from] vbsp::BspError),
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error("{0}")]
    Other(&'static str),
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
        max_size: Some((1920, 1080)),
        ..Default::default()
    })?;

    let data = fs::read(&file)?;
    let bsp = Bsp::read(&data)?;
    let world_model = bsp.models().next().ok_or(Error::Other("No world model"))?;

    let context = window.gl().unwrap();

    let mut cpu_mesh = model_to_mesh(world_model);
    cpu_mesh.compute_normals();
    let forward_pipeline = ForwardPipeline::new(&context).unwrap();
    let mut camera = Camera::new_perspective(
        &context,
        window.viewport().unwrap(),
        vec3(2.0, 2.0, 5.0),
        vec3(0.0, 0.0, 0.0),
        vec3(0.0, 1.0, 0.0),
        degrees(90.0),
        0.1,
        30.0,
    )
    .unwrap();
    let mut control = FirstPerson::new(0.05);
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
        directional: vec![
            DirectionalLight::new(&context, 1.0, Color::WHITE, &vec3(0.0, -1.0, 0.0))?,
            DirectionalLight::new(&context, 1.0, Color::WHITE, &vec3(0.0, 1.0, 0.0))?,
        ],
        ..Default::default()
    };

    // main loop
    let mut shadows_enabled = false;
    let mut directional_intensity = lights.directional[0].intensity();
    let mut depth_max = 30.0;
    let mut fov = 60.0;
    let mut debug_type = DebugType::NONE;

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
                    lights.directional[1].set_intensity(directional_intensity);
                    if ui.checkbox(&mut shadows_enabled, "Shadows").clicked() {
                        if !shadows_enabled {
                            lights.directional[0].clear_shadow_map();
                            lights.directional[1].clear_shadow_map();
                        }
                    }

                    ui.label("Debug options");
                    ui.radio_value(&mut debug_type, DebugType::NONE, "None");
                    ui.radio_value(&mut debug_type, DebugType::POSITION, "Position");
                    ui.radio_value(&mut debug_type, DebugType::NORMAL, "Normal");
                    ui.radio_value(&mut debug_type, DebugType::COLOR, "Color");
                    ui.radio_value(&mut debug_type, DebugType::DEPTH, "Depth");
                    ui.radio_value(&mut debug_type, DebugType::ORM, "ORM");

                    ui.label("View options");
                    ui.add(Slider::new(&mut depth_max, 1.0..=30.0).text("Depth max"));
                    ui.add(Slider::new(&mut fov, 45.0..=90.0).text("FOV"));

                    ui.label("Position");
                    ui.add(Label::new(format!("\tx: {}", camera.position().x)));
                    ui.add(Label::new(format!("\ty: {}", camera.position().y)));
                    ui.add(Label::new(format!("\tz: {}", camera.position().z)));
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
            camera
                .set_perspective_projection(degrees(fov), 0.1, 30.0)
                .unwrap();
            if shadows_enabled {
                lights.directional[0]
                    .generate_shadow_map(4.0, 1024, 1024, &[&model])
                    .unwrap();
                lights.directional[1]
                    .generate_shadow_map(4.0, 1024, 1024, &[&model])
                    .unwrap();
            }

            // Light pass
            Screen::write(&context, ClearState::default(), || {
                match debug_type {
                    DebugType::NORMAL => {
                        model.render_with_material(
                            &NormalMaterial::from_physical_material(&model.material),
                            &camera,
                            &lights,
                        )?;
                    }
                    DebugType::DEPTH => {
                        let mut depth_material = DepthMaterial::default();
                        depth_material.max_distance = Some(depth_max);
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
                    DebugType::NONE => forward_pipeline.render_pass(&camera, &[&model], &lights)?,
                };
                gui.render()?;
                Ok(())
            })
            .unwrap();
        }

        let _ = change;

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
        / 50.0;
    let positions: Vec<f32> = model
        .faces()
        .filter(|face| face.is_visible())
        .flat_map(|face| {
            face.displacement()
                .and_then(|displacement| displacement.triangulated_displaced_vertices())
                .map(|verts| Either::Left(verts))
                .unwrap_or_else(|| Either::Right(face.triangulate().flat_map(|verts| verts)))
        })
        .flat_map(|vertex| [-vertex.x, vertex.z, vertex.y])
        .map(|c| c / size)
        .collect();

    CPUMesh {
        positions,
        ..Default::default()
    }
}
