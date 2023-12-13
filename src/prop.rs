use crate::bsp::map_coords;
use crate::material::load_material_fallback;
use crate::{Error, Loader};
use std::collections::HashMap;
use three_d::{CpuMaterial, CpuModel, Mat4, Positions, Vec2, Vec3, Vec4};
use three_d_asset::{Geometry, Primitive, TriMesh};
use tracing::{error, warn};
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
        .filter_map(|prop| match load_prop(loader, prop.model()) {
            Ok(model) => Some((prop, model)),
            Err(e) => {
                error!(error = ?e, prop = prop.model(), "Failed to load prop");
                None
            }
        })
        .map(|(prop, model)| {
            let transform =
                Mat4::from_translation(map_coords(prop.origin)) * Mat4::from(prop.rotation());
            PropData {
                name: prop.model(),
                model,
                transform,
                skin: prop.skin,
            }
        })
        .collect();

    let materials: HashMap<_, _> = props
        .iter()
        .flat_map(|prop| prop.model.textures())
        .map(|tex| (tex.name.as_str(), tex))
        .collect();
    let materials: Vec<_> = materials.into_values().collect();

    let geometries = props
        .iter()
        .flat_map(|prop| prop_to_meshes(prop, materials.as_slice()))
        .collect();

    let materials: Vec<_> = materials
        .into_iter()
        .map(|tex| prop_texture_to_material(tex, loader))
        .collect();

    Ok(CpuModel {
        name: "props".into(),
        geometries,
        materials,
    })
}

struct PropData<'a> {
    name: &'a str,
    model: vmdl::Model,
    transform: Mat4,
    skin: i32,
}

fn prop_to_meshes<'a>(
    prop: &'a PropData,
    textures: &'a [&TextureInfo],
) -> impl Iterator<Item = Primitive> + 'a {
    let transform = prop.transform;
    let model = &prop.model;

    let skin = match model.skin_tables().nth(prop.skin as usize) {
        Some(skin) => skin,
        None => {
            warn!(index = prop.skin, prop = prop.name, "invalid skin index");
            model.skin_tables().next().unwrap()
        }
    };

    model.meshes().map(move |mesh| {
        let texture = skin
            .texture(mesh.material_index())
            .expect("texture out of bounds");
        let material_index = textures
            .iter()
            .enumerate()
            .find_map(|(i, texture_info)| (texture_info.name == texture).then_some(i));

        let positions: Vec<Vec3> = mesh
            .vertices()
            .map(|vertex| map_coords(vertex.position))
            // .map(|v| apply_transform(v, transform))
            .collect();
        let normals: Vec<Vec3> = mesh
            .vertices()
            .map(|vertex| map_coords(vertex.normal))
            // .map(|v| apply_transform(v, normal_transform))
            .collect();
        let uvs: Vec<Vec2> = mesh
            .vertices()
            .map(|vertex| vertex.texture_coordinates.into())
            .collect();

        let tangents: Vec<Vec4> = mesh.tangents().map(|tangent| tangent.into()).collect();

        let geometry = Geometry::Triangles(TriMesh {
            positions: Positions::F32(positions),
            indices: Default::default(),
            normals: Some(normals),
            uvs: Some(uvs),
            tangents: Some(tangents),
            colors: None,
        });

        Primitive {
            name: "".to_string(),
            transformation: transform,
            animations: vec![],
            geometry,
            material_index,
        }
    })
}

fn prop_texture_to_material(texture: &TextureInfo, loader: &Loader) -> CpuMaterial {
    load_material_fallback(&texture.name, &texture.search_paths, loader)
}
