use crate::common::*;
use crate::event::*;
use crate::shaders::PushConstants;

/// In m/s
pub const MOVE_SPEED: f32 = 10.0;

pub struct Camera {
    fov: f32,
    resolution: (f64, f64),
    pub pos: Point3<f32>,
    pub start: Vector3<i32>,
    dir: Vector3<f32>,
    up: Vector3<f32>,
    rx: f64,
    ry: f64,
    moving: Vector3<f32>, // vec3(right, up, forward)
}

impl Camera {
    pub fn new(resolution: (f64, f64)) -> Self {
        let fov = radians(90.0);
        let pos = Point3::from(na::Vector3::new(1.0, 1.0, 1.0));
        let dir = Vector3::z();
        let up = Vector3::y();

        Camera {
            fov,
            resolution,
            pos,
            start: [-8; 3].into(),
            dir,
            up,
            rx: 0.0,
            ry: 0.0,
            moving: Vector3::zeros(),
        }
    }

    pub fn update(&mut self, delta: f64) {
        // self.up is the CAMERA up, but jumping moves up in the WORLD
        let up = Vector3::y();
        self.pos += self.dir * self.moving.z * delta as f32 * MOVE_SPEED;
        self.pos += up * self.moving.y * delta as f32 * MOVE_SPEED;
        self.pos += self.dir.cross(&up).normalize() * self.moving.x * delta as f32 * MOVE_SPEED;
    }

    pub fn push(&self) -> PushConstants {
        PushConstants {
            fov: self.fov,
            resolution: [self.resolution.0 as f32, self.resolution.1 as f32],
            camera_pos: [self.pos.x, self.pos.y, self.pos.z],
            camera_dir: self.dir.into(),
            camera_up: self.up.into(),
            start: self.start.into(),
            _dummy0: [0; 4],
            _dummy1: [0; 4],
            _dummy2: [0; 4],
            _dummy3: [0; 4],
        }
    }

    pub fn process(&mut self, event: &Event) {
        match event {
            // /*w*/ my layout
            Event::KeyPressed(/*0x11*/ 52) => {
                self.moving.z = 1.0;
            }
            // s
            Event::KeyPressed(/*0x1f*/ 18) => {
                self.moving.z = -1.0;
            }
            Event::KeyReleased(/*0x11*/ 52) | Event::KeyReleased(/*0x1f*/ 18) => {
                self.moving.z = 0.0;
            }
            // a
            Event::KeyPressed(/*0x1e*/ 24) => {
                self.moving.x = -1.0;
            }
            // d
            Event::KeyPressed(/*0x20*/ 22) => {
                self.moving.x = 1.0;
            }
            Event::KeyReleased(/*0x1e*/ 24) | Event::KeyReleased(/*0x20*/ 22) => {
                self.moving.x = 0.0;
            }
            Event::Mouse(x, y) => {
                self.rx -= x / self.resolution.0;
                self.ry += y / self.resolution.1;
                self.ry = na::clamp(
                    self.ry,
                    0.01 - std::f64::consts::FRAC_PI_2,
                    -0.01 + std::f64::consts::FRAC_PI_2,
                );
                self.dir = na::UnitQuaternion::from_axis_angle(
                    &na::Unit::new_unchecked(na::Vector3::y()),
                    self.rx as f32,
                ) * na::UnitQuaternion::from_axis_angle(
                    &na::Unit::new_unchecked(na::Vector3::x()),
                    self.ry as f32,
                ) * na::Vector3::z();
                self.up = na::UnitQuaternion::from_axis_angle(
                    &na::Unit::new_unchecked(na::Vector3::y()),
                    self.rx as f32,
                ) * na::UnitQuaternion::from_axis_angle(
                    &na::Unit::new_unchecked(na::Vector3::x()),
                    self.ry as f32,
                ) * na::Vector3::y();
            }
            Event::Resize(x, y) => {
                self.resolution = (*x, *y);
            }
            _ => {}
        }
    }
}
