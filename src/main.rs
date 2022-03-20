mod camera;
mod loader;
mod renderer;
mod ui;

use crate::renderer::Renderer;
use crate::ui::DebugUI;
use camera::FirstPerson;
use itertools::Either;
use loader::Loader;
use std::env::args;
use std::path::Path;
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

    let mut renderer = Renderer::new(&window)?;

    let (cpu_mesh, bsp) = load_world(file.as_ref())?;
    let material = PhysicalMaterial {
        albedo: Color {
            r: 128,
            g: 128,
            b: 128,
            a: 255,
        },
        ..Default::default()
    };

    let loader = Loader::new(bsp.pack.clone())?;
    let model = Model::new_with_material(&renderer.context, &cpu_mesh, material.clone())?;
    let merged_props = load_props(&loader, bsp.static_props())?;
    let props_model = Model::new_with_material(&renderer.context, &merged_props, material)?;
    renderer.models = vec![model, props_model];

    window.render_loop(move |frame_input| renderer.render(frame_input).unwrap())?;

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
                .map(Either::Left)
                .unwrap_or_else(|| Either::Right(face.triangulate().flatten()))
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

fn load_props<'a, I: Iterator<Item = Handle<'a, StaticPropLump>>>(
    loader: &Loader,
    props: I,
) -> Result<CPUMesh, Error> {
    merge_meshes(props.map(|prop| {
        let mut mesh = load_prop_mesh(loader, prop.model())?;

        let transform =
            Mat4::from_translation(map_coords(prop.origin).into()) * Mat4::from(prop.rotation());
        mesh.transform(&transform);
        Ok(mesh)
    }))
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

    CPUMesh {
        positions,
        normals: Some(normals),
        indices: Some(indices),
        ..Default::default()
    }
}

fn load_world(path: &Path) -> Result<(CPUMesh, Bsp), Error> {
    let map = std::fs::read(path)?;
    let bsp = Bsp::read(map.as_ref())?;
    let world_model = bsp.models().next().ok_or(Error::Other("No world model"))?;

    Ok((model_to_mesh(world_model), bsp))
}

fn merge_meshes<I: IntoIterator<Item = Result<CPUMesh, Error>>>(
    models: I,
) -> Result<CPUMesh, Error> {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    for mesh in models {
        let mesh = mesh?;
        let offset = positions.len() as u32 / 3;
        positions.extend_from_slice(&mesh.positions);
        normals.extend_from_slice(&mesh.normals.unwrap());
        if let Indices::U32(mesh_indices) = mesh.indices.unwrap() {
            indices.extend(mesh_indices.into_iter().map(|index| index + offset));
        } else {
            unreachable!();
        }
    }

    Ok(CPUMesh {
        positions,
        normals: Some(normals),
        indices: Some(Indices::U32(indices)),
        ..Default::default()
    })
}
