use crate::bsp::map_coords;
use crate::material::{convert_material, load_material_fallback, MaterialSet};
use crate::Error;
use rayon::prelude::*;
use tf_asset_loader::Loader;
use three_d::{CpuMaterial, CpuModel, Mat4, Positions, Vec2, Vec3, Vec4};
use three_d_asset::{Geometry, Primitive, TriMesh};
use tracing::{error, warn};
use vbsp::PropPlacement;
use vmdl::mdl::Mdl;
use vmdl::vtx::Vtx;
use vmdl::vvd::Vvd;

#[tracing::instrument(skip(loader))]
pub fn load_prop(loader: &Loader, name: &str) -> Result<vmdl::Model, Error> {
    let load = |name: &str| -> Result<Vec<u8>, Error> {
        loader
            .load(name)?
            .ok_or(Error::ResourceNotFound(name.into()))
    };
    let mdl = Mdl::read(&load(name)?)?;
    let vtx = Vtx::read(&load(&name.replace(".mdl", ".dx90.vtx"))?)?;
    let vvd = Vvd::read(&load(&name.replace(".mdl", ".vvd"))?)?;

    Ok(vmdl::Model::from_parts(mdl, vtx, vvd))
}
pub fn load_props<'a, I: Iterator<Item = PropPlacement<'a>>>(
    loader: &Loader,
    props: I,
    show_textures: bool,
) -> Result<Vec<CpuModel>, Error> {
    let props: Vec<_> = props
        .filter_map(|prop| match load_prop(loader, prop.model) {
            Ok(model) => Some((prop, model)),
            Err(e) => {
                error!(error = ?e, prop = prop.model, "Failed to load prop");
                None
            }
        })
        .map(|(prop, model)| {
            let transform = Mat4::from_translation(map_coords(prop.origin))
                * Mat4::from(prop.rotation)
                * Mat4::from_scale(prop.scale);
            PropData {
                name: prop.model,
                model,
                transform,
                skin: prop.skin,
            }
        })
        .collect();

    let used_materials = MaterialSet::new(loader);

    let geometries = props
        .iter()
        .flat_map(|prop| prop_to_meshes(prop, &used_materials, show_textures))
        .collect();

    let materials = used_materials
        .into_materials()
        .into_par_iter()
        .map(|mat| prop_texture_to_material(&mat, loader))
        .collect();

    Ok(vec![CpuModel {
        name: "props".into(),
        geometries,
        materials,
    }])
}

struct PropData<'a> {
    name: &'a str,
    model: vmdl::Model,
    transform: Mat4,
    skin: i32,
}

fn prop_to_meshes<'a>(
    prop: &'a PropData,
    used_materials: &'a MaterialSet<'a>,
    show_textures: bool,
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
        let material_index = if show_textures {
            skin.texture_info(mesh.material_index())
                .map(|mat| used_materials.get_index(mat))
        } else {
            None
        };

        let positions: Vec<Vec3> = mesh
            .vertices()
            .map(|vertex| model.apply_root_transform(vertex.position))
            .map(map_coords)
            .collect();
        let normals: Vec<Vec3> = mesh
            .vertices()
            .map(|vertex| map_coords(vertex.normal))
            .collect();
        let uvs: Vec<Vec2> = mesh
            .vertices()
            .map(|vertex| vertex.texture_coordinates.into())
            .collect();

        let tangents: Vec<Vec4> = mesh.tangents().map(|tangent| tangent.into()).collect();

        let geometry = Geometry::Triangles(TriMesh {
            positions: Positions::F32(positions),
            normals: Some(normals),
            uvs: Some(uvs),
            tangents: Some(tangents),
            ..TriMesh::default()
        });

        Primitive {
            name: mesh.model_name.into(),
            transformation: transform,
            animations: vec![],
            geometry,
            material_index,
        }
    })
}

fn prop_texture_to_material(texture: &str, loader: &Loader) -> CpuMaterial {
    convert_material(load_material_fallback(texture, loader))
}
