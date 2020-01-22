use std::sync::mpsc::*;
pub use na::{Point3, Vector3};
pub use nalgebra as na;
pub use vulkano::half::prelude::*;
pub use num_traits::Zero;

pub const CHUNK_SIZE: f32 = 16.0;

pub const REGION_SIZE: i32 = 2;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Chunk(pub Vec<u32>);

use std::ops::{Deref, DerefMut};
impl Deref for Chunk {
    type Target = Vec<u32>;
    fn deref(&self) -> &Vec<u32> {
        &self.0
    }
}
impl DerefMut for Chunk {
    fn deref_mut(&mut self) -> &mut Vec<u32> {
        &mut self.0
    }
}

impl Chunk {
    pub fn empty() -> Self {
        Chunk(vec![0; 8])
    }

    pub fn from_dist(mut dist: impl FnMut(Vector3<f32>) -> f32) -> Self {
        struct ST {
            parent: usize,
            idx: Vector3<f32>,
            pos: Vector3<f32>,
            scale: i32,
        }

        let levels = CHUNK_SIZE.log2() as i32 - 1;
        let mut stack: Vec<ST> = vec![];
        let d_corner = 0.75_f32.sqrt();

        let mut tree: Vec<u32> = Vec::new();
        for i in 0.. {
            let (pos, root, idx, parent, scale) =
                if i == 0 { (Vector3::repeat(CHUNK_SIZE * 0.5), true, Vector3::zeros(), 0, 0) }
                else if !stack.is_empty() { let s = stack.pop().unwrap(); (s.pos, false, s.idx, s.parent, s.scale) }
                else { break };

            let mut v = vec![0; 8];
            let size = 2.0_f32.powf(-scale as f32) * CHUNK_SIZE * 0.5; // Next level's size
            for j in 0..8 {
                let jdx = idx_to_pos(j);
                let np = pos + jdx * size * 0.5;

                let d = dist(np);
                if scale >= levels {
                    if d > size * d_corner {
                        v[j] = 0;
                    } else {
                        v[j] = 0b10;
                    }
                } else if d > size * d_corner {
                    //v.leaf[j] = true;
                    v[j] = 0;
                } else if d < -size * d_corner {
                    //v.leaf[j] = true;
                    v[j] = 0b10;
                } else {
                    stack.push(ST{parent: i*8, idx: jdx, pos: np, scale: scale+1 });
                }
            }
            if !root {
                let uidx = pos_to_idx(idx);
                tree[parent + uidx] = (((i*8 - parent) as u32) << 1) | 1;
            }
            tree.append(&mut v);
        }
        Chunk(tree)
    }
}


/// Converts between a 3D vector representing the child slot, and the actual index into the `pointer` array
pub fn pos_to_idx<T: na::Scalar + Zero + PartialOrd>(idx: Vector3<T>) -> usize {
    // Once again, this function closely mirrors the GLSL one for testing
    let mut ret = 0;
    ret |= usize::from(idx.x > T::zero()) << 2;
    ret |= usize::from(idx.y > T::zero()) << 1;
    ret |= usize::from(idx.z > T::zero());
    ret
}

/// Converts between a 3D vector representing the child slot, and the actual index into the `pointer` array
pub fn idx_to_pos(idx: usize) -> Vector3<f32> {
    Vector3::new(
        if idx & (1 << 2) > 0 { 1.0 } else { -1.0 },
        if idx & (1 << 1) > 0 { 1.0 } else { -1.0 },
        if idx & 1 > 0 { 1.0 } else { -1.0 },
    )
}


pub fn neighbors(idx: Vector3<i32>) -> Vec<Vector3<i32>> {
    [
        -Vector3::x(),
        Vector3::x(),
        -Vector3::y(),
        Vector3::y(),
        -Vector3::z(),
        Vector3::z(),
    ]
    .iter()
    .map(|x| idx + x)
    .collect()
}


pub fn radians(degrees: f32) -> f32 {
    std::f32::consts::PI / 180.0 * degrees
}

// These functions define the coordinate system of the world

/// Returns the center of a chunk
pub fn chunk_to_world(chunk: Vector3<i32>) -> Vector3<f32> {
    chunk.map(|x| (x as f32 - 0.5) * CHUNK_SIZE)
}
pub fn world_to_chunk(world: Vector3<f32>) -> Vector3<i32> {
    world.map(|x| (x / CHUNK_SIZE) as i32 + 1)
}

pub fn region_to_chunk(chunk: Vector3<i32>) -> Vector3<i32> {
    chunk.map(|x| x * REGION_SIZE)
}
pub fn chunk_to_region(world: Vector3<i32>) -> Vector3<i32> {
    world.map(|x| (x + REGION_SIZE / 2) / REGION_SIZE)
}
pub fn in_region(chunk: Vector3<i32>) -> usize {
    let v = chunk.map(|x| ((x % REGION_SIZE) + REGION_SIZE) as usize % REGION_SIZE as usize);
    v.x + v.y * REGION_SIZE as usize + v.z * REGION_SIZE as usize * REGION_SIZE as usize
}


pub enum Connection {
    Local(Sender<Message>, Receiver<Message>),
    // TODO some sort of buffered TCP stream inplementation of Connection
}

impl Connection {
    /// Create a two new Local connections - (client, server)
    pub fn local() -> (Connection, Connection) {
        let (cto, sfrom) = channel();
        let (sto, cfrom) = channel();
        let client = Connection::Local(cto, cfrom);
        let server = Connection::Local(sto, sfrom);
        (client, server)
    }

    /// Equivalent to Sender::send() but as an option
    pub fn send(&self, m: Message) -> Option<()> {
        match self {
            Connection::Local(to, _from) => to.send(m).ok(),
        }
    }

    /// Equivalent to Receiver::try_recv() but as an option - doesn't block
    pub fn recv(&self) -> Option<Message> {
        match self {
            Connection::Local(_to, from) => from.try_recv().ok(),
        }
    }
}

#[derive(Debug)]
pub enum Message {
    PlayerMove(Vector3<f32>),
    Chunks(Vec<(Vector3<i32>, Chunk)>),
    //SetBlock(Vector3<i32>, Material),
    Leave,
}

#[derive(Debug)]
pub enum ChunkMessage {
    Done,
    UpdateChunks(Vec<Vector3<i32>>),
    LoadChunks(Vec<Vector3<i32>>),
    // Chunks(Vec<(Vector3<i32>, Chunk)>),
    UnloadChunk(Vector3<i32>, Chunk),
    Players(Vec<Vector3<f32>>),
}

pub enum ClientMessage {
    Done,
    PlayerMove(Vector3<f32>),
    /// CommandBuffer, origin, root_size
    Submit(vulkano::command_buffer::AutoCommandBuffer, Vector3<f32>, f32),
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
