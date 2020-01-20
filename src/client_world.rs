use vulkano::sync::GpuFuture;
use crate::common::*;
use std::collections::HashMap;
use std::sync::Arc;
use vulkano::command_buffer::CommandBuffer;

pub struct ClientWorld {
    conn: Connection,
    origin: Vector3<f32>,
    player: Vector3<f32>,
    pub root_size: f32,
    pub root: Vec<u32>, // The root structure. Points to chunks, gets buffer in the map
    chunks: HashMap<Vector3<i32>, Chunk>,
    pub map: HashMap<Vector3<i32>, (usize, usize)>, // (start, end)
    spaces: Vec<(usize, usize)>,                       // (start, end)
    pub tree_buffer: Arc<vulkano::buffer::DeviceLocalBuffer<[u32]>>,
    upload: vulkano::buffer::CpuBufferPool<u32>,
}

impl ClientWorld {
    pub fn new(device: Arc<vulkano::device::Device>, conn: Connection, player: Vector3<f32>) -> Self {
        let start_len = 3_200_000;
        let mut max_root_size = CHUNK_NUM[0] * CHUNK_NUM[1] * CHUNK_NUM[2];
        let mut last = max_root_size * 8;
        while last > 0 {
            last = last / 8;
            max_root_size += last;
        }
        println!("Max root size = {}", max_root_size);

        ClientWorld {
            conn,
            origin: player.map(|x| x % CHUNK_SIZE),
            player,
            root_size: 8.0,//CHUNK_NUM.max() as f32 * CHUNK_SIZE,
            root: vec![0; 8],
            chunks: HashMap::new(),
            map: HashMap::new(),
            spaces: vec![(max_root_size as usize, start_len)],
            tree_buffer: vulkano::buffer::DeviceLocalBuffer::array(device.clone(), start_len, vulkano::buffer::BufferUsage {
                storage_buffer: true,
                // This actually shouldn't have to be set, this is a bug in vulkano: https://github.com/vulkano-rs/vulkano/issues/1283
                uniform_buffer: true,
                transfer_destination: true,
                ..vulkano::buffer::BufferUsage::none()
            }, device.active_queue_families()).unwrap(),
            upload: vulkano::buffer::CpuBufferPool::upload(device.clone()),
        }
    }

    pub fn origin(&self) -> [f32; 3] {
        self.origin.into()
    }

    pub fn update(&mut self, player: Vector3<f32>, device: Arc<vulkano::device::Device>, queue: Arc<vulkano::device::Queue>, future: &mut Box<dyn GpuFuture>) {
        self.player = player;
        if let Some(m) = self.conn.recv() {
            // Only load chunks once per frame
            match m {
                Message::Chunks(chunks) => {
                    /*
                    println!(
                        "Requested load of {} chunks: \n{:?}",
                        chunks.len(),
                        chunks.iter().map(|x| x.0).collect::<Vec<Vector3<i32>>>()
                    );
                    */
                    self.load_chunks(chunks, device, queue, future)
                }
                _ => (),
            }
        }
        self.conn.send(Message::PlayerMove(player));
    }

    /// Load a bunch of chunks at once. Prunes the root as well
    /// Uploads everything to the GPU
    pub fn load_chunks(&mut self, chunks: Vec<(Vector3<i32>, Chunk)>, device: Arc<vulkano::device::Device>, queue: Arc<vulkano::device::Queue>, future: &mut Box<dyn GpuFuture>) {
        let mut f: Box<dyn GpuFuture> = Box::new(vulkano::sync::now(device.clone()));
        std::mem::swap(&mut f, future);

        f.then_signal_fence_and_flush().unwrap().wait(None).unwrap();
        let mut f: Box<dyn GpuFuture> = Box::new(vulkano::sync::now(device.clone()));

        for (i, c) in chunks {
            f = Box::new(self.load(i, c, device.clone(), queue.family()).execute_after(f, queue.clone()).unwrap());
        }

        self.prune_chunks();
        self.create_root();
        f = Box::new(self.upload_root(device, queue.family()).execute_after(f, queue).unwrap());

        f.then_signal_fence_and_flush().unwrap().wait(None).unwrap();
    }

    pub fn upload_root(&mut self, device: Arc<vulkano::device::Device>, queue_family: vulkano::instance::QueueFamily) -> vulkano::command_buffer::AutoCommandBuffer {
        let chunk = self.upload.chunk(self.root.clone()).unwrap();
        let view = vulkano::buffer::BufferSlice::from_typed_buffer_access(self.tree_buffer.clone()).slice(0..self.root.len()).unwrap();
        vulkano::command_buffer::AutoCommandBufferBuilder::primary_one_time_submit(device, queue_family)
            .unwrap()
            .copy_buffer(chunk, view)
            .unwrap()
            .build()
            .unwrap()
    }

