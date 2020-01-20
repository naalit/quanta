use std::io::Write;
use crate::config::*;
use std::fs::File;
use vulkano::buffer::CpuBufferPool;
use vulkano::command_buffer::AutoCommandBufferBuilder;
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::framebuffer::Subpass;
use vulkano::image::{Dimensions, ImageUsage, StorageImage};
use vulkano::pipeline::{vertex::BufferlessVertices, GraphicsPipeline};
use vulkano::sampler::{Filter, MipmapMode, Sampler, SamplerAddressMode};
use vulkano::sync::GpuFuture;

use std::sync::Arc;

mod camera;
mod chunks;
mod common;
mod event;
mod terrain;
mod server;
mod world;
mod shaders;
mod window;
mod config;
mod input;
mod client_world;
mod client;
use common::*;

pub const APP_INFO: app_dirs2::AppInfo = app_dirs2::AppInfo {
    name: "quanta",
    author: "Lorxu",
};


fn main() {
    let queue = event::EventQueue::new();

    let mut config_file =
        app_dirs2::app_root(app_dirs2::AppDataType::UserConfig, &APP_INFO).unwrap();
    config_file.push("config.ron");
    let client_config = if config_file.exists() {
        ron::de::from_reader(File::open(config_file).unwrap()).expect("bad config file")
    } else {
        let c = ClientConfig {
            keycodes: crate::input::DEFAULT_KEY_CODES,
            game_config: Arc::new(GameConfig {
                draw_chunks: 16,
                batch_size: 64,
                save_chunks: true,
            }),
        };
        let s = ron::ser::to_string(&c).unwrap();
        let mut f = File::create(config_file).unwrap();
        writeln!(f, "{}", s).unwrap();
        c
    };
    let client_config = Arc::new(client_config);

    let config = Arc::clone(&client_config.game_config);

    let (conn_client, conn_server) = Connection::local();
    std::thread::spawn(move || {
        let mut server = server::Server::new(config);
        server.join(conn_server, Vector3::zeros());
        server.run();
    });

    let client = client::Client::new(queue, conn_client);
    client.game_loop();
}
