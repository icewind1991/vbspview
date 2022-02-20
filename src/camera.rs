use three_d::*;

pub struct FirstPerson {
    control: CameraControl,
    speed: f32,
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
        }
    }

    pub fn handle_events(
        &mut self,
        camera: &mut Camera,
        events: &mut [Event],
    ) -> ThreeDResult<bool> {
        let mut change = self.control.handle_events(camera, events)?;
        for event in events.iter_mut() {
            change |= match event {
                Event::KeyPress { kind, handled, .. } => {
                    if let Some((action, x)) = self.key_to_action(kind) {
                        *handled = true;
                        self.handle_action(camera, action, x)?;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            };
        }
        Ok(change)
    }

    fn key_to_action(&self, key: &Key) -> Option<(CameraAction, f64)> {
        match key {
            Key::W => Some((CameraAction::Forward { speed: self.speed }, 1.0)),
            Key::S => Some((CameraAction::Forward { speed: self.speed }, -1.0)),
            Key::A => Some((CameraAction::Left { speed: self.speed }, 1.0)),
            Key::D => Some((CameraAction::Left { speed: self.speed }, -1.0)),
            _ => None,
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
