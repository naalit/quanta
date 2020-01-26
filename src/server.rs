use crate::chunk_thread::*;
use crate::common::*;
use crate::config::*;
use crate::world::*;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::mpsc::*;
use std::sync::Arc;
use std::thread;

struct Player {
    pos: Vector3<f32>,
    conn: Rc<Connection>,
    id: usize,
}

pub struct Server {
    world: ArcWorld,
    refs: HashMap<Vector3<i32>, usize>,
    players: Vec<Player>,
    orders: HashMap<Vector3<i32>, Vec<(usize, Rc<Connection>)>>,
    ch: (Sender<ChunkMessage>, Receiver<ChunkMessage>),
    config: Arc<GameConfig>,
}

impl Server {
    /// Creates and starts a chunk thread, and creates a Server
    pub fn new(config: Arc<GameConfig>) -> Self {
        let (to, from_them) = channel();
        let (to_them, from) = channel();
        let c = Arc::clone(&config);
        let world = arcworld();
        let wc = Arc::clone(&world);

        thread::spawn(move || ChunkThread::new(c, wc, to_them, from_them).run());

        Server {
            world,
            refs: HashMap::new(),
            players: Vec::new(),
            orders: HashMap::new(),
            ch: (to, from),
            config,
        }
    }

    /// Add a player to the game
    pub fn join(&mut self, conn: Connection, pos: Vector3<f32>) {
        let new_player = Player {
            pos,
            conn: Rc::new(conn),
            id: self.players.len(),
        };
        let (wait, load) = self.load_chunks_around(pos);
        //p.to_send.append(&mut wait);
        for i in wait {
            self.orders
                .entry(i)
                .or_insert_with(Vec::new)
                .push((new_player.id, Rc::clone(&new_player.conn)));
        }
        if !load.is_empty() {
            new_player.conn.send(Message::Chunks(load)).unwrap();
        }
        self.players.push(new_player);
    }

    /// Runs an infinite tick loop. It's infinite, start as a new thread!
    pub fn run(mut self) {
        let mut running = true;
        while running {
            let mut p = Vec::new();
            std::mem::swap(&mut p, &mut self.players);
            let mut change = false;
            self.players = p
                .into_iter()
                .filter_map(|mut p| {
                    let mut np = p.pos;
                    while let Some(m) = p.conn.recv() {
                        match m {
                            Message::PlayerMove(n_pos) => {
                                np = n_pos;
                            }
                            Message::Leave => match *p.conn {
                                Connection::Local(_, _) => {
                                    running = false;
                                    break;
                                }
                                _ => return None,
                            },
                            // Message::SetBlock(p, b) => {
                            //     self.world
                            //         .write()
                            //         .unwrap()
                            //         .set_block(p.map(|x| x as f32), b);
                            // }
                            _ => panic!("Hey, a client sent a message {:?}", m),
                        }
                    }
                    let (wait, load) = self.load_chunk_diff(p.pos, np);
                    //p.to_send.append(&mut wait);
                    if !change && (!wait.is_empty() || !load.is_empty()) {
                        change = true;
                    }
                    for i in wait {
                        self.orders
                            .entry(i)
                            .or_insert_with(Vec::new)
                            .push((p.id, Rc::clone(&p.conn)));
                    }
                    if !load.is_empty() {
                        p.conn.send(Message::Chunks(load)).unwrap();
                    }
                    p.pos = np;
                    Some(p)
                })
                .collect();

            if change {
                let p: Vec<Vector3<f32>> = self.players.iter().map(|x| x.pos).collect();
                let p2: Vec<_> = p.iter().map(|x| world_to_chunk(*x)).collect();
                let keys: Vec<_> = self.orders.keys().cloned().collect();
                for k in keys {
                    if !p2
                        .iter()
                        .any(|y| (y - k).map(|x| x as f32).norm() <= self.config.draw_chunks as f32)
                    {
                        self.orders.remove(&k);
                    }
                }
                self.ch.0.send(ChunkMessage::Players(p)).unwrap();
            }

            while let Ok(m) = self.ch.1.try_recv() {
                match m {
                    ChunkMessage::LoadChunks(x) => {
                        let batches = {
                            let mut batches = HashMap::new();
                            let world = self.world.read().unwrap();
                            for i in &x {
                                if let Some(v) = self.orders.remove(i) {
                                    if let Some(c) = world.chunk(*i) {
                                        for (id, conn) in v {
                                            batches
                                                .entry(id)
                                                .or_insert_with(|| (conn, Vec::new()))
                                                .1
                                                .push((*i, c.clone()));
                                        }
                                    } else {
                                        println!("Error!");
                                    }
                                }
                            }
                            batches
                        };
                        for (_, (conn, v)) in batches {
                            conn.send(Message::Chunks(v));
                        }
                    }
                    ChunkMessage::UpdateChunks(v) => {
                        let mut batches = HashMap::new();
                        for i in v {
                            for p in &self.players {
                                if (world_to_chunk(p.pos) - i).map(|x| x as f32).norm()
                                    <= self.config.draw_chunks as f32
                                {
                                    batches
                                        .entry(p.id)
                                        .or_insert((p.conn.clone(), Vec::new()))
                                        .1
                                        .push(i);
                                }
                            }
                        }
                        let world = self.world.read().unwrap();
                        for (_, (conn, v)) in batches {
                            conn.send(Message::Chunks(
                                v.into_iter()
                                    .filter_map(|x| world.chunks.get(&x).cloned().map(|y| (x, y)))
                                    .collect(),
                            ))
                            .unwrap();
                        }
                    }
                    _ => panic!("Chunk thread sent {:?}", m),
                }
            }
        }
        self.unload_all();
        for p in self.players {
            p.conn.send(Message::Leave);
        }
    }

