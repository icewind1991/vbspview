use crate::Error;
use image::DynamicImage;
use std::cell::RefCell;
use tf_asset_loader::Loader;
use three_d::{CpuMaterial, CpuTexture};
use three_d_asset::Srgba;
use tracing::{error, instrument};
use vmdl::mdl::TextureInfo;
use vmt_parser::material::{Material, WaterMaterial};
use vmt_parser::{from_str, TextureTransform};
use vtf::vtf::VTF;

pub fn load_material_fallback(name: &str, loader: &Loader) -> MaterialData {
    match load_material(name, loader) {
        Ok(mat) => mat,
        Err(e) => {
            error!(error = ?e, material = name, "failed to load material");
            MaterialData {
                path: name.into(),
                color: [255, 0, 255, 255],
                ..MaterialData::default()
            }
        }
    }
}

#[derive(Default, Debug)]
pub struct MaterialData {
    pub path: String,
    pub color: [u8; 4],
    pub texture: Option<TextureData>,
    pub alpha_test: Option<f32>,
    pub bump_map: Option<TextureData>,
    pub translucent: bool,
    #[allow(dead_code)]
    pub transform: Option<TextureTransform>,
}

#[derive(Debug)]
pub struct TextureData {
    pub name: String,
    pub image: DynamicImage,
}

#[instrument(skip(loader))]
pub fn load_material(path: &str, loader: &Loader) -> Result<MaterialData, Error> {
    let path = if path.starts_with("materials/") {
        path.to_string()
    } else {
        format!(
            "materials/{}.vmt",
            path.to_ascii_lowercase().trim_end_matches(".vmt")
        )
    };
    let raw = loader
        .load(&path)?
        .ok_or_else(|| Error::ResourceNotFound(path.clone()))?;
    let vdf = String::from_utf8(raw)?;

    let material = from_str(&vdf).map_err(|e| {
        let report = miette::ErrReport::new(e);
        println!("{:?}", report);
        Error::Other(format!("Failed to load material {}", path))
    })?;
    let material = material.resolve(|path| {
        let data = loader
            .load(path)?
            .ok_or(Error::ResourceNotFound(path.into()))?;
        let vdf = String::from_utf8(data)?;
        Ok::<_, Error>(vdf)
    })?;

    if let Material::Water(WaterMaterial {
        base_texture: None, ..
    }) = &material
    {
        return Ok(MaterialData {
            color: [82, 180, 217, 128],
            path,
            translucent: true,
            ..MaterialData::default()
        });
    }

    let base_texture = material
        .base_texture()
        .ok_or_else(|| Error::Other(format!("{path} has no base texture")))?;

    let translucent = material.translucent();
    let glass = material.surface_prop() == Some("glass");
    let alpha_test = material.alpha_test();
    let texture = load_texture(base_texture, loader)?;

    let bump_map = material.bump_map().and_then(|path| {
        Some(TextureData {
            image: load_texture(path, loader).ok()?,
            name: path.into(),
        })
    });

    let transform = material
        .base_texture_transform()
        .filter(|transform| **transform != TextureTransform::default())
        .cloned();

    Ok(MaterialData {
        color: [255; 4],
        path,
        texture: Some(TextureData {
            name: base_texture.into(),
            image: texture,
        }),
        bump_map,
        alpha_test,
        translucent: translucent | glass,
        transform,
    })
}

fn load_texture(name: &str, loader: &Loader) -> Result<DynamicImage, Error> {
    let path = format!(
        "materials/{}.vtf",
        name.trim_end_matches(".vtf").trim_start_matches('/')
    );
    let mut raw = loader.load(&path)?.ok_or(Error::ResourceNotFound(path))?;
    let vtf = VTF::read(&mut raw)?;
    let image = vtf.highres_image.decode(0)?;
    Ok(image)
}

pub fn convert_material(material: MaterialData) -> CpuMaterial {
    CpuMaterial {
        albedo: Srgba::new(
            material.color[0],
            material.color[1],
            material.color[2],
            material.color[3],
        ),
        name: material.path,
        albedo_texture: material
            .texture
            .map(|tex| convert_texture(tex, material.translucent | material.alpha_test.is_some())),
        alpha_cutout: material.alpha_test,
        normal_texture: material.bump_map.map(|tex| convert_texture(tex, true)),
        ..CpuMaterial::default()
    }
}
pub fn convert_texture(texture: TextureData, keep_alpha: bool) -> CpuTexture {
    let image = texture.image;
    let width = image.width();
    let height = image.height();
    let data = if image.color().has_alpha() && keep_alpha {
        three_d_asset::TextureData::RgbaU8(
            image.into_rgba8().pixels().map(|pixel| pixel.0).collect(),
        )
    } else {
        three_d_asset::TextureData::RgbU8(image.into_rgb8().pixels().map(|pixel| pixel.0).collect())
    };
    CpuTexture {
        data,
        name: texture.name,
        height,
        width,
        ..CpuTexture::default()
    }
}

#[derive(Debug)]
pub struct MaterialSet<'a> {
    loader: &'a Loader,
    materials: RefCell<Vec<String>>,
}

impl<'s> MaterialSet<'s> {
    pub fn new(loader: &'s Loader) -> Self {
        MaterialSet {
            loader,
            materials: RefCell::default(),
        }
    }

    pub fn get_index(&self, material: &TextureInfo) -> usize {
        let search_path = material
            .search_paths
            .iter()
            .map(|dir| {
                format!(
                    "materials/{}",
                    dir.to_ascii_lowercase().trim_start_matches('/')
                )
            })
            .collect::<Vec<_>>();

        let material = format!(
            "{}.vmt",
            material.name.to_ascii_lowercase().trim_end_matches(".vmt")
        );

        let material = if search_path.is_empty() {
            material.to_string()
        } else {
            self.loader
                .find_in_paths(&material, &search_path)
                .unwrap_or(material.into())
        };

        let mut materials = self.materials.borrow_mut();

        match materials
            .iter()
            .enumerate()
            .find_map(|(i, name)| (*name == material).then_some(i))
        {
            Some(i) => i,
            None => {
                let i = materials.len();
                materials.push(material);
                i
            }
        }
    }

    pub fn into_materials(self) -> Vec<String> {
        self.materials.into_inner()
    }
}
