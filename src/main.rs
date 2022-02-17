use miette::Diagnostic;
use std::env::args;
use std::fs;
use thiserror::Error;
use three_d::*;
use vbsp::{Bsp, Handle};

#[derive(Debug, Error, Diagnostic)]
enum Error {
    #[error(transparent)]
    Three(#[from] Box<dyn std::error::Error>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Bsp(#[from] vbsp::BspError),
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error("{0}")]
    Other(&'static str),
}

fn main() -> Result<(), Error> {
    miette::set_panic_hook();

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
        max_size: Some((1280, 720)),
        ..Default::default()
    })?;

    let data = fs::read(&file)?;
    let bsp = Bsp::read(&data)?;
    let world_model = bsp.models().next().ok_or(Error::Other("No world model"))?;

    let context = window.gl().unwrap();

    let mut camera = Camera::new_perspective(
        &context,
        window.viewport().unwrap(),
        vec3(0.0, 0.0, 2.0),
        vec3(0.0, 0.0, 0.0),
        vec3(0.0, 1.0, 0.0),
        degrees(45.0),
        0.1,
        10.0,
    )?;

    let cpu_mesh = model_to_mesh(world_model);

    // Construct a model, with a default color material, thereby transferring the mesh data to the GPU
    let mut model = Model::new(&context, &cpu_mesh)?;

    // Start the main render loop
    window.render_loop(move |frame_input: FrameInput| // Begin a new frame with an updated frame input
        {
            // Ensure the viewport matches the current window viewport which changes if the window is resized
            camera.set_viewport(frame_input.viewport).unwrap();

            // Start writing to the screen and clears the color and depth
            Screen::write(&context, ClearState::color_and_depth(0.8, 0.8, 0.8, 1.0, 1.0), || {
                // Set the current transformation of the triangle
                model.set_transformation(Mat4::from_angle_y(radians((frame_input.accumulated_time * 0.005) as f32)));

                // Render the triangle with the color material which uses the per vertex colors defined at construction
                model.render(&camera, &Lights::default())?;
                Ok(())
            }).unwrap();

            FrameOutput::default()
        })?;
    Ok(())
}

fn model_to_mesh(model: Handle<vbsp::data::Model>) -> CPUMesh {
    let size = [
        model.maxs.z - model.mins.z,
        model.maxs.y - model.mins.y,
        model.maxs.x - model.mins.x,
    ]
    .into_iter()
    .max_by(|a, b| a.partial_cmp(b).unwrap())
    .unwrap();
    let positions: Vec<f32> = model
        .faces()
        .filter(|face| face.is_visible())
        .flat_map(|face| face.triangulate())
        .flat_map(|triangle| triangle.into_iter())
        .flat_map(|vertex| vertex.iter())
        .map(|c| c / size)
        .collect();

    CPUMesh {
        positions,
        ..Default::default()
    }
}
