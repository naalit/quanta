use crate::common::*;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub struct World {
    pub chunks: HashMap<Vector3<i32>, Chunk>,
}

pub type ArcWorld = Arc<RwLock<World>>;
pub fn arcworld() -> ArcWorld {
    Arc::new(RwLock::new(World::new()))
}

impl World {
    pub fn new() -> Self {
        World {
            chunks: HashMap::new(),
        }
    }

    pub fn contains_chunk(&self, chunk: Vector3<i32>) -> bool {
        self.chunks.contains_key(&chunk)
    }

    pub fn locs(&self) -> std::collections::hash_map::Keys<'_, Vector3<i32>, Chunk> {
        self.chunks.keys()
    }

    pub fn chunk(&self, k: Vector3<i32>) -> Option<&Chunk> {
        self.chunks.get(&k)
    }
    pub fn add_chunk(&mut self, k: Vector3<i32>, v: Chunk) {
        self.chunks.insert(k, v);
    }
    pub fn remove_chunk(&mut self, k: Vector3<i32>) -> Option<Chunk> {
        self.chunks.remove(&k)
    }

    pub fn block(&self, k: Vector3<f32>) -> Option<Material> {
        let chunk = world_to_chunk(k);
        let in_chunk = k - chunk_to_world(chunk);
        let chunk = self.chunks.get(&chunk)?;
        Some(chunk.block(in_chunk))
    }
    pub fn set_block(&mut self, k: Vector3<f32>, v: Material) {
        let chunk = world_to_chunk(k);
        let in_chunk = k - chunk_to_world(chunk);
        let chunk = self.chunks.get_mut(&chunk).unwrap();
        chunk.set_block(in_chunk, CHUNK_SIZE.log2().ceil() as u32, v);
    }

    pub fn raycast(&self, ro: Vector3<f32>, rd: Vector3<f32>, max_t: f32) -> Option<RayCast> {
        // Adapted from _A Fast Voxel Traversal Algorithm for Ray Tracing_ by Amanatides and Woo
        // Basically DDA
        let mut pos = world_to_chunk(ro);
        // rd is m/t
        // CHUNK_SIZE is m/chunk
        // t/chunk
        let tdelta = rd.map(|x| CHUNK_SIZE / x).abs();
        let tstep = rd.map(|x| x.signum() as i32);
        // t
        let mut tmax = (chunk_to_world(pos) + rd.map(f32::signum) * CHUNK_SIZE * 0.5
            - ro
            - Vector3::repeat(CHUNK_SIZE * 0.5))
        .zip_map(&rd, |p, r| p / r);

        loop {
            let chunk = self.chunk(pos)?;
            if chunk[0..8] != [0; 8] {
                if let Some(x) = chunk.raycast(
                    ro - chunk_to_world(pos) + Vector3::repeat(CHUNK_SIZE * 0.5),
                    rd,
                    64,
                ) {
                    if x.t[0] > max_t {
                        return None;
                    }
                    return Some(RayCast {
                        pos: chunk_to_world(pos) + x.pos,
                        ..x
                    });
                }
            }

            if tmax.min() > max_t {
                return None;
            }

            if tmax.x < tmax.y {
                if tmax.x < tmax.z {
                    tmax.x += tdelta.x;
                    pos.x += tstep.x;
                } else {
                    tmax.z += tdelta.z;
                    pos.z += tstep.z;
                }
            } else if tmax.y < tmax.z {
                tmax.y += tdelta.y;
                pos.y += tstep.y;
            } else {
                tmax.z += tdelta.z;
                pos.z += tstep.z;
            }
        }
    }
}

impl Extend<(Vector3<i32>, Chunk)> for World {
    fn extend<T: IntoIterator<Item = (Vector3<i32>, Chunk)>>(&mut self, it: T) {
        self.chunks.extend(it);
    }
}
