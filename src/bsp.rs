use crate::{Error, Loader};
use cgmath::{vec4, Matrix, SquareMatrix};
use std::collections::HashMap;
use three_d::{Color, CpuMaterial, CpuMesh, CpuModel, Indices, Mat4, Positions, Vec2, Vec3};
use vbsp::{Bsp, Face, Handle, StaticPropLump};
use vmdl::mdl::Mdl;
use vmdl::vtx::Vtx;
use vmdl::vvd::Vvd;

pub fn load_map(data: &[u8], loader: &mut Loader) -> Result<Vec<CpuModel>, Error> {
    let (cpu_model, bsp) = load_world(data, loader)?;
    let merged_props = load_props(loader, bsp.static_props())?;
    Ok(vec![cpu_model, merged_props])
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

fn face_to_mesh(face: &Handle<Face>) -> CpuMesh {
    let texture = face.texture();
    let positions = face.vertex_positions().map(map_coords).collect();
    let uvs = face
        .vertex_positions()
        .map(|pos| Vec2 {
            x: texture.u(pos),
            y: texture.v(pos),
        })
        .collect();

    let mut mesh = CpuMesh {
        positions: Positions::F32(positions),
        uvs: Some(uvs),
        material_name: Some(texture.name().into()),
        ..Default::default()
    };
    mesh.compute_normals();
    mesh
}

fn model_to_model(model: Handle<vbsp::data::Model>, loader: &Loader) -> CpuModel {
    let mut faces_by_texture: HashMap<&str, Vec<_>> = HashMap::with_capacity(64);
    for face in model.faces().filter(|face| face.is_visible()) {
        faces_by_texture
            .entry(face.texture().name())
            .or_default()
            .push(face)
    }

    let geometries = faces_by_texture
        .values()
        .map(|faces| {
            let mut faces = faces.iter();
            let first = faces.next().unwrap();
            let mut mesh = face_to_mesh(first);
            for face in faces {
                let face_mesh = face_to_mesh(face);
                if let Positions::F32(positions) = &mut mesh.positions {
                    positions.extend_from_slice(&face_mesh.positions.into_f32());
                }
                if let Some(uvs) = &mut mesh.uvs {
                    uvs.extend_from_slice(&face_mesh.uvs.unwrap());
                }
            }
            mesh.compute_normals();
            mesh
        })
        .collect();

    let materials = faces_by_texture
        .values()
        .map(|face| {
            let texture = face.first().unwrap().texture();
            let color = texture.texture().debug_color();
            CpuMaterial {
                albedo: Color {
                    r: color[0],
                    g: color[1],
                    b: color[2],
                    a: 255,
                },
                name: texture.name().into(),
                ..Default::default()
            }
        })
        .collect();

    CpuModel {
        geometries,
        materials,
    }
}

fn load_props<'a, I: Iterator<Item = Handle<'a, StaticPropLump>>>(
    loader: &Loader,
    props: I,
) -> Result<CpuModel, Error> {
    let material = CpuMaterial {
        albedo: Color {
            r: 128,
            g: 128,
            b: 128,
            a: 255,
        },
        ..Default::default()
    };

    let mesh = merge_models(props.map(|prop| {
        let model = load_prop(loader, prop.model())?;
        let transform =
            Mat4::from_translation(map_coords(prop.origin)) * Mat4::from(prop.rotation());
        Ok(ModelData { model, transform })
    }))?;

    Ok(CpuModel {
        geometries: vec![mesh],
        materials: vec![material],
    })
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

fn load_world(data: &[u8], loader: &mut Loader) -> Result<(CpuModel, Bsp), Error> {
    let bsp = Bsp::read(data)?;

    loader.set_pack(bsp.pack.clone());

    let world_model = bsp.models().next().ok_or(Error::Other("No world model"))?;

    Ok((model_to_model(world_model, loader), bsp))
}
