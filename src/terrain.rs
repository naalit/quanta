use crate::common::*;
use crate::world::World;
use noise::*;
// use rayon::prelude::*;

pub struct Gen {
    noise: HybridMulti,
}

impl Gen {
    pub fn new() -> Self {
        Gen {
            noise: HybridMulti::new()
                .set_seed(1)
                .set_octaves(8)
                .set_persistence(0.5),
        }
    }

    pub fn decorate(&self, world: &mut World, chunk: Vector3<i32>) -> Vec<Vector3<i32>> {
        // TODO port
        Vec::new()
    }

/*
    /// Returns the chunks it modified besides the one it's decorating (neighbor chunks)
    pub fn decorate(&self, world: &mut World, chunk: Vector3<i32>) -> Vec<Vector3<i32>> {
        let mut modified = Vec::new();

        let start = chunk.map(|x| x * CHUNK_SIZE as i32);

        let chunk_heightmap = (0..CHUNK_SIZE as usize)
            .map(move |x| {
                (0..CHUNK_SIZE as usize)
                    .map(move |z| {
                        3.0 + 48.0
                            * self.noise.get([
                                (start.x as f64 + x as f64) * 0.0004,
                                (start.z as f64 + z as f64) * 0.0004,
                            ]) as f32
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let ntrees = (self.noise.get([
            chunk.x as f64 * 0.04,
            chunk.y as f64 * 0.04,
            chunk.z as f64 * 0.04,
        ]) * 14.0)
            .abs() as i32;

        for i in 0..ntrees {
            // Attempt to generate a tree

            // Modifier so every tree is different
            let f = i as f64;

            let fx = self
                .noise
                .get([
                    chunk.x as f64 + f * 2.3,
                    chunk.y as f64 - f,
                    chunk.z as f64 + f,
                ])
                .abs()
                / 1.5;
            let x = (fx * CHUNK_SIZE as f64).min(CHUNK_SIZE as f64 - 1.0) as usize;
            let fz = self
                .noise
                .get([
                    chunk.x as f64 + f,
                    chunk.y as f64 + f * 3.9,
                    chunk.z as f64 - f * 0.91,
                ])
                .abs()
                / 1.5;
            let z = (fz * CHUNK_SIZE as f64).min(CHUNK_SIZE as f64 - 1.0) as usize;

            let y = chunk_heightmap[x][z].ceil() as i32 + 1;
            // Is it in this chunk? (instead of above or below)
            if (y - start.y) > 0 && (y - start.y) < CHUNK_SIZE as i32 {
                let x = start.x + x as i32;
                let z = start.z + z as i32;

                if world.block(Vector3<f32>::new(x as f32, (y - 1) as f32, z as f32))
                    == Some(Material::Grass)
                {
                    let tree_height = (self.noise.get([
                        chunk.x as f64 + fz,
                        chunk.y as f64 - fx,
                        chunk.z as f64 + y as f64,
                    ]) * 8.0)
                        .abs() as i32;

                    // Trunk
                    for y in y..y + tree_height {
                        let v = Vector3<f32>::new(x as f32, y as f32, z as f32);
                        if world_to_chunk(v) != chunk {
                            modified.push(world_to_chunk(v));
                        }
                        world.set_block(v, Material::Wood);
                    }

                    let canopy_width = (self.noise.get([
                        chunk.x as f64 - fz * 2.3,
                        chunk.y as f64 + fx * 3.0,
                        chunk.z as f64 - y as f64,
                    ]))
                    .abs() as i32
                        + 1;

                    for x in x - canopy_width..x + 1 + canopy_width {
                        for z in z - canopy_width..z + 1 + canopy_width {
                            for y in y + tree_height - canopy_width..y + tree_height + canopy_width
                            {
                                let v = Vector3<f32>::new(x as f32, y as f32, z as f32);

                                if world_to_chunk(v) != chunk {
                                    modified.push(world_to_chunk(v));
                                }

                                if world.block(v) == Some(Material::Air) {
                                    world.set_block(v, Material::Leaf);
                                }
                            }
                        }
                    }
                }
            }
        }

        modified.sort_by_key(|x| (x.x, x.y, x.z));
        modified.dedup();

        modified
    }
*/
    pub fn gen(&self, pos: Vector3<i32>) -> Chunk {
        let start = pos.map(|x| x * CHUNK_SIZE as i32);

        let chunk_heightmap = (0..CHUNK_SIZE as usize)
            .map(move |x| {
                (0..CHUNK_SIZE as usize)
                    .map(move |z| {
                        3.0 + 48.0
                            * self.noise.get([
                                (start.x as f64 + x as f64) * 0.0004,
                                (start.z as f64 + z as f64) * 0.0004,
                            ]) as f32
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        // The whole chunk is above the ground, so we don't need to bother
        if start.y > 0
            && start.y
                > chunk_heightmap
                    .iter()
                    .flatten()
                    .map(|x| x.ceil() as i32)
                    .max()
                    .unwrap()
        {
            return Chunk::empty();
        }

        Chunk::from_dist(|p| {
            let height = chunk_heightmap[p.x as usize][p.z as usize];
            let y = p.y as i32 + start.y;
            (y - height.ceil() as i32) as f32 * 0.4
        })

        /*
        Chunk::full(&|p| {
            let height = chunk_heightmap[p.x][p.z]; //3.0 + 4.0 * self.noise.get([(start.x as f64 + x as f64) * 0.01, (start.z as f64 + z as f64) * 0.01]) as f32;
            let y = p.y as i32 + start.y;
            if y == height.ceil() as i32 {
                if y < 3 + self.noise.get([
                    (start.x as f64 + p.x as f64) * 0.04,
                    (start.z as f64 + p.z as f64) * 0.04,
                ]) as i32
                {
                    Material::Sand
                } else {
                    Material::Grass
                }
            } else if y < height.ceil() as i32 && y > height as i32 - 3 {
                Material::Dirt
            } else if y < height as i32 {
                Material::Stone
            } else if y < 0 {
                Material::Water
            } else {
                Material::Air
            }
        })
        */
    }
}
