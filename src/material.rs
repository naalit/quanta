use enum_iterator::IntoEnumIterator;
use serde::{Deserialize, Serialize};
use crate::shaders::MatData;

#[derive(IntoEnumIterator, PartialEq, Clone, Copy, Debug, Serialize, Deserialize)]
#[repr(u16)]
pub enum Material {
    Air = 0,
    Stone,
    Grass,
    Dirt,
    Water = 4,
    Sand,
    Wood,
    Leaf,
    Wrong,
}

impl Material {
    pub fn all() -> Vec<MatData> {
        Material::into_enum_iter()
            .map(|x| x.mat_data())
            .collect()
    }

    pub fn mat_data(self) -> MatData {
        match self {
            Material::Stone => MatData {
                color: [0.4; 3],
                roughness: 0.2,
                trans: 0.0,
                metal: 0.0,
                ior: 1.45,
                nothing: 0.0,
            },
            Material::Grass => MatData {
                color: [0.4, 0.7, 0.5],
                roughness: 0.6,
                trans: 0.0,
                metal: 0.0,
                ior: 1.45,
                nothing: 0.0,
            },
            Material::Dirt => MatData {
                color: [0.4, 0.3, 0.3],
                roughness: 0.9,
                trans: 0.0,
                metal: 0.0,
                ior: 1.45,
                nothing: 0.0,
            },
            Material::Sand => MatData {
                color: [0.9, 0.7, 0.6],
                roughness: 0.6,
                trans: 0.0,
                metal: 0.0,
                ior: 1.45,
                nothing: 0.0,
            },
            Material::Water => MatData {
                color: [0.3, 0.4, 0.5],
                roughness: 0.01,
                trans: 0.5,
                metal: 0.0,
                ior: 1.33,
                nothing: 0.0,
            },
            Material::Air => MatData {
                color: [0.0; 3],
                roughness: 1.0,
                trans: 1.0,
                metal: 0.0,
                ior: 1.0,
                nothing: 0.0,
            },
            Material::Wood => MatData {
                color: [0.1, 0.1, 0.1],
                roughness: 0.9,
                trans: 0.0,
                metal: 0.0,
                ior: 1.45,
                nothing: 0.0,
            },
            Material::Leaf => MatData {
                color: [0.1, 0.3, 0.2],
                roughness: 0.6,
                trans: 0.0,
                metal: 0.0,
                ior: 1.45,
                nothing: 0.0,
            },
            Material::Wrong => MatData {
                color: [1000.0, 0.0, 0.0],
                roughness: 1.0,
                trans: 0.0,
                metal: 0.0,
                ior: 1.45,
                nothing: 0.0,
            }
        }
    }
}
