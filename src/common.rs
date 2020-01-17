pub use na::{Point3, Vector3};
pub use nalgebra as na;
pub use vulkano::half::prelude::*;

pub fn radians(degrees: f32) -> f32 {
    std::f32::consts::PI / 180.0 * degrees
}

pub fn world_to_chunk(w: Vector3<f32>) -> Vector3<i32> {
    let a = w.map(|w| if w < 0.0 { 1 } else { 0 });
    w.map(|x| x as i32) / 14 - a
}
pub fn chunk_to_world(c: Vector3<i32>) -> Vector3<f32> {
    c.map(|x| x as f32 + 0.5) * 14.0
}
pub fn pos_in_chunk(w: Vector3<f32>) -> Vector3<f32> {
    w.map(|x| ((x % 14.0) + 14.0) % 14.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversion_recip() {
        let v = Vector3::new(-23.0, 3.0, -5.0);
        println!("{:?}", world_to_chunk(v));
        println!("{:?}", chunk_to_world(world_to_chunk(v)));
        assert!(
            (v - chunk_to_world(world_to_chunk(v))).norm() < 14.0,
            "Difference was {}",
            (v - chunk_to_world(world_to_chunk(v))).norm()
        );
    }
}
