mod bsp;
mod camera;
mod demo;
mod loader;
mod renderer;
mod ui;
use clap::Parser;

use crate::bsp::load_map;
use crate::demo::DemoInfo;
use crate::renderer::Renderer;
use crate::ui::DebugUI;
use camera::FirstPerson;
use loader::Loader;
use thiserror::Error;
use three_d::*;
use tracing_subscriber::{prelude::*, EnvFilter};
use tracing_tree::HierarchicalLayer;

/// View a demo file
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path of the demo file
    demo: String,
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
        title: args.demo.clone(),
        max_size: Some((1920, 1080)),
        ..Default::default()
    })?;

    let demo = DemoInfo::new(args.demo, &args.player)?;
    let mut loader = Loader::new()?;
    let map = loader.load(&format!("maps/{}.bsp", demo.map))?;

    let mut renderer = Renderer::new(&window)?;

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

    let mut positions = demo.positions.into_iter();
    let forward = vec4(0.0, 0.0, 1.0, 1.0);

    window.render_loop(move |frame_input| {
        if let Some((position, angle, pitch)) = positions.next() {
            let angle_transform =
                Mat4::from_angle_y(degrees(angle)) * Mat4::from_angle_x(degrees(pitch));
            let target = position + (angle_transform * forward).truncate();
            renderer
                .camera
                .set_view(position, target, vec3(0.0, 1.0, 0.0))
                .unwrap();
        }
        renderer.render(frame_input).unwrap()
    })?;

    Ok(())
}
