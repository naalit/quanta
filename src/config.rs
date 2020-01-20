use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Config for both the client and server
#[derive(Deserialize, Serialize)]
pub struct GameConfig {
    pub draw_chunks: usize, // The number of chunks to draw in every direction
    pub batch_size: usize,  // The number of chunks to load per batch
    pub save_chunks: bool,
}

/// Config for just the client
#[derive(Deserialize, Serialize)]
pub struct ClientConfig {
    pub keycodes: crate::input::KeyCodes,

    pub game_config: Arc<GameConfig>,
}
