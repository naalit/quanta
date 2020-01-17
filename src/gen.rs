/// Temporary generator
use crate::common::*;
use noise::{MultiFractal, NoiseFn, Seedable};

/// indexed as chunk[x + y * 16 + z * 256]
/// because that's what Vulkan uses
pub fn gen_chunk(pos: Vector3<i32>) -> Vec<u8> {
    let noise = noise::HybridMulti::new()
        .set_seed(1)
        .set_octaves(8)
        .set_persistence(0.5);

    let mid = chunk_to_world(pos);
    let start = mid.map(|x| x - 7.0);

    (0..16)
        .flat_map(|x| (0..16).map(move |y| (x, y)))
        .flat_map(|(x, y)| (0..16).map(move |z| (x, y, z)))
        .map(|(x, y, z)| {
            let p = start + Vector3::new(x as f32 - 1.0, y as f32 - 1.0, z as f32 - 1.0);
            let h = -1.0 + 7.0 * noise.get([p.x as f64 * 0.004, p.z as f64 * 0.004]) as f32;

            let f = (p.y - h) * 0.6;
            let f = (f / 14.0).min(1.0).max(0.0) * 255.0;
            f as u8
        })
        .collect()
}
