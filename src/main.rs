mod bsp;
mod control;
mod demo;
mod material;
mod prop;
mod renderer;
mod ui;
mod wrapping;

use clap::Parser;
use std::fs;
use std::string::FromUtf8Error;
use tf_asset_loader::{Loader, LoaderError};

use crate::bsp::load_map;
use crate::control::{Control, DemoCamera};
use crate::demo::DemoInfo;
use crate::renderer::Renderer;
use crate::ui::DebugUI;
use control::FirstPerson;
use thiserror::Error;
use three_d::*;
use tracing_subscriber::{prelude::*, EnvFilter};
use tracing_tree::HierarchicalLayer;
use vmt_parser::VdfError;

/// View a demo file
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path of the demo or map file
    path: String,
    /// Name of the player to follow, when using a demo file
    player: Option<String>,
    /// Disable loading and showing props in the map
    #[arg(long)]
    no_props: bool,
    /// Disable loading of textures
    #[arg(long)]
    no_textures: bool,
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
    Vtf(#[from] vtf::Error),
    #[error(transparent)]
    Vdf(#[from] VdfError),
    #[error(transparent)]
    Mdl(#[from] vmdl::ModelError),
    #[error(transparent)]
    Demo(#[from] tf_demo_parser::ParseError),
    #[error("{0}")]
    Other(String),
    #[error(transparent)]
    Window(#[from] WindowError),
    #[error(transparent)]
    Render(#[from] RendererError),
    #[error(transparent)]
    String(#[from] FromUtf8Error),
    #[error(transparent)]
    Loader(#[from] LoaderError),
    #[error("resource {0} not found in vpks or pack")]
    ResourceNotFound(String),
}

impl From<&'static str> for Error {
    fn from(e: &'static str) -> Self {
        Error::Other(e.into())
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

    if args.path.ends_with(".dem") {
        let demo = DemoInfo::new(args.path, &args.player.unwrap_or_default())?;
        let mut loader = Loader::new()?;
        let map = loader
            .load(&format!("maps/{}.bsp", demo.map))?
            .ok_or(Error::ResourceNotFound(demo.map.clone()))?;

        let models = load_map(&map, &mut loader, !args.no_props, !args.no_textures)?;
        play(window, DemoCamera::new(demo), models)
    } else {
        let mut loader = Loader::new()?;
        let map = fs::read(args.path)?;

        let models = load_map(&map, &mut loader, !args.no_props, !args.no_textures)?;
        play(window, FirstPerson::new(0.1), models)
    }
}

fn play<C: Control + 'static>(
    window: Window,
    control: C,
    models: Vec<CpuModel>,
) -> Result<(), Error> {
    let mut renderer = Renderer::new(&window, control);

    renderer.models = models
        .into_iter()
        .map(|model| Model::new(&renderer.context, &model))
        .collect::<Result<_, _>>()?;

    window.render_loop(move |frame_input| renderer.render(frame_input));

    Ok(())
}
