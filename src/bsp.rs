use crate::prop::load_props;
use crate::{Error, Loader};
use cgmath::vec4;
use std::collections::HashMap;
use three_d::{
    Color, CpuMaterial, CpuMesh, CpuModel, CpuTexture, Mat4, Positions, TextureData, Vec2, Vec3,
};
use vbsp::{Bsp, Face, Handle};

pub fn load_map(data: &[u8], loader: &mut Loader) -> Result<Vec<CpuModel>, Error> {
    let (world, bsp) = load_world(data, loader)?;
    let props = load_props(loader, bsp.static_props())?;
    Ok(vec![world, props])
}

pub fn apply_transform<C: Into<Vec3>>(coord: C, transform: Mat4) -> Vec3 {
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
            let tex_file = format!("materials/{}.vtf", texture.name().to_lowercase());
            let vtf_data = loader.load(&tex_file).ok();
            let texture_data = vtf_data.and_then(|mut vtf_data| {
                let vtf = vtf::from_bytes(&mut vtf_data).ok()?;
                let image = vtf.highres_image.decode(0).ok()?;
                Some(CpuTexture {
                    name: texture.name().into(),
                    data: TextureData::RgbaU8(
                        image.into_rgba8().pixels().map(|pixel| pixel.0).collect(),
                    ),
                    height: texture.texture_data().height as u32,
                    width: texture.texture_data().width as u32,
                    ..CpuTexture::default()
                })
            });
            let color = if texture_data.is_some() {
                Color::default()
            } else {
                Color::new(255, 0, 255, 255)
            };
            CpuMaterial {
                albedo: color,
                albedo_texture: texture_data,
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

fn load_world(data: &[u8], loader: &mut Loader) -> Result<(CpuModel, Bsp), Error> {
    let bsp = Bsp::read(data)?;

    loader.set_pack(bsp.pack.clone());

    let world_model = bsp.models().next().ok_or(Error::Other("No world model"))?;

    Ok((model_to_model(world_model, loader), bsp))
}
