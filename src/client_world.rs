use crate::common::*;
use crate::config::*;
use crate::world::*;
use std::collections::HashMap;
use std::sync::mpsc::*;
use std::sync::Arc;
use vulkano::command_buffer::{AutoCommandBuffer, AutoCommandBufferBuilder};

pub struct ClientWorld {
    conn: Connection,
    client: (Sender<ClientMessage>, Receiver<ClientMessage>),
    device: Arc<vulkano::device::Device>,
    queue: Arc<vulkano::device::Queue>,
    origin: Vector3<f32>,
    player: Vector3<f32>,
    pub root_size: f32,
    pub root: Vec<u32>, // The root structure. Points to chunks, gets buffer in the map
    world: ArcWorld,
    pub map: HashMap<Vector3<i32>, (usize, usize)>, // (start, end)
    spaces: Vec<(usize, usize)>,                    // (start, end)
    pub tree_buffer: Arc<vulkano::buffer::DeviceLocalBuffer<[u32]>>,
    upload: vulkano::buffer::CpuBufferPool<u32>,
    config: Arc<ClientConfig>,
}

impl ClientWorld {
    pub fn new(
        device: Arc<vulkano::device::Device>,
        queue: Arc<vulkano::device::Queue>,
        client: (Sender<ClientMessage>, Receiver<ClientMessage>),
        conn: Connection,
        player: Vector3<f32>,
        world: ArcWorld,
        config: Arc<ClientConfig>,
    ) -> Self {
        let start_len = 3_200_000; // = 12 MB
        let mut max_root_size = config.game_config.draw_chunks.pow(3);
        let mut last = max_root_size * 8;
        while last > 0 {
            last /= 8;
            max_root_size += last;
        }
        println!("Max root size = {}", max_root_size);

        ClientWorld {
            conn,
            client,
            device: device.clone(),
            queue,
            origin: player.map(|x| x % CHUNK_SIZE),
            player,
            root_size: 8.0, //CHUNK_NUM.max() as f32 * CHUNK_SIZE,
            root: vec![0; 8],
            world,
            map: HashMap::new(),
            spaces: vec![(max_root_size as usize * 8, start_len)],
            tree_buffer: vulkano::buffer::DeviceLocalBuffer::array(
                device.clone(),
                start_len,
                vulkano::buffer::BufferUsage {
                    storage_buffer: true,
                    // This actually shouldn't have to be set, this is a bug in vulkano: https://github.com/vulkano-rs/vulkano/issues/1283
                    uniform_buffer: true,
                    transfer_destination: true,
                    ..vulkano::buffer::BufferUsage::none()
                },
                device.active_queue_families(),
            )
            .unwrap(),
            upload: vulkano::buffer::CpuBufferPool::upload(device.clone()),
            config,
        }
    }

    pub fn run(mut self) {
        while let Ok(m) = self.client.1.recv() {
            match m {
                ClientMessage::PlayerMove(x) => self.update(x),
                ClientMessage::Done => {
                    self.conn
                        .send(Message::Leave)
                        .expect("Disconnected from server");
                    loop {
                        if let Some(Message::Leave) = self.conn.recv() {
                            break;
                        }
                    }
                    self.client.0.send(ClientMessage::Done).unwrap();
                    break;
                }
                _ => panic!("Unknown message from client thread"),
            }
        }
    }

    pub fn update(&mut self, player: Vector3<f32>) {
        self.player = player;
        if let Some(m) = self.conn.recv() {
            // Only load chunks once per frame
            match m {
                Message::Chunks(chunks) => {
                    // println!(
                    //     "Requested load of {} chunks: \n{:?}",
                    //     chunks.len(),
                    //     chunks.iter().map(|x| x.0).collect::<Vec<Vector3<i32>>>()
                    // );

                    let cmd = self.load_chunks(chunks);
                    self.client
                        .0
                        .send(ClientMessage::Submit(cmd, self.origin, self.root_size, self.map.clone()))
                        .unwrap();
                }
                _ => (),
            }
        }
        self.conn.send(Message::PlayerMove(player));
    }

    /// Load a bunch of chunks at once. Prunes out-of-range chunks as well
    /// Uploads everything to GPU memory, returns a command buffer to copy it to the right spots in the main buffer
    pub fn load_chunks(&mut self, chunks: Vec<(Vector3<i32>, Chunk)>) -> AutoCommandBuffer {
        let mut cmd = AutoCommandBufferBuilder::primary_one_time_submit(
            self.device.clone(),
            self.queue.family(),
        )
        .unwrap();

        for (i, c) in chunks {
            cmd = self.load(i, c, cmd);
        }

        self.prune_chunks();
        self.create_root();
        self.upload_root(cmd).build().unwrap()
    }

    pub fn upload_root(&mut self, builder: AutoCommandBufferBuilder) -> AutoCommandBufferBuilder {
        let chunk = self.upload.chunk(self.root.clone()).unwrap();
        let view = vulkano::buffer::BufferSlice::from_typed_buffer_access(self.tree_buffer.clone())
            .slice(0..self.root.len())
            .unwrap();
        builder.copy_buffer(chunk, view).unwrap()
    }

    fn upload_chunk(
        &mut self,
        r: std::ops::Range<usize>,
        data: Chunk,
        builder: AutoCommandBufferBuilder,
    ) -> AutoCommandBufferBuilder {
        let chunk = self.upload.chunk(data.0).unwrap();
        let view = vulkano::buffer::BufferSlice::from_typed_buffer_access(self.tree_buffer.clone())
            .slice(r)
            .unwrap();
        builder.copy_buffer(chunk, view).unwrap()
    }