    fn unload_all(&mut self) {
        let mut m = HashMap::new();
        std::mem::swap(&mut self.world.write().unwrap().chunks, &mut m);
        for (loc, chunk) in m {
            self.ch
                .0
                .send(ChunkMessage::UnloadChunk(loc, chunk))
                .unwrap();
        }
        self.ch.0.send(ChunkMessage::Done).unwrap();
        while let Ok(m) = self.ch.1.recv() {
            if let ChunkMessage::Done = m {
                break;
            }
        }
    }

    /// Loads initial chunks around a player
    /// Returns `(chunks_to_wait_for, chunks_already_loaded)`
    /// Doesn't update `orders`
    fn load_chunks_around(
        &mut self,
        pos: Vector3<f32>,
    ) -> (Vec<Vector3<i32>>, Vec<(Vector3<i32>, Chunk)>) {
        let chunk_pos = world_to_chunk(pos);

        let mut to_load = Vec::new();

        let draw_chunks = self.config.draw_chunks as i32;

        for x in -draw_chunks..draw_chunks {
            for y in -draw_chunks..draw_chunks {
                for z in -draw_chunks..draw_chunks {
                    let p = Vector3::new(x, y, z);
                    if p.map(|x| x as f32).norm() <= self.config.draw_chunks as f32 {
                        to_load.push(p);
                    }
                }
            }
        }

        to_load.sort_by_cached_key(|a| ((a.map(|x| x as f32)).norm() * 10.0) as i32);

        let mut to_send = Vec::new();
        let mut to_pass = Vec::new();
        let world = self.world.read().unwrap();
        for p in to_load {
            let p = chunk_pos + p;
            match world.chunk(p) {
                Some(chunk) => to_pass.push((p, chunk.clone())),
                None => to_send.push(p),
            }
            match self.refs.get_mut(&p) {
                Some(x) => *x += 1, // This is indeed possible but ugly; see Todo below
                None => {
                    self.refs.insert(p, 1);
                }
            }
        }

        // If it's already being loaded, don't tell the chunk thread to load it again.
        // The calling function will add this player to `orders` too, so we don't need to bother here
        to_send.retain(|x| !self.orders.contains_key(&x));

        self.ch
            .0
            .send(ChunkMessage::LoadChunks(to_send.clone()))
            .unwrap();
        (to_send, to_pass)
    }

    /// Figures out what chunks need to be loaded, and either returns them or sends them to the chunk thread
    /// Returns `(chunks_to_wait_for, chunks_already_loaded)`
    /// Doesn't update `orders`
    fn load_chunk_diff(
        &mut self,
        old: Vector3<f32>,
        new: Vector3<f32>,
    ) -> (Vec<Vector3<i32>>, Vec<(Vector3<i32>, Chunk)>) {
        let chunk_old = world_to_chunk(old);
        let chunk_new = world_to_chunk(new);

        if chunk_old == chunk_new {
            return (Vec::new(), Vec::new());
        }

        // println!("Loading chunks around {:?}", new);

        let mut around_old = HashSet::new();
        let mut around_new = HashSet::new();
        let draw_chunks = self.config.draw_chunks as i32;

        for x in -draw_chunks..draw_chunks {
            for y in -draw_chunks..draw_chunks {
                for z in -draw_chunks..draw_chunks {
                    let p = Vector3::new(x, y, z);
                    if p.map(|x| x as f32).norm() <= self.config.draw_chunks as f32 {
                        around_old.insert(chunk_old + p);
                        around_new.insert(chunk_new + p);
                    }
                }
            }
        }
        let to_load = &around_new - &around_old;
        let to_unload = &around_old - &around_new;

        let mut world = self.world.write().unwrap();
        for i in to_unload {
            if self.refs.contains_key(&i) {
                let r = {
                    // Lower the refcount on this chunk by one
                    let q = self
                        .refs
                        .get_mut(&i)
                        .expect("Tried to unload a chunk that isn't loaded");
                    let r = *q - 1;
                    *q = r;
                    r
                };
                // If the refcount is zero, nobody's using it so we can unload it
                if r == 0 {
                    // TODO tell chunk thread to unload this chunk
                    if let Some(chunk) = world.remove_chunk(i) {
                        self.ch.0.send(ChunkMessage::UnloadChunk(i, chunk)).unwrap();
                    }
                    self.refs.remove(&i);
                }
            } else {
                panic!("Tried to unload a chunk that isn't loaded [2]: {:?}", i);
            }
        }

        let mut to_send = Vec::new();
        let mut to_pass = Vec::new();
        for p in to_load {
            match world.chunk(p) {
                Some(chunk) => to_pass.push((p, chunk.clone())),
                None => to_send.push(p),
            }
            match self.refs.get_mut(&p) {
                Some(x) => *x += 1, // This is indeed possible but ugly; see Todo below
                None => {
                    self.refs.insert(p, 1);
                }
            }
        }

        // If it's already being loaded, don't tell the chunk thread to load it again.
        // The calling function will add this player to `orders` too, so we don't need to bother here
        to_send.retain(|x| !self.orders.contains_key(&x));

        to_send.sort_by_cached_key(|a| ((a - chunk_new).map(|x| x as f32).norm() * 10.0) as i32);

        self.ch
            .0
            .send(ChunkMessage::LoadChunks(to_send.clone()))
            .unwrap();
        (to_send, to_pass)
    }
}
