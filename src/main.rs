use crate::config::*;
use std::fs::File;
use std::io::Write;

use std::sync::Arc;

mod camera;
mod chunks;
mod client;
mod client_world;
mod common;
mod config;
mod event;
mod input;
mod server;
mod shaders;
mod terrain;
mod window;
mod world;
mod material;
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

    let client = client::Client::new(queue, conn_client, client_config);
    client.game_loop();
}
