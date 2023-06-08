use crate::{Error, Loader};
use cgmath::{vec4, Matrix, SquareMatrix};
use itertools::Either;
use three_d::{CpuMesh, Indices, Mat4, Positions, Vec3};
use vbsp::{Bsp, Handle, StaticPropLump};
use vmdl::mdl::Mdl;
use vmdl::vtx::Vtx;
use vmdl::vvd::Vvd;

pub fn load_map(data: &[u8], loader: &mut Loader) -> Result<Vec<CpuMesh>, Error> {
    let (cpu_mesh, bsp) = load_world(data)?;
    loader.set_pack(bsp.pack.clone());
    let merged_props = load_props(loader, bsp.static_props())?;
    Ok(vec![cpu_mesh, merged_props])
}

fn apply_transform<C: Into<Vec3>>(coord: C, transform: Mat4) -> Vec3 {
    let coord = coord.into();
    (transform * vec4(coord.x, coord.y, coord.z, 1.0)).truncate()
}

pub fn map_coords<C: Into<Vec3>>(vec: C) -> Vec3 {
    let vec = vec.into();
    Vec3 {
        x: vec.y * UNIT_SCALE,
        y: vec.z * UNIT_SCALE,
        z: vec.x * UNIT_SCALE,
    }
}

// 1 hammer unit is ~1.905cm
pub const UNIT_SCALE: f32 = 1.0 / (1.905 * 100.0);

fn model_to_mesh(model: Handle<vbsp::data::Model>) -> CpuMesh {
    let positions: Vec<Vec3> = model
        .faces()
        .filter(|face| face.is_visible())
        .flat_map(|face| {
            face.displacement()
                .map(|displacement| displacement.triangulated_displaced_vertices())
                .map(Either::Left)
                .unwrap_or_else(|| Either::Right(face.triangulate().flatten()))
        })
        .map(map_coords)
        .collect();

    let mut mesh = CpuMesh {
        positions: Positions::F32(positions),
        ..Default::default()
    };

    mesh.compute_normals();

    mesh
}

fn load_props<'a, I: Iterator<Item = Handle<'a, StaticPropLump>>>(
    loader: &Loader,
    props: I,
) -> Result<CpuMesh, Error> {
    merge_models(props.map(|prop| {
        let model = load_prop(loader, prop.model())?;
        let transform =
            Mat4::from_translation(map_coords(prop.origin)) * Mat4::from(prop.rotation());
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

fn merge_models<I: Iterator<Item = Result<ModelData, Error>>>(props: I) -> Result<CpuMesh, Error> {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    for prop in props {
        let prop = prop?;
        let transform = prop.transform;
        let normal_transform = transform.invert().unwrap().transpose() * -1.0;
        let model = prop.model;

        let offset = positions.len() as u32;

        positions.extend(
            model
                .vertices()
                .iter()
                .map(|v| map_coords(v.position))
                .map(|v| apply_transform(v, transform)),
        );
        normals.extend(
            model
                .vertices()
                .iter()
                .map(|v| map_coords(v.normal))
                .map(|v| apply_transform(v, normal_transform)),
        );
        indices.extend(
            model
                .vertex_strip_indices()
                .flat_map(|strip| strip.map(|index| index as u32))
                .map(|index| index + offset),
        );
    }

    Ok(CpuMesh {
        positions: Positions::F32(positions),
        normals: Some(normals),
        indices: Indices::U32(indices),
        ..Default::default()
    })
}

fn load_world(data: &[u8]) -> Result<(CpuMesh, Bsp), Error> {
    let bsp = Bsp::read(data)?;
    let world_model = bsp.models().next().ok_or(Error::Other("No world model"))?;

    Ok((model_to_mesh(world_model), bsp))
}
