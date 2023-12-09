mod bsp;
mod control;
mod demo;
mod loader;
mod renderer;
mod ui;
mod wrapping;

use clap::Parser;
use std::fs;

use crate::bsp::load_map;
use crate::control::{Control, DemoCamera};
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
    player: Option<String>,
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
    #[error(transparent)]
    Window(#[from] WindowError),
    #[error(transparent)]
    Render(#[from] RendererError),
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

    if args.path.ends_with(".dem") {
        let demo = DemoInfo::new(args.path, &args.player.unwrap_or_default())?;
        let mut loader = Loader::new()?;
        let map = loader.load(&format!("maps/{}.bsp", demo.map))?;

        let models = load_map(&map, &mut loader)?;
        play(window, DemoCamera::new(demo), models)
    } else {
        let mut loader = Loader::new()?;
        let map = fs::read(args.path)?;

        let models = load_map(&map, &mut loader)?;
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