    fn upload_chunk(&mut self, r: std::ops::Range<usize>, data: Chunk, device: Arc<vulkano::device::Device>, queue_family: vulkano::instance::QueueFamily) -> vulkano::command_buffer::AutoCommandBuffer {
        let chunk = self.upload.chunk(data.0).unwrap();
        let view = vulkano::buffer::BufferSlice::from_typed_buffer_access(self.tree_buffer.clone()).slice(r).unwrap();
        vulkano::command_buffer::AutoCommandBufferBuilder::primary_one_time_submit(device, queue_family)
            .unwrap()
            .copy_buffer(chunk, view)
            .unwrap()
            .build()
            .unwrap()
    }

    /// Loads a chunk in at position `idx` in world-space (divided by CHUNK_SIZE)
    /// Will automatically unload the chunk that was previously there.
    /// Uploads this chunk to the GPU, but not the modified root structure.
    pub fn load(&mut self, idx: Vector3<i32>, chunk: Chunk, device: Arc<vulkano::device::Device>, queue_family: vulkano::instance::QueueFamily) -> vulkano::command_buffer::AutoCommandBuffer {
        // Unload the previous chunk
        self.unload(idx);

        // We need this much space
        // We add 64 to allow for the chunk to grow without moving. We'll move it if it goes past 32 - TODO
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

        // println!("Found a space at {}", start);

        // Add the 64 empty nodes here
        let mut chunk_gpu = chunk.clone();
        chunk_gpu.append(&mut vec![0; 64 * 8]);

        // Add to map & chunks
        self.chunks.insert(idx, chunk);
        self.map.insert(idx, (start, end));

        // Upload to GPU
        self.upload_chunk(start..end, chunk_gpu, device, queue_family)
    }

    /// Unload the chunk at position `idx` in world space.
    /// This is the client function, so it won't store it anywhere or anything, that's the server's job.
    pub fn unload(&mut self, idx: Vector3<i32>) {
        if let Some((start, end)) = self.map.remove(&idx) {
            self.chunks.remove(&idx);

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
        for i in self.map.clone().keys() {
            let p = chunk_to_world(*i);
            let d = (p - self.player).norm();
            if d > ROOT_SIZE {
                self.unload(*i);
            }
        }
    }

    /// Recreates the root node to incorporate newly loaded chunks
    fn create_root(&mut self) {
        // Find the extent of the root in each direction
        let l = self.chunks.keys().fold(
            Vector3::new(10_000_000, 10_000_000, 10_000_000),
            |x, a| x.zip_map(a, i32::min),
        );
        let h = self.chunks.keys().fold(
            -Vector3::new(10_000_000, 10_000_000, 10_000_000),
            |x, a| x.zip_map(a, i32::max),
        );

        let h = chunk_to_world(h);
        let l = chunk_to_world(l);

        self.origin = chunk_to_world(world_to_chunk((h+l)*0.5)) + Vector3::repeat(CHUNK_SIZE * 0.5);
        self.root_size = (h-l).abs().max() + CHUNK_SIZE; // Add two halves of a chunk
        self.root_size = self.root_size.log2().ceil().exp2(); // Round up to a power of 2

        self.root = self.create_node(self.origin, self.root_size, 0);
    }

    /// Create a node in the root structure, returning that node and all children
    fn create_node(&self, pos: Vector3<f32>, size: f32, pointer: usize) -> Vec<u32> {
        let size = size * 0.5; // Child size
        let mut ret = Vec::new();
        ret.append(&mut vec![0; 8]); // ret[0] is the node we're actually working on
        for uidx in 0..8 {
            let idx = idx_to_pos(uidx);
            let pos = pos + idx * size * 0.5;
            if size > CHUNK_SIZE {
                // Descend
                let ptr = ret.len(); // Relative pointer to the first of the new nodes
                ret.append(&mut self.create_node(pos, size, pointer+ptr));
                let ptr = (ptr << 1) | 1;
                ret[uidx] = ptr as u32;
            } else {
                // This is a chunk, so figure out which one
                let chunk_loc = world_to_chunk(pos);
                let ptr = if let Some((chunk_ptr, _)) = self.map.get(&chunk_loc) {
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
