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
    // pub fn block(&self, k: Vector3<f32>) -> Option<Material> {
    //     let chunk = world_to_chunk(k);
    //     let in_chunk = in_chunk(k);
    //     let chunk = self.chunks.get(&chunk)?;
    //     Some(chunk.block(in_chunk))
    // }
    // pub fn set_block(&mut self, k: Vector3<f32>, v: Material) {
    //     let chunk = world_to_chunk(k);
    //     let in_chunk = in_chunk(k);
    //     let chunk = self.chunks.get_mut(&chunk).unwrap();
    //     chunk.set_block(in_chunk, v);
    // }
}

impl Extend<(Vector3<i32>, Chunk)> for World {
    fn extend<T: IntoIterator<Item = (Vector3<i32>, Chunk)>>(&mut self, it: T) {
        self.chunks.extend(it);
    }
}
