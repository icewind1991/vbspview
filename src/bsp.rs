use crate::material::load_material_fallback;
use crate::prop::load_props;
use crate::{Error, Loader};
use cgmath::Matrix4;
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use three_d::{CpuModel, Positions, Vec3};
use three_d_asset::{Geometry, Primitive, TriMesh};
use vbsp::{Bsp, Handle};

pub fn load_map(data: &[u8], loader: &mut Loader) -> Result<Vec<CpuModel>, Error> {
    let (world, bsp) = load_world(data, loader)?;
    let props = load_props(loader, bsp.static_props())?;
    Ok(vec![world, props])
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

fn model_to_model(model: Handle<vbsp::data::Model>, loader: &Loader) -> CpuModel {
    let textures: HashSet<&str> = model.textures().map(|texture| texture.name()).collect();
    let textures: Vec<&str> = textures.into_iter().collect();

    let faces_by_texture: HashMap<&str, _> = model
        .faces()
        .filter(|face| face.is_visible())
        .map(|face| (face.texture().name(), face))
        .into_group_map();

    let geometries: Vec<_> = faces_by_texture
        .into_values()
        .map(|faces| {
            let positions: Vec<_> = faces
                .iter()
                .flat_map(|face| face.vertex_positions())
                .map(map_coords)
                .collect();

            let uvs: Vec<_> = faces
                .iter()
                .flat_map(|face| {
                    let texture = face.texture();
                    face.vertex_positions()
                        .map(move |position| texture.uv(position))
                })
                .map(|uv| uv.into())
                .collect();

            let mut mesh = TriMesh {
                positions: Positions::F32(positions),
                uvs: Some(uvs),
                ..Default::default()
            };
            mesh.compute_normals();
            mesh.compute_tangents();

            let texture = faces.first().unwrap().texture().name();
            let material_index = textures
                .iter()
                .enumerate()
                .find_map(|(i, tex)| (*tex == texture).then_some(i));

            Primitive {
                name: "".to_string(),
                transformation: Matrix4::from_scale(1.0),
                animations: vec![],
                geometry: Geometry::Triangles(mesh),
                material_index,
            }
        })
        .collect();

    let materials: Vec<_> = textures
        .iter()
        .map(|texture| load_material_fallback(texture, &["".into()], loader))
        .collect();

    CpuModel {
        name: "".to_string(),
        geometries,
        materials,
    }
}

fn load_world(data: &[u8], loader: &mut Loader) -> Result<(CpuModel, Bsp), Error> {
    let bsp = Bsp::read(data)?;

    loader.set_pack(bsp.pack.clone());

    let world_model = bsp.models().next().ok_or(Error::Other("No world model"))?;

    Ok((model_to_model(world_model, loader), bsp))
}
