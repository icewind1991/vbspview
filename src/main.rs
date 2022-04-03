mod bsp;
mod control;
mod demo;
mod loader;
mod renderer;
mod ui;
use clap::Parser;

use crate::bsp::load_map;
use crate::control::DemoCamera;
use crate::demo::DemoInfo;
use crate::renderer::Renderer;
use crate::ui::DebugUI;
use control::FirstPerson;
use loader::Loader;
use thiserror::Error;
use three_d::*;
use tracing_subscriber::{prelude::*, EnvFilter};
use tracing_tree::HierarchicalLayer;

/// View a demo file
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path of the demo or map file
    path: String,
    /// Name of the player to follow
    player: String,
}

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
    #[error(transparent)]
    Demo(#[from] tf_demo_parser::ParseError),
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

    let args = Args::parse();

    let window = Window::new(WindowSettings {
        title: args.path.clone(),
        max_size: Some((1920, 1080)),
        ..Default::default()
    })?;

    let demo = DemoInfo::new(args.path, &args.player)?;
    let mut loader = Loader::new()?;
    let map = loader.load(&format!("maps/{}.bsp", demo.map))?;

    let mut renderer = Renderer::new(&window, DemoCamera::new(demo))?;

    let meshes = load_map(&map, &mut loader)?;
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
