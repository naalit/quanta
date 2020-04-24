use crate::common::*;
use crate::config::GameConfig;
use crate::terrain::*;
use crate::world::*;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::mpsc::*;
use std::sync::Arc;

const CACHE_SIZE: usize = 16;

struct RegionCache {
    indices: VecDeque<(Vector3<i32>, usize)>,
    regions: Vec<Vec<Option<Vec<u8>>>>,
    path: std::path::PathBuf,
}

impl RegionCache {
    fn new() -> Self {
        let mut chunks_path =
            app_dirs2::app_root(app_dirs2::AppDataType::UserData, &crate::APP_INFO).unwrap();
        chunks_path.push("regions");
        if !chunks_path.exists() {
            std::fs::create_dir_all(&chunks_path).unwrap();
        }

        RegionCache {
            indices: VecDeque::new(),
            regions: Vec::new(),
            path: chunks_path,
        }
    }

    fn _store(&mut self, v: Vector3<i32>, mut region: Vec<Option<Vec<u8>>>) -> usize {
        if self.indices.len() < CACHE_SIZE {
            assert_eq!(self.regions.len(), self.indices.len());
            self.regions.push(region);
            let i = self.regions.len() - 1;
            self.indices.push_front((v, i));
            i
        } else {
            let (nv, i) = self.indices.pop_back().unwrap();
            self.indices.push_front((v, i));

            std::mem::swap(&mut region, &mut self.regions[i]);

            use std::fs::File;
            use std::io::Write;

            let mut path = self.path.clone();
            path.push(format!("{},{},{}.region.zst", nv.x, nv.y, nv.z));
            let f = File::create(path).unwrap();
            let mut f = zstd::stream::write::Encoder::new(f, 3).unwrap();

            f.write_all(&bincode::serialize(&region).unwrap()).unwrap();

            f.finish().unwrap();

            i
        }
    }

    fn _load(&mut self, v: Vector3<i32>) -> usize {
        for i in 0..self.indices.len() {
            if self.indices[i].0 == v {
                let t = self.indices[i];
                self.indices.remove(i);
                self.indices.push_front(t);
                return t.1;
            }
        }

        // It's not in the cache, so load it from disk
        let mut path = self.path.clone();
        path.push(format!("{},{},{}.region.zstd", v.x, v.y, v.z));

        let region: Vec<Option<Vec<u8>>> = if path.exists() {
            use std::fs::File;
            use std::io::Read;

            let f = File::open(path).unwrap();
            let mut f = zstd::stream::read::Decoder::new(f).unwrap();

            let mut buf = Vec::new();
            f.read_to_end(&mut buf).unwrap();

            bincode::deserialize(&buf).unwrap()
        } else {
            (0..REGION_SIZE * REGION_SIZE * REGION_SIZE)
                .map(|_| None)
                .collect()
        };

        self._store(v, region)
    }

    fn load(&mut self, chunk: Vector3<i32>) -> Option<Chunk> {
        let v = chunk_to_region(chunk);
        let idx = in_region(chunk);

        let ri = self._load(v);
        if let Some(x) = &self.regions[ri][idx] {
            bincode::deserialize(x).ok()
        } else {
            None
        }
    }

    fn store(&mut self, pos: Vector3<i32>, chunk: Chunk) {
        let ser = bincode::serialize(&chunk).unwrap();

        let v = chunk_to_region(pos);
        let idx = in_region(pos);

        let ri = self._load(v);
        self.regions[ri][idx] = Some(ser);
    }
}

pub struct ChunkThread {
    pub gen: Gen,
    ch: (Sender<ChunkMessage>, Receiver<ChunkMessage>),
    config: Arc<GameConfig>,
    world: ArcWorld,
}

impl ChunkThread {
    pub fn new(
        config: Arc<GameConfig>,
        world: ArcWorld,
        to: Sender<ChunkMessage>,
        from: Receiver<ChunkMessage>,
    ) -> Self {
        ChunkThread {
            gen: Gen::new(),
            ch: (to, from),
            config,
            world,
        }
    }

