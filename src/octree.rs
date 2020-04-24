use crate::common::*;
use num_traits::FromPrimitive;

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

/// The result of a raycast
#[derive(Clone, Debug)]
pub struct RayCast {
    pub t: [f32; 2],
    pub mat: Material,
    /// The center of the block we hit, relative to the chunk center.
    /// Note that `ro+rd*t` is the hit position.
    pub pos: Vector3<f32>,
}

/// Returns (t, tmid, tmax)
pub fn isect(
    ro: Vector3<f32>,
    rdi: Vector3<f32>,
    pos: Vector3<f32>,
    size: f32,
) -> ([f32; 2], Vector3<f32>, Vector3<f32>) {
    let mn = pos.map(|x| x - size * 0.5);
    let mx = pos.map(|x| x + size * 0.5);
    let t1 = (mn - ro).zip_map(&rdi, std::ops::Mul::mul);
    let t2 = (mx - ro).zip_map(&rdi, std::ops::Mul::mul);
    let tmid = (pos - ro).zip_map(&rdi, std::ops::Mul::mul);

    let tmin = t1.zip_map(&t2, f32::min);
    let tmax = t1.zip_map(&t2, f32::max);

    ([tmin.max(), tmax.min()], tmid, tmax)
}

impl Chunk {
    /// Casts a ray from a position relative to the chunk center
    #[allow(clippy::float_cmp)]
    pub fn raycast(&self, ro: Vector3<f32>, rd: Vector3<f32>, max_iters: usize) -> Option<RayCast> {
        struct ST {
            parent: usize,
            pos: Vector3<f32>,
            idx: Vector3<f32>,
            size: f32,
            h: f32,
        }

        let mut stack = Vec::new();

        let tstep = rd.map(f32::signum);
        let rdi = rd.map(|x| 1.0 / x); // Inverse for isect

        let mut pos = Vector3::zeros();

        let (t, tmid, tmax) = isect(ro, rdi, pos, CHUNK_SIZE);
        if t[0] > t[1] || t[1] <= 0.0 {
            return None;
        }
        let mut h = t[1];

        // Which axes we're skipping the first voxel on (hitting it from the side)
        let q = tmid.map(|x| x <= t[0]);
        let idx = q.zip_map(&tstep, |b, x| if b { x } else { -x });
        // tmax of the resulting voxel
        let tq = q.zip_zip_map(&tmid, &tmax, |b, x, y| if b { y } else { x });
        // Don't worry about voxels behind `ro`
        let mut idx = tq.zip_map(&idx, |x, y| if x >= 0.0 { y } else { -y });

        let mut size = CHUNK_SIZE * 0.5;
        pos += 0.5 * size * idx;
        let mut parent = 0;

        let mut c = true;

        for _ in 0..max_iters {
            let (t, tmid, tmax) = isect(ro, rdi, pos, size);

            let uidx = pos_to_idx(idx);

            let node = self[parent + uidx];

            if (node & 1) > 0 {
                // Non-leaf
                if c {
                    //-- PUSH --//
                    if t[1] < h {
                        stack.push(ST {
                            parent,
                            pos,
                            idx,
                            size,
                            h,
                        });
                    }
                    h = t[1];
                    parent += (node >> 1) as usize;
                    size *= 0.5;
                    // Which axes we're skipping the first voxel on (hitting it from the side)
                    let q = tmid.map(|x| x <= t[0]);
                    idx = q.zip_map(&tstep, |b, x| if b { x } else { -x });
                    // tmax of the resulting voxel
                    let tq = q.zip_zip_map(&tmid, &tmax, |b, x, y| if b { y } else { x });
                    // Don't worry about voxels behind `ro`
                    idx = tq.zip_map(&idx, |x, y| if x >= 0.0 { y } else { -y });
                    pos += 0.5 * size * idx;
                    continue;
                }
            } else if node != 0 {
                // Nonempty, but leaf
                return Some(RayCast {
                    mat: Material::from_u32(node >> 1).unwrap_or(Material::Wrong),
                    t,
                    pos,
                });
            }

            //-- ADVANCE --//

            // Advance for every direction where we're hitting the side
            let old = idx;
            idx = idx.zip_zip_map(&tstep, &tmax, |a, b, s| if s == t[1] { b } else { a });
            pos += tstep.zip_zip_map(&old, &idx, |x, a, b| if a != b { x * size } else { 0.0 });

            if old == idx {
                // We're at the last child
                //-- POP --//
                let st = stack.pop()?;
                h = st.h;
                idx = st.idx;
                parent = st.parent;
                pos = st.pos;
                size = st.size;

                c = false;
            } else {
                c = true;
            }
        }

        println!("WARNING: ran out of iterations in Chunk::raycast()!");
        None
    }

