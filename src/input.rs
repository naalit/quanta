use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct KeyCodes {
    pub forward: u32,
    pub left: u32,
    pub back: u32,
    pub right: u32,

    pub up: u32,
    pub down: u32,
}

pub const DEFAULT_KEY_CODES: KeyCodes = KeyCodes {
    forward: 17, // W
    left: 30,    // A
    back: 31,    // S
    right: 32,   // D

    up: 56,   // Space
    down: 42, // LShift
};
