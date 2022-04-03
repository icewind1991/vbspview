use crate::DemoInfo;
use three_d::*;
use tracing::{debug, info};

pub trait Control {
    fn handle(
        &mut self,
        camera: &mut Camera,
        events: &mut [Event],
        elapsed_time: f64,
        accumulated_time: f64,
    ) -> ThreeDResult<bool>;
}

pub struct FirstPerson {
    control: CameraControl,
    speed: f32,
    keys: [bool; 4],
}

impl Control for FirstPerson {
    fn handle(
        &mut self,
        camera: &mut Camera,
        events: &mut [Event],
        _elapsed_time: f64,
        _accumulated_time: f64,
    ) -> ThreeDResult<bool> {
        let change = self.control.handle_events(camera, events)?;
        for event in events.iter_mut() {
            match event {
                Event::KeyPress { kind, .. } => {
                    self.key_press(kind);
                }
                Event::KeyRelease { kind, .. } => {
                    self.key_release(kind);
                }
                _ => {}
            };
        }

        if self.keys[0] {
            apply_camera_action(camera, CameraAction::Forward { speed: self.speed }, 1.0)?;
        }
        if self.keys[1] {
            apply_camera_action(camera, CameraAction::Forward { speed: self.speed }, -1.0)?;
        }
        if self.keys[2] {
            apply_camera_action(camera, CameraAction::Left { speed: self.speed }, 1.0)?;
        }
        if self.keys[3] {
            apply_camera_action(camera, CameraAction::Left { speed: self.speed }, -1.0)?;
        }

        Ok(self.keys.iter().fold(change, |change, key| change && *key))
    }
}

impl FirstPerson {
    pub fn new(speed: f32) -> Self {
        Self {
            control: CameraControl {
                left_drag_horizontal: CameraAction::Yaw {
                    speed: std::f32::consts::PI / 1800.0,
                },
                left_drag_vertical: CameraAction::Pitch {
                    speed: std::f32::consts::PI / 1800.0,
                },
                ..Default::default()
            },
            speed,
            keys: [false; 4],
        }
    }

    fn key_press(&mut self, key: &Key) {
        match key {
            Key::W => self.keys[0] = true,
            Key::S => self.keys[1] = true,
            Key::A => self.keys[2] = true,
            Key::D => self.keys[3] = true,
            _ => {}
        }
    }

    fn key_release(&mut self, key: &Key) {
        match key {
            Key::W => self.keys[0] = false,
            Key::S => self.keys[1] = false,
            Key::A => self.keys[2] = false,
            Key::D => self.keys[3] = false,
            _ => {}
        }
    }
}

pub struct DebugToggle {
    pub enabled: bool,
}

impl Control for DebugToggle {
    fn handle(
        &mut self,
        _camera: &mut Camera,
        events: &mut [Event],
        _elapsed_time: f64,
        _accumulated_time: f64,
    ) -> ThreeDResult<bool> {
        for event in events.iter_mut() {
            match event {
                Event::Text(text) => {
                    if text == "`" {
                        self.enabled = !self.enabled;
                        return Ok(true);
                    }
                }
                _ => {}
            };
        }

        Ok(false)
    }
}

impl DebugToggle {
    pub fn new() -> Self {
        DebugToggle { enabled: true }
    }
}

pub struct DemoCamera {
    demo: DemoInfo,
    playing: bool,
    start_tick: f64,
    playback_start_time: f64,
}

impl Control for DemoCamera {
    fn handle(
        &mut self,
        _camera: &mut Camera,
        events: &mut [Event],
        _elapsed_time: f64,
        accumulated_time: f64,
    ) -> ThreeDResult<bool> {
        let mut change = false;
        for event in events.iter_mut() {
            match event {
                Event::Text(text) => {
                    if text == "p" {
                        change = true;
                        self.playing = !self.playing;
                        dbg!(self.playing);
                        if self.playing {
                            self.playback_start_time = accumulated_time;
                        } else {
                            self.start_tick = self.tick(accumulated_time);
                        }
                    }
                }
                _ => {}
            };
        }

        if self.playing {
            let tick = self.tick(accumulated_time);
            if self.demo.positions.len() as f64 <= tick {
                self.playing = false;
                self.start_tick = self.tick(accumulated_time);
                change = true;
                info!(
                    tick = tick,
                    length = self.demo.positions.len(),
                    "end of demo"
                );
            } else {
                debug!(
                    tick = tick,
                    start_tick = self.start_tick,
                    play_time = accumulated_time - self.playback_start_time,
                    "playing tick"
                );
                // todo: interpolate
                let (position, yaw, pitch) = self.demo.positions[tick as usize];
                self.apply_view(_camera, position, yaw, pitch);
            }
        }

        Ok(self.playing | change)
    }
}

impl DemoCamera {
    pub fn new(demo: DemoInfo) -> Self {
        DemoCamera {
            demo,
            playing: false,
            start_tick: 0.0,
            playback_start_time: 0.0,
        }
    }

    fn tick(&self, time: f64) -> f64 {
        let playback_time = (time - self.playback_start_time) / 1000.0;
        self.start_tick + playback_time / self.demo.time_per_tick
    }

    fn apply_view(&self, camera: &mut Camera, position: Vec3, yaw: f32, pitch: f32) {
        let forward = vec4(0.0, 0.0, 1.0, 1.0);
        let angle_transform = Mat4::from_angle_y(degrees(yaw)) * Mat4::from_angle_x(degrees(pitch));
        let target = position + (angle_transform * forward).truncate();
        camera
            .set_view(position, target, vec3(0.0, 1.0, 0.0))
            .unwrap();
    }
}

fn apply_camera_action(
    camera: &mut Camera,
    control_type: CameraAction,
    x: f64,
) -> ThreeDResult<bool> {
    match control_type {
        CameraAction::Pitch { speed } => {
            camera.pitch(radians(speed * x as f32))?;
        }
        CameraAction::OrbitUp { speed, target } => {
            camera.rotate_around_with_fixed_up(&target, 0.0, speed * x as f32)?;
        }
        CameraAction::Yaw { speed } => {
            camera.yaw(radians(speed * x as f32))?;
        }
        CameraAction::OrbitLeft { speed, target } => {
            camera.rotate_around_with_fixed_up(&target, speed * x as f32, 0.0)?;
        }
        CameraAction::Roll { speed } => {
            camera.roll(radians(speed * x as f32))?;
        }
        CameraAction::Left { speed } => {
            let change = -camera.right_direction() * x as f32 * speed;
            camera.translate(&change)?;
        }
        CameraAction::Up { speed } => {
            let right = camera.right_direction();
            let up = right.cross(camera.view_direction());
            let change = up * x as f32 * speed;
            camera.translate(&change)?;
        }
        CameraAction::Forward { speed } => {
            let change = camera.view_direction() * speed * x as f32;
            camera.translate(&change)?;
        }
        CameraAction::Zoom {
            target,
            speed,
            min,
            max,
        } => {
            camera.zoom_towards(&target, speed * x as f32, min, max)?;
        }
        CameraAction::None => {}
    }
    Ok(control_type != CameraAction::None)
}
