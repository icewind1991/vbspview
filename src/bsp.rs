use crate::material::{convert_material, load_material_fallback};
use crate::prop::load_props;
use crate::Error;
use cgmath::Matrix4;
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use tf_asset_loader::Loader;
use three_d::{CpuModel, Positions, Vec3};
use three_d_asset::{Geometry, Primitive, TriMesh};
use vbsp::{Bsp, Entity, Handle, Vector};

pub fn load_map(
    data: &[u8],
    loader: &mut Loader,
    props: bool,
    textures: bool,
) -> Result<Vec<CpuModel>, Error> {
    let (world, bsp) = load_world(data, loader, textures)?;
    let mut models = Vec::with_capacity(bsp.static_props().count() + 1);
    models.push(world);
    if props {
        let props = load_props(loader, bsp.static_props(), textures)?;
        models.extend(props);
    }
    Ok(models)
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

fn model_to_model(
    models: &[(Handle<vbsp::data::Model>, Vector)],
    loader: &Loader,
    textures: bool,
) -> CpuModel {
    let textures: Vec<&str> = if textures {
        let textures: HashSet<&str> = models
            .iter()
            .flat_map(|(model, _)| model.textures())
            .map(|texture| texture.name())
            .collect();
        textures.into_iter().collect()
    } else {
        Vec::new()
    };

    let faces_by_texture: HashMap<&str, _> = models
        .iter()
        .flat_map(|(model, origin)| model.faces().map(|face| (face, *origin)))
        .filter(|(face, _)| face.is_visible())
        .map(|(face, origin)| (face.texture().name(), (face, origin)))
        .into_group_map();

    let geometries: Vec<_> = faces_by_texture
        .into_values()
        .map(|faces| {
            let positions: Vec<_> = faces
                .iter()
                .flat_map(|(face, origin)| face.vertex_positions().map(|pos| pos + *origin))
                .map(map_coords)
                .collect();

            let uvs: Vec<_> = faces
                .iter()
                .flat_map(|(face, _)| {
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

            let texture = faces.first().unwrap().0.texture().name();
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
        .map(convert_material)
        .collect();

    CpuModel {
        name: "bsp".to_string(),
        geometries,
        materials,
    }
}

fn load_world(data: &[u8], loader: &mut Loader, textures: bool) -> Result<(CpuModel, Bsp), Error> {
    let bsp = Bsp::read(data)?;

    loader.add_source(bsp.pack.clone());

    let world_model = bsp
        .models()
        .next()
        .ok_or(Error::Other("No world model".into()))?;

    let mut models: Vec<_> = bsp
        .entities
        .iter()
        .flat_map(|ent| ent.parse())
        .filter_map(|ent| match ent {
            Entity::Brush(ent)
            | Entity::BrushIllusionary(ent)
            | Entity::BrushWall(ent)
            | Entity::BrushWallToggle(ent) => Some(ent),
            _ => None,
        })
        .flat_map(|brush| Some((brush.model[1..].parse::<usize>().ok()?, brush.origin)))
        .flat_map(|(index, origin)| Some((bsp.models().nth(index)?, origin)))
        .collect();
    models.push((
        world_model,
        Vector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
    ));

    let world_model = model_to_model(&models, loader, textures);
    Ok((world_model, bsp))
}
