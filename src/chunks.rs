/// A background thread that loads chunks and sends them to the client thread
use std::sync::mpsc::*;
use crate::common::*;

pub struct ChunkUpdate {
    pub start: Vector3<i32>,
    pub chunks: Vec<[u16; 4]>,
    pub blocks: Vec<([u32; 3], Vec<u8>)>,
}

pub struct ChunkThread {
    ch: (Sender<ChunkUpdate>, Receiver<Vector3<i32>>),
}
impl ChunkThread {
    pub fn new(send: Sender<ChunkUpdate>, recv: Receiver<Vector3<i32>>) -> Self {
        ChunkThread {
            ch: (send, recv),
        }
    }

    pub fn run(self, mut start: Vector3<i32>, mut last_chunk: Vector3<i32>, mut chunks: Vec<[u16; 4]>) {
        while let Ok(chunk) = self.ch.1.recv() {
            let dif = chunk - last_chunk;
            let i = dif.iamax();
            let mut s = Vector3::zeros();
            s[i] = dif[i];
            println!("Loading chunks in direction {:?}", s);
            let new_start = start + s;
            let mut new_chunks = chunks.clone();

            let mut blocks = Vec::new();

            for x in 0..16 {
                for y in 0..16 {
                    for z in 0..16 {
                        let v = Vector3::new(x, y, z);
                        let world_pos = new_start + v;

                        // The index of this chunk in the old chunk index
                        let old_v = v + s;

                        if old_v.min() >= 0 && old_v.max() < 16 {
                            // It's already in blocks, so just change the offset
                            new_chunks[(x + y * 16 + z * 16 * 16) as usize] =
                                chunks[(old_v.x + 16 * old_v.y + 16 * 16 * old_v.z) as usize];
                        } else {
                            // It's out of bounds, we need to make a new chunk and delete the old one

                            // Wrap the coordinates around. If it's `-1`, this will be `15`;
                            //  if it's `16`, this will be `32 % 16 = 0`.
                            //  And if it's something else, it won't change
                            let old_v = old_v.map(|x| (x + 16) % 16);

                            // A now-unnocupied chunk
                            let slot =
                                chunks[(old_v.x + 16 * old_v.y + 16 * 16 * old_v.z) as usize];
                            new_chunks[(x + y * 16 + z * 16 * 16) as usize] = slot;

                            // Generate a new chunk and add it to blocks
                            let c = crate::gen::gen_chunk(world_pos.zyx());
                            blocks.push((
                                // - 1 to compensate for the lip
                                [slot[0] as u32 - 1, slot[1] as u32 - 1, slot[2] as u32 - 1],
                                c,
                            ));
                        }
                    }
                }
            }
            println!("Loaded chunks, sending to client");
            chunks = new_chunks.clone();

            let update = ChunkUpdate {
                start: new_start,
                chunks: new_chunks,
                blocks,
            };
            self.ch.0.send(update).unwrap();

            start = new_start;
            last_chunk = chunk;
        }
    }
}