    /// Loads a chunk in at position `idx` in world-space (divided by CHUNK_SIZE)
    /// Will automatically unload the chunk that was previously there.
    /// Uploads this chunk to GPU memory, and returns a command buffer to copy it to the right location.
    pub fn load(
        &mut self,
        idx: Vector3<i32>,
        chunk: Chunk,
        builder: AutoCommandBufferBuilder,
    ) -> AutoCommandBufferBuilder {
        // Unload the previous chunk at this location, if there was one
        self.unload(idx);

        // We need this much space
        // We add space for 64 nodes to allow for the chunk to grow without moving. We'll move it if it goes past 32 - TODO
        let size = chunk.len() + 64 * 8;

        // Find a space
        let mut i = 0;
        let (start, end) = loop {
            let (space_start, space_end) = self.spaces[i];
            let space_size = space_end - space_start;
            if space_size == size {
                // Our chunk fits EXACTLY, so just remove this space
                self.spaces.remove(i);
                break (space_start, space_end);
            }
            if space_size > size {
                // Our chunk fits, so we can shrink this space
                // We'll put our new chunk at the start
                self.spaces[i] = (space_start + size, space_end);
                break (space_start, space_start + size);
            }

            // This one doesn't fit, so move on to the next space
            i += 1;
            if i >= self.spaces.len() {
                // We're to the end of `spaces`, so this chunk can't fit anywhere
                panic!("Could not find space for chunk {:?}, size {}!", idx, size);
            }
        };

        // Add the 64 empty nodes here
        let mut chunk_gpu = chunk.clone();
        chunk_gpu.append(&mut vec![0; 64 * 8]);

        // Add to map & chunks
        self.world.write().unwrap().add_chunk(idx, chunk);
        self.map.insert(idx, (start, end));

        // Upload to GPU
        self.upload_chunk(start..end, chunk_gpu, builder)
    }

    /// Unload the chunk at position `idx` in world space.
    /// This is the client function, so it won't store it anywhere or anything, that's the server's job.
    pub fn unload(&mut self, idx: Vector3<i32>) {
        if let Some((start, end)) = self.map.remove(&idx) {
            self.world.write().unwrap().remove_chunk(idx);

            // Add a space
            for i in 0..self.spaces.len() {
                let (space_start, space_end) = self.spaces[i];

                if space_start == end {
                    // This space was at the end of our chunk, so we can just extend it backwards to fill the space
                    self.spaces[i] = (start, space_end);
                    break;
                }
                if space_end == start {
                    // Our chunk was just after this space, so we can extend the space forwards
                    self.spaces[i] = (space_start, end);
                    break;
                }

                if space_start > end {
                    // This space is after our chunk, so we'll put our new space here. It's like insertion sort
                    self.spaces.insert(i, (start, end));
                    break;
                }

                // This space is before our chunk, so we'll keep going until we find the right position
            }

            // We don't have to touch GPU memory, because we aren't necessarily replacing this chunk with anything
        }
    }

    /// Unloads chunks that are too far away
    fn prune_chunks(&mut self) {
        let c = world_to_chunk(self.player);
        for i in self.map.clone().keys() {
            if (c - i).map(|x| x as f32).norm() > self.config.game_config.draw_chunks as f32 {
                self.unload(*i);
            }
        }
    }

    /// Recreates the root node to incorporate newly loaded chunks
    fn create_root(&mut self) {
        // Find the extent of the root in each direction
        let k: Vec<_> = self.world.read().unwrap().locs().cloned().collect();
        let l = k.iter()
            .fold(Vector3::new(10_000_000, 10_000_000, 10_000_000), |x, a| {
                x.zip_map(a, i32::min)
            });
        let h = k.into_iter()
            .fold(-Vector3::new(10_000_000, 10_000_000, 10_000_000), |x, a| {
                x.zip_map(&a, i32::max)
            });

        let h = chunk_to_world(h);
        let l = chunk_to_world(l);

        self.origin = chunk_to_world(world_to_chunk((h + l) * 0.5)); // + Vector3::repeat(CHUNK_SIZE * 0.5);
        self.root_size = (h - l).abs().max() + CHUNK_SIZE; // Add two halves of a chunk
        self.root_size = self.root_size.log2().ceil().exp2(); // Round up to a power of 2

        self.root = self.create_node(self.origin, self.root_size, 0);
    }

    /// Create a node in the root structure, returning that node and all children
    fn create_node(&self, pos: Vector3<f32>, size: f32, pointer: usize) -> Vec<u32> {
        let size = size * 0.5; // Child size
        let mut ret = vec![0; 8]; // ret[0..8] is the node we're actually working on
        for uidx in 0..8 {
            let idx = idx_to_pos(uidx);
            let pos = pos + idx * size * 0.5;
            if size > CHUNK_SIZE {
                // Descend
                let ptr = ret.len(); // Relative pointer to the first of the new nodes
                ret.append(&mut self.create_node(pos, size, pointer + ptr));
                let ptr = (ptr << 1) | 1;
                ret[uidx] = ptr as u32;
            } else {
                // This is a chunk, so figure out which one
                let chunk_loc = world_to_chunk(pos);
                let ptr = if let Some((chunk_ptr, _)) = self.map.get(&chunk_loc) {
                    if pointer >= *chunk_ptr {
                        panic!(
                            "pointer={}, chunk_ptr={}, chunk {}",
                            pointer, chunk_ptr, chunk_loc
                        );
                    }
                    ((chunk_ptr - pointer) << 1) | 1
                } else {
                    // There's no chunk here, it's empty
                    0
                };
                ret[uidx] = ptr as u32;
            }
        }
        ret
    }
}
