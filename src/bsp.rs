use crate::{Error, Loader};
use cgmath::{vec4, Matrix, SquareMatrix};
use std::collections::HashMap;
use three_d::{
    Color, CpuMaterial, CpuMesh, CpuModel, CpuTexture, Mat4, Positions, TextureData, Vec2, Vec3,
};
use tracing::error;
use vbsp::{Bsp, Face, Handle, StaticPropLump};
use vmdl::mdl::{Mdl, TextureInfo};
use vmdl::vtx::Vtx;
use vmdl::vvd::Vvd;
use vtf::vtf::VTF;

pub fn load_map(data: &[u8], loader: &mut Loader) -> Result<Vec<CpuModel>, Error> {
    let (world, bsp) = load_world(data, loader)?;
    let props = load_props(loader, bsp.static_props())?;
    Ok(vec![world, props])
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

fn load_props<'a, I: Iterator<Item = Handle<'a, StaticPropLump>>>(
    loader: &Loader,
    props: I,
) -> Result<CpuModel, Error> {
    let props: Vec<ModelData> = props
        .map(|prop| {
            let model = load_prop(loader, prop.model())?;
            let transform =
                Mat4::from_translation(map_coords(prop.origin)) * Mat4::from(prop.rotation());
            Ok::<_, Error>(ModelData {
                model,
                transform,
                skin: prop.skin,
            })
        })
        .collect::<Result<_, _>>()?;

    let geometries = props.iter().flat_map(prop_to_meshes).collect();

    let textures: HashMap<_, _> = props
        .iter()
        .flat_map(|prop| prop.model.textures())
        .map(|tex| (tex.name.as_str(), tex))
        .collect();
    let materials: Vec<_> = textures
        .into_values()
        .map(|tex| prop_texture_to_material(tex, loader))
        .collect();
    Ok(CpuModel {
        geometries,
        materials,
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
    skin: i32,
}

fn prop_to_meshes(prop: &ModelData) -> impl Iterator<Item = CpuMesh> + '_ {
    let transform = prop.transform;
    let normal_transform = transform.invert().unwrap().transpose() * -1.0;
    let model = &prop.model;

    let skin = match model.skin_tables().nth(prop.skin as usize) {
        Some(skin) => skin,
        None => {
            error!(index = prop.skin, "invalid skin index");
            model.skin_tables().next().unwrap()
        }
    };

    model.meshes().map(move |mesh| {
        let texture = skin
            .texture(mesh.material_index())
            .expect("texture out of bounds");

        let positions: Vec<Vec3> = mesh
            .vertices()
            .map(|vertex| map_coords(vertex.position))
            .map(|v| apply_transform(v, transform))
            .collect();
        let normals: Vec<Vec3> = mesh
            .vertices()
            .map(|vertex| map_coords(vertex.normal))
            .map(|v| apply_transform(v, normal_transform))
            .collect();
        let uvs: Vec<Vec2> = mesh
            .vertices()
            .map(|vertex| vertex.texture_coordinates.into())
            .collect();

        CpuMesh {
            positions: Positions::F32(positions),
            normals: Some(normals),
            uvs: Some(uvs),
            material_name: Some(texture.into()),
            ..Default::default()
        }
    })
}

fn prop_texture_to_material(texture: &TextureInfo, loader: &Loader) -> CpuMaterial {
    match load_texture(&texture.name, &texture.search_paths, loader) {
        Ok(texture) => CpuMaterial {
            albedo: Color::default(),
            name: texture.name.clone(),
            albedo_texture: Some(texture),
            ..Default::default()
        },
        Err(_) => CpuMaterial {
            albedo: Color {
                r: 255,
                g: 0,
                b: 255,
                a: 255,
            },
            name: texture.name.clone(),
            ..Default::default()
        },
    }
}

fn load_texture(name: &str, dirs: &[String], loader: &Loader) -> Result<CpuTexture, Error> {
    let dirs = dirs
        .iter()
        .map(|dir| format!("materials/{}", dir))
        .collect::<Vec<_>>();
    let path = format!("{}.vtf", name);
    let mut raw = loader.load_from_paths(&path, &dirs)?;
    let vtf = VTF::read(&mut raw)?;
    let image = vtf.highres_image.decode(0)?;
    Ok(CpuTexture {
        name: name.into(),
        data: TextureData::RgbaU8(image.into_rgba8().pixels().map(|pixel| pixel.0).collect()),
        height: vtf.header.height as u32,
        width: vtf.header.width as u32,
        ..CpuTexture::default()
    })
}

fn load_world(data: &[u8], loader: &mut Loader) -> Result<(CpuModel, Bsp), Error> {
    let bsp = Bsp::read(data)?;

    loader.set_pack(bsp.pack.clone());

    let world_model = bsp.models().next().ok_or(Error::Other("No world model"))?;

    Ok((model_to_model(world_model, loader), bsp))
}
