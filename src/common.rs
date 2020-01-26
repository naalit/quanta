pub use na::{Point3, Vector3};
pub use nalgebra as na;
pub use num_traits::Zero;
use std::sync::mpsc::*;
pub use vulkano::half::prelude::*;
pub use crate::material::Material;
pub use crate::octree::*;
pub use std::collections::HashMap;

pub const CHUNK_SIZE: f32 = 16.0;

pub const REGION_SIZE: i32 = 4;

pub fn radians(degrees: f32) -> f32 {
    std::f32::consts::PI / 180.0 * degrees
}

// These functions define the coordinate system of the world

/// Returns the center of a chunk
pub fn chunk_to_world(chunk: Vector3<i32>) -> Vector3<f32> {
    chunk.map(|x| (x as f32 + 0.5) * CHUNK_SIZE)
}
pub fn world_to_chunk(world: Vector3<f32>) -> Vector3<i32> {
    world.map(|x| (x / CHUNK_SIZE).floor() as i32)
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
    Submit(
        vulkano::command_buffer::AutoCommandBuffer,
        Vector3<f32>,
        f32,
        HashMap<Vector3<i32>, (usize, usize)>,
    ),
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