    pub fn run(self) {
        let save = self.config.save_chunks;

        let mut cache = RegionCache::new();

        let mut to_decorate = HashSet::new();

        let mut chunks_path =
            app_dirs2::app_root(app_dirs2::AppDataType::UserData, &crate::APP_INFO).unwrap();
        chunks_path.push("chunks");
        if !chunks_path.exists() {
            std::fs::create_dir_all(&chunks_path).unwrap();
        }

        let mut to_load = Vec::new();

        loop {
            if !to_load.is_empty() {
                // let timer = Stopwatch::start_new();
                let (decorate, mut ret): (Vec<_>, _) = {
                    let mut world = self.world.write().unwrap();
                    to_load
                        .drain(0..self.config.batch_size.min(to_load.len()))
                        .map(|p: Vector3<i32>| {
                            let chunk = if save {
                                cache.load(p).unwrap_or_else(|| {
                                    to_decorate.insert(p);
                                    self.gen.gen(p)
                                })
                            } else {
                                to_decorate.insert(p);
                                self.gen.gen(p)
                            };

                            world.add_chunk(p, chunk);
                            p
                        })
                        // So it's not lazy and can borrow to_decorate
                        .collect::<Vec<_>>()
                        .into_iter()
                        .partition(|x| to_decorate.contains(x))
                };

                let mut modified = Vec::new();

                let s: HashSet<Vector3<i32>> = ret.iter().chain(decorate.iter()).cloned().collect();

                for &p in s.iter() {
                    for n in neighbors(p) {
                        if to_decorate.contains(&n) {
                            let mut world = self.world.write().unwrap();
                            if neighbors(n).into_iter().all(|x| world.contains_chunk(x)) {
                                let m = self.gen.decorate(&mut *world, n);
                                modified.extend(m.into_iter().filter(|x| !s.contains(x)));
                                ret.push(n);
                                to_decorate.remove(&n);
                            }
                        }
                    }
                }

                self.ch.0.send(ChunkMessage::LoadChunks(ret)).unwrap();
                if !modified.is_empty() {
                    self.ch
                        .0
                        .send(ChunkMessage::UpdateChunks(modified))
                        .unwrap();
                }

                // println!("Loading took {} ms/chunk, {} ms total", timer.elapsed_ms() as f64 / l as f64, timer.elapsed_ms());

                let mut connected = true;
                let mut sort = Vec::new();
                loop {
                    match self.ch.1.try_recv() {
                        Ok(ChunkMessage::LoadChunks(mut chunks)) => {
                            to_load.append(&mut chunks);
                        }
                        Ok(ChunkMessage::UnloadChunk(_, _)) => {
                            // TODO save chunk
                        }
                        Ok(ChunkMessage::Players(players)) => {
                            sort = players;
                        }
                        Ok(ChunkMessage::Done) => {
                            self.ch.0.send(ChunkMessage::Done).unwrap();
                            connected = false;
                            break;
                        }
                        Err(TryRecvError::Disconnected) => {
                            connected = false;
                            break;
                        }
                        _ => break,
                    }
                }
                if !connected {
                    break;
                }
                if !sort.is_empty() {
                    for chunk in to_decorate.iter().cloned().collect::<Vec<_>>() {
                        let in_range = sort.iter().any(|y| {
                            (world_to_chunk(*y) - chunk).map(|x| x as f32).norm()
                                <= self.config.draw_chunks as f32
                        });
                        if !in_range {
                            self.world.write().unwrap().remove_chunk(chunk);
                            to_decorate.remove(&chunk);
                        }
                    }
                    // let timer = Stopwatch::start_new();
                    to_load.retain(|x| {
                        sort.iter().any(|y| {
                            (world_to_chunk(*y) - x).map(|x| x as f32).norm()
                                <= self.config.draw_chunks as f32
                        })
                    });
                    to_load.sort_by_cached_key(|x| {
                        let x = chunk_to_world(*x);
                        sort.iter().map(|y| ((x - y).norm() * 100.0) as usize).min()
                    });
                    // println!("Sorting took {} ms for to_load len {}", timer.elapsed().as_micros() as f64 / 1000.0, to_load.len());
                }
            } else {
                // Wait for more chunks to load
                match self.ch.1.recv() {
                    Ok(ChunkMessage::LoadChunks(mut chunks)) => {
                        to_load.append(&mut chunks);
                    }
                    Ok(ChunkMessage::UnloadChunk(p, chunk)) => {
                        if save {
                            cache.store(p, chunk);
                        }
                    }
                    Ok(ChunkMessage::Players(_)) => {}
                    _ => break,
                }
            }
        }
    }
}
