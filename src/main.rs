mod camera;
mod loader;

use camera::FirstPerson;
use itertools::Either;
use loader::Loader;
use std::env::args;
use std::path::Path;
use std::time::Instant;
use thiserror::Error;
use three_d::*;
use tracing_subscriber::{prelude::*, EnvFilter};
use tracing_tree::HierarchicalLayer;
use vbsp::{Bsp, Handle, StaticPropLump};
use vmdl::mdl::Mdl;
use vmdl::vtx::Vtx;
use vmdl::vvd::Vvd;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Three(#[from] Box<dyn std::error::Error>),
    #[error(transparent)]
    Bsp(#[from] vbsp::BspError),
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    Vpk(#[from] vpk::Error),
    #[error(transparent)]
    Mdl(#[from] vmdl::ModelError),
    #[error("{0}")]
    Other(&'static str),
}

impl From<&'static str> for Error {
    fn from(e: &'static str) -> Self {
        Error::Other(e)
    }
}

fn setup() {
    miette::set_panic_hook();

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(
            HierarchicalLayer::new(2)
                .with_targets(true)
                .with_bracketed_fields(true),
        )
        .init();
}

fn main() -> Result<(), Error> {
    setup();

    let mut args = args();
    let _bin = args.next().unwrap();
    let file = match args.next() {
        Some(file) => file,
        None => {
            "koth_bagel_rc2a.bsp".into()
            // eprintln!("usage: {} <file.bsp>", bin);
            // return Ok(());
        }
    };

    let loader = Loader::new()?;

    let window = Window::new(WindowSettings {
        title: file.clone(),
        max_size: Some((1920, 1080)),
        ..Default::default()
    })?;

    let context = window.gl().unwrap();

    let (cpu_mesh, bsp) = load_world(file.as_ref())?;
    let forward_pipeline = ForwardPipeline::new(&context).unwrap();
    let mut camera = Camera::new_perspective(
        &context,
        window.viewport().unwrap(),
        vec3(9.0, 4.0, 5.0),
        vec3(0.0, 0.0, 0.0),
        vec3(0.0, 1.0, 0.0),
        degrees(90.0),
        0.1,
        30.0,
    )
    .unwrap();
    let mut control = FirstPerson::new(0.1);
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

    let model = Model::new_with_material(&context, &cpu_mesh, material.clone())?;
    let props = bsp
        .static_props()
        .map(|prop| load_prop(&loader, prop))
        .collect::<Result<Vec<_>, _>>()?;
    let merged_props = merge_meshes(props);
    let props_model = Model::new_with_material(&context, &merged_props, material)?;

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
    let mut debug_type = DebugType::NORMAL;

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
                .set_perspective_projection(degrees(fov), camera.z_near(), camera.z_far())
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
                        props_model.render_with_material(
                            &NormalMaterial::from_physical_material(&model.material),
                            &camera,
                            &lights,
                        )?;
                    }
                    DebugType::DEPTH => {
                        let mut depth_material = DepthMaterial::default();
                        depth_material.max_distance = Some(depth_max);
                        model.render_with_material(&depth_material, &camera, &lights)?;
                        props_model.render_with_material(&depth_material, &camera, &lights)?;
                    }
                    DebugType::ORM => {
                        model.render_with_material(
                            &ORMMaterial::from_physical_material(&model.material),
                            &camera,
                            &lights,
                        )?;
                        props_model.render_with_material(
                            &ORMMaterial::from_physical_material(&model.material),
                            &camera,
                            &lights,
                        )?;
                    }
                    DebugType::POSITION => {
                        let position_material = PositionMaterial::default();
                        model.render_with_material(&position_material, &camera, &lights)?;
                        props_model.render_with_material(&position_material, &camera, &lights)?;
                    }
                    DebugType::UV => {
                        let uv_material = UVMaterial::default();
                        model.render_with_material(&uv_material, &camera, &lights)?;
                        props_model.render_with_material(&uv_material, &camera, &lights)?;
                    }
                    DebugType::COLOR => {
                        model.render_with_material(
                            &ColorMaterial::from_physical_material(&model.material),
                            &camera,
                            &lights,
                        )?;
                        props_model.render_with_material(
                            &ColorMaterial::from_physical_material(&model.material),
                            &camera,
                            &lights,
                        )?;
                    }
                    DebugType::NONE => {
                        forward_pipeline.render_pass(&camera, &[&model, &props_model], &lights)?
                    }
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

fn map_coords<C: Into<[f32; 3]>>(vec: C) -> [f32; 3] {
    let vec = vec.into();
    [
        vec[1] * UNIT_SCALE,
        vec[2] * UNIT_SCALE,
        vec[0] * UNIT_SCALE,
    ]
}

// 1 hammer unit is ~1.905cm
const UNIT_SCALE: f32 = 1.0 / (1.905 * 100.0);

fn model_to_mesh(model: Handle<vbsp::data::Model>) -> CPUMesh {
    let positions: Vec<f32> = model
        .faces()
        .filter(|face| face.is_visible())
        .flat_map(|face| {
            face.displacement()
                .map(|displacement| displacement.triangulated_displaced_vertices())
                .map(|verts| Either::Left(verts))
                .unwrap_or_else(|| Either::Right(face.triangulate().flat_map(|verts| verts)))
        })
        .flat_map(map_coords)
        .collect();

    let mut mesh = CPUMesh {
        positions,
        ..Default::default()
    };

    mesh.compute_normals();

    mesh
}

fn load_prop(loader: &Loader, prop: Handle<StaticPropLump>) -> Result<CPUMesh, Error> {
    let mut mesh = load_prop_mesh(loader, prop.model())?;

    let transform =
        Mat4::from_translation(map_coords(prop.origin).into()) * Mat4::from(prop.rotation());
    mesh.transform(&transform);
    Ok(mesh)
}

#[tracing::instrument(skip(loader))]
fn load_prop_mesh(loader: &Loader, name: &str) -> Result<CPUMesh, Error> {
    let mdl = Mdl::read(&loader.load(name)?)?;
    let vtx = Vtx::read(&loader.load(&name.replace(".mdl", ".dx90.vtx"))?)?;
    let vvd = Vvd::read(&loader.load(&name.replace(".mdl", ".vvd"))?)?;

    let model = vmdl::Model::from_parts(mdl, vtx, vvd);
    Ok(prop_to_mesh(&model))
}

fn prop_to_mesh(model: &vmdl::Model) -> CPUMesh {
    let positions: Vec<f32> = model
        .vertices()
        .iter()
        .flat_map(|v| map_coords(v.position))
        .collect();
    let normals: Vec<f32> = model
        .vertices()
        .iter()
        .flat_map(|vertex| map_coords(vertex.normal))
        .collect();
    let indices = Indices::U32(
        model
            .vertex_strip_indices()
            .flat_map(|strip| strip.map(|index| index as u32))
            .collect(),
    );

    let mut mesh = CPUMesh {
        positions,
        normals: Some(normals),
        indices: Some(indices),
        ..Default::default()
    };
    mesh.compute_normals();
    mesh
}

fn load_world(path: &Path) -> Result<(CPUMesh, Bsp), Error> {
    use mmarinus::{perms, Kind};

    let map = Kind::Private.load::<perms::Read, _>(path).unwrap();
    let bsp = Bsp::read(map.as_ref())?;
    let world_model = bsp.models().next().ok_or(Error::Other("No world model"))?;

    Ok((model_to_mesh(world_model), bsp))
}

fn merge_meshes<I: IntoIterator<Item = CPUMesh>>(meshes: I) -> CPUMesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    for mesh in meshes {
        mesh.validate().expect("invalid mesh");
        let offset = positions.len() as u32 / 3;
        positions.extend_from_slice(&mesh.positions);
        normals.extend_from_slice(&mesh.normals.unwrap());
        if let Indices::U32(mesh_indices) = mesh.indices.unwrap() {
            indices.extend(mesh_indices.into_iter().map(|index| index + offset));
        } else {
            unreachable!();
        }
    }

    CPUMesh {
        positions,
        normals: Some(normals),
        indices: Some(Indices::U32(indices)),
        ..Default::default()
    }
}