    /// Get the material at a location relative to the chunk center
    pub fn block(&self, target: Vector3<f32>) -> Material {
        let mut size = CHUNK_SIZE;
        let mut pos = Vector3::zeros();
        let mut parent = 0;

        loop {
            size *= 0.5;
            let idx = (target - pos).map(f32::signum);
            pos += idx * size * 0.5;

            let uidx = pos_to_idx(idx);
            let node = self[parent + uidx];

            // We have more nodes to traverse within this one
            if node & 1 > 0 {
                parent += (node >> 1) as usize;
            } else {
                break Material::from_u32(node >> 1).unwrap();
            }
        }
    }

    /// Set the material at a location relative to the chunk center
    /// Setting a block to air does work, but uses more space
    pub fn set_block(&mut self, target: Vector3<f32>, level: u32, new: Material) {
        let mut size = CHUNK_SIZE;
        let mut pos = Vector3::zeros();
        let mut parent = 0;

        // Find the spot to put this block, creating a new subtree if necessary
        for i in 0..level {
            size *= 0.5;
            let idx = (target - pos).map(f32::signum);
            pos += idx * size * 0.5;

            let uidx = pos_to_idx(idx);
            let ptr = parent + uidx;

            if i == level - 1 {
                // Actually put the new material there
                self[ptr] = (new as u32) << 1;
                break;
            }

            let node = self[ptr];

            // We have more nodes to traverse within this one
            if node & 1 > 0 {
                parent += (node >> 1) as usize;
            } else {
                // Create a new node
                self[ptr] = ((self.len() - parent) as u32) << 1 | 1;
                parent = self.len();
                self.extend((0..8).map(|_| node));
            }
        }
    }

    pub fn empty() -> Self {
        Chunk(vec![0; 8])
    }

    pub fn from_dist(mut dist: impl FnMut(Vector3<f32>) -> (f32, Material)) -> Self {
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
            let (pos, root, idx, parent, scale) = if i == 0 {
                (
                    Vector3::repeat(CHUNK_SIZE * 0.5),
                    true,
                    Vector3::zeros(),
                    0,
                    0,
                )
            } else if !stack.is_empty() {
                let s = stack.pop().unwrap();
                (s.pos, false, s.idx, s.parent, s.scale)
            } else {
                break;
            };

            let mut v = vec![0; 8];
            let size = 2.0_f32.powf(-scale as f32) * CHUNK_SIZE * 0.5; // Next level's size
            for j in 0..8 {
                let jdx = idx_to_pos(j);
                let np = pos + jdx * size * 0.5;

                let (d, mat) = dist(np);
                if scale >= levels {
                    if d > size * d_corner {
                        v[j] = 0;
                    } else {
                        v[j] = (mat as u32) << 1;
                    }
                } else if d > size * d_corner {
                    //v.leaf[j] = true;
                    v[j] = 0;
                } else if d < -size * d_corner {
                    //v.leaf[j] = true;
                    v[j] = (mat as u32) << 1;
                } else {
                    stack.push(ST {
                        parent: i * 8,
                        idx: jdx,
                        pos: np,
                        scale: scale + 1,
                    });
                }
            }
            if !root {
                let uidx = pos_to_idx(idx);
                tree[parent + uidx] = (((i * 8 - parent) as u32) << 1) | 1;
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
