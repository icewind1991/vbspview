use crate::{Error, Loader};
use cgmath::{vec4, Matrix, SquareMatrix};
use itertools::Either;
use three_d::{CPUMesh, Indices, Mat4};
use vbsp::{Bsp, Handle, StaticPropLump};
use vmdl::mdl::Mdl;
use vmdl::vtx::Vtx;
use vmdl::vvd::Vvd;

pub fn load_map(data: &[u8]) -> Result<Vec<CPUMesh>, Error> {
    let (cpu_mesh, bsp) = load_world(data)?;
    let loader = Loader::new(bsp.pack.clone())?;
    let merged_props = load_props(&loader, bsp.static_props())?;
    Ok(vec![cpu_mesh, merged_props])
}

fn apply_transform(coord: [f32; 3], transform: Mat4) -> [f32; 3] {
    let coord = (transform * vec4(coord[0], coord[1], coord[2], 1.0)).truncate();
    [coord.x, coord.y, coord.z]
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
    merge_models(props.map(|prop| {
        let model = load_prop(loader, prop.model())?;
        let transform =
            Mat4::from_translation(map_coords(prop.origin).into()) * Mat4::from(prop.rotation());
        Ok(ModelData { model, transform })
    }))
}

#[tracing::instrument(skip(loader))]
fn load_prop(loader: &Loader, name: &str) -> Result<vmdl::Model, Error> {
    let mdl = Mdl::read(&loader.load(name)?)?;
    let vtx = Vtx::read(&loader.load(&name.replace(".mdl", ".dx90.vtx"))?)?;
    let vvd = Vvd::read(&loader.load(&name.replace(".mdl", ".vvd"))?)?;

    Ok(vmdl::Model::from_parts(mdl, vtx, vvd))
}

struct ModelData {
    model: vmdl::Model,
    transform: Mat4,
}

fn merge_models<I: Iterator<Item = Result<ModelData, Error>>>(props: I) -> Result<CPUMesh, Error> {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    for prop in props {
        let prop = prop?;
        let transform = prop.transform;
        let normal_transform = transform.invert().unwrap().transpose() * -1.0;
        let model = prop.model;

        let offset = positions.len() as u32 / 3;

        positions.extend(
            model
                .vertices()
                .iter()
                .map(|v| map_coords(v.position))
                .flat_map(|v| apply_transform(v, transform)),
        );
        normals.extend(
            model
                .vertices()
                .iter()
                .map(|v| map_coords(v.normal))
                .flat_map(|v| apply_transform(v, normal_transform)),
        );
        indices.extend(
            model
                .vertex_strip_indices()
                .flat_map(|strip| strip.map(|index| index as u32))
                .map(|index| index + offset),
        );
    }

    Ok(CPUMesh {
        positions,
        normals: Some(normals),
        indices: Some(Indices::U32(indices)),
        ..Default::default()
    })
}

fn load_world(data: &[u8]) -> Result<(CPUMesh, Bsp), Error> {
    let bsp = Bsp::read(data)?;
    let world_model = bsp.models().next().ok_or(Error::Other("No world model"))?;

    Ok((model_to_mesh(world_model), bsp))
}
