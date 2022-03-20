mod bsp;
mod camera;
mod loader;
mod renderer;
mod ui;

use crate::bsp::load_map;
use crate::renderer::Renderer;
use crate::ui::DebugUI;
use camera::FirstPerson;
use loader::Loader;
use std::env::args;
use thiserror::Error;
use three_d::*;
use tracing_subscriber::{prelude::*, EnvFilter};
use tracing_tree::HierarchicalLayer;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Three(#[from] Box<dyn std::error::Error>),
    #[error(transparent)]
    Bsp(#[from] vbsp::BspError),
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    Vpk(#[from] vpk::Error),
    #[error(transparent)]
    Mdl(#[from] vmdl::ModelError),
    #[error("{0}")]
    Other(&'static str),
}

impl From<&'static str> for Error {
    fn from(e: &'static str) -> Self {
        Error::Other(e)
    }
}

fn setup() {
    miette::set_panic_hook();

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(
            HierarchicalLayer::new(2)
                .with_targets(true)
                .with_bracketed_fields(true),
        )
        .init();
}

fn main() -> Result<(), Error> {
    setup();

    let mut args = args();
    let bin = args.next().unwrap();
    let file = match args.next() {
        Some(file) => file,
        None => {
            eprintln!("usage: {} <file.bsp>", bin);
            return Ok(());
        }
    };

    let window = Window::new(WindowSettings {
        title: file.clone(),
        max_size: Some((1920, 1080)),
        ..Default::default()
    })?;

    let mut renderer = Renderer::new(&window)?;

    let map = std::fs::read(&file)?;
    let meshes = load_map(&map)?;
    let material = PhysicalMaterial {
        albedo: Color {
            r: 128,
            g: 128,
            b: 128,
            a: 255,
        },
        ..Default::default()
    };

    renderer.models = meshes
        .into_iter()
        .map(|mesh| Model::new_with_material(&renderer.context, &mesh, material.clone()))
        .collect::<Result<_, _>>()?;

    window.render_loop(move |frame_input| renderer.render(frame_input).unwrap())?;

    Ok(())
}
