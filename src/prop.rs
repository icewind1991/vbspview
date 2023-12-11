use crate::bsp::{apply_transform, map_coords};
use crate::material::load_material_fallback;
use crate::{Error, Loader};
use cgmath::{Matrix, SquareMatrix};
use std::collections::HashMap;
use three_d::{CpuMaterial, CpuMesh, CpuModel, Mat4, Positions, Vec2, Vec3};
use tracing::warn;
use vbsp::{Handle, StaticPropLump};
use vmdl::mdl::{Mdl, TextureInfo};
use vmdl::vtx::Vtx;
use vmdl::vvd::Vvd;

#[tracing::instrument(skip(loader))]
pub fn load_prop(loader: &Loader, name: &str) -> Result<vmdl::Model, Error> {
    let mdl = Mdl::read(&loader.load(name)?)?;
    let vtx = Vtx::read(&loader.load(&name.replace(".mdl", ".dx90.vtx"))?)?;
    let vvd = Vvd::read(&loader.load(&name.replace(".mdl", ".vvd"))?)?;

    Ok(vmdl::Model::from_parts(mdl, vtx, vvd))
}
pub fn load_props<'a, I: Iterator<Item = Handle<'a, StaticPropLump>>>(
    loader: &Loader,
    props: I,
) -> Result<CpuModel, Error> {
    let props: Vec<PropData> = props
        .map(|prop| {
            let model = load_prop(loader, prop.model())?;
            let transform =
                Mat4::from_translation(map_coords(prop.origin)) * Mat4::from(prop.rotation());
            Ok::<_, Error>(PropData {
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

struct PropData {
    model: vmdl::Model,
    transform: Mat4,
    skin: i32,
}

fn prop_to_meshes(prop: &PropData) -> impl Iterator<Item = CpuMesh> + '_ {
    let transform = prop.transform;
    let normal_transform = transform.invert().unwrap().transpose() * -1.0;
    let model = &prop.model;

    let skin = match model.skin_tables().nth(prop.skin as usize) {
        Some(skin) => skin,
        None => {
            warn!(index = prop.skin, "invalid skin index");
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
    load_material_fallback(&texture.name, &texture.search_paths, loader)
}
