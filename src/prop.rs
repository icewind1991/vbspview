use crate::bsp::map_coords;
use crate::material::{convert_material, load_material_fallback};
use crate::{Error, Loader};
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
) -> Result<Vec<CpuModel>, Error> {
    let props = props
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
        });

    props
        .map(|prop| {
            let geometries: Vec<_> = prop_to_meshes(&prop).collect();
            let materials: Vec<_> = prop
                .model
                .textures()
                .iter()
                .map(|tex| prop_texture_to_material(tex, loader))
                .collect();

            Ok(CpuModel {
                name: prop.name.into(),
                geometries,
                materials,
            })
        })
        .collect()
}

struct PropData<'a> {
    name: &'a str,
    model: vmdl::Model,
    transform: Mat4,
    skin: i32,
}

fn prop_to_meshes<'a>(prop: &'a PropData) -> impl Iterator<Item = Primitive> + 'a {
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
        let material_index = skin.texture_index(mesh.material_index());

        let positions: Vec<Vec3> = mesh
            .vertices()
            .map(|vertex| map_coords(vertex.position))
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

fn prop_texture_to_material(texture: &TextureInfo, loader: &Loader) -> CpuMaterial {
    convert_material(load_material_fallback(
        &texture.name,
        &texture.search_paths,
        loader,
    ))
}
