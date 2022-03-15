use three_d::*;

pub struct FirstPerson {
    control: CameraControl,
    speed: f32,
    keys: [bool; 4],
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

    pub fn handle_events(
        &mut self,
        camera: &mut Camera,
        events: &mut [Event],
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
            self.handle_action(camera, CameraAction::Forward { speed: self.speed }, 1.0)?;
        }
        if self.keys[1] {
            self.handle_action(camera, CameraAction::Forward { speed: self.speed }, -1.0)?;
        }
        if self.keys[2] {
            self.handle_action(camera, CameraAction::Left { speed: self.speed }, 1.0)?;
        }
        if self.keys[3] {
            self.handle_action(camera, CameraAction::Left { speed: self.speed }, -1.0)?;
        }

        Ok(self.keys.iter().fold(change, |change, key| change && *key))
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

    fn handle_action(
        &mut self,
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
}
