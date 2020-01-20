use crate::common::*;
use crate::client_world::*;
use crate::config::*;
use crate::window::*;
use crate::event::*;
use crate::camera::*;

use vulkano::descriptor::PipelineLayoutAbstract;
use vulkano::buffer::CpuBufferPool;
use vulkano::command_buffer::AutoCommandBufferBuilder;
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::framebuffer::Subpass;
use vulkano::image::{Dimensions, ImageUsage, StorageImage};
use vulkano::pipeline::{vertex::BufferlessVertices, vertex::BufferlessDefinition, GraphicsPipeline};
use vulkano::sampler::{Filter, MipmapMode, Sampler, SamplerAddressMode};
use vulkano::sync::GpuFuture;

use std::sync::Arc;

pub struct Client {
    world: ClientWorld,
    window: Window,
    cam: Camera,
    queue: EventQueue,
    pipeline: Arc<GraphicsPipeline<BufferlessDefinition, Box<dyn PipelineLayoutAbstract + Send + Sync>, Arc<dyn vulkano::framebuffer::RenderPassAbstract + Send + Sync>>>,
}

impl Client {
    pub fn new(queue: EventQueue, conn: Connection) -> Self {
        let window = Window::new("Quanta", queue.clone());

        let cam = Camera::new(window.size());

        let world = ClientWorld::new(window.device(), conn, Vector3::zeros());

        let vs = crate::shaders::Vertex::load(window.device()).unwrap();
        let fs = crate::shaders::Fragment::load(window.device()).unwrap();

        let pipeline = Arc::new(
            GraphicsPipeline::start()
                .vertex_shader(vs.main_entry_point(), ())
                .fragment_shader(fs.main_entry_point(), ())
                .triangle_strip()
                .viewports_dynamic_scissors_irrelevant(1)
                .render_pass(Subpass::from(window.rpass.clone(), 0).unwrap())
                .build(window.device())
                .unwrap(),
        );

        Client {
            window,
            cam,
            world,
            queue,
            pipeline,
        }
    }

    pub fn game_loop(mut self) {
        let mut future: Box<dyn GpuFuture> = Box::new(vulkano::sync::now(self.window.device()));

        let desc = Arc::new(
            PersistentDescriptorSet::start(self.pipeline.clone(), 0)
                .add_buffer(self.world.tree_buffer.clone())
                .unwrap()
                .build()
                .unwrap(),
        );

        let mut recreate_swapchain = false;
        let clear_values = vec![[0.0, 0.0, 0.0, 1.0].into()];

        let mut timer = stopwatch::Stopwatch::start_new();

        let mut i = 0;
        loop {
            let delta = timer.elapsed().as_secs_f64();
            i = (i + 1) % 30;
            if i == 0 {
                println!(
                    "Main loop at {} Mpixels/s",
                    self.window.size().0 * self.window.size().1 * (1.0 / delta) / 1_000_000.0
                );
                println!("Camera at {:?}", self.cam.pos);
            }
            timer.restart();

            future.cleanup_finished();
            if recreate_swapchain {
                if !self.window.recreate() {
                    continue;
                }
                recreate_swapchain = false;
            }

            let frame = match self.window.frame() {
                Ok(r) => r,
                Err(vulkano::swapchain::AcquireError::OutOfDate) => {
                    recreate_swapchain = true;
                    continue;
                }
                Err(err) => panic!("{:?}", err),
            };

            let pc = self.cam.push(self.world.origin(), self.world.root_size);

            let command_buffer =
                AutoCommandBufferBuilder::primary_one_time_submit(self.window.device(), self.window.queue.family())
                    .unwrap()
                    .begin_render_pass(frame.framebuffer, false, clear_values.clone())
                    .unwrap()
                    .draw(
                        self.pipeline.clone(),
                        &self.window.dynamic_state,
                        BufferlessVertices {
                            vertices: 4,
                            instances: 1,
                        },
                        desc.clone(),
                        pc,
                    )
                    .unwrap()
                    .end_render_pass()
                    .unwrap()
                    .build()
                    .unwrap();
            let f = future
                .join(frame.acquire)
                .then_execute(self.window.queue.clone(), command_buffer)
                .unwrap()
                .then_swapchain_present(self.window.queue.clone(), self.window.swapchain.clone(), frame.image_num)
                .then_signal_fence_and_flush();

            match f {
                Ok(f) => {
                    future = Box::new(f) as Box<_>;
                }
                Err(vulkano::sync::FlushError::OutOfDate) => {
                    recreate_swapchain = true;
                    future = Box::new(vulkano::sync::now(self.window.device())) as Box<_>;
                }
                Err(err) => {
                    // We'll keep going, it's probably not a big deal
                    println!("{:?}", err);
                    future = Box::new(vulkano::sync::now(self.window.device())) as Box<_>;
                }
            }

            self.window.update();
            self.cam.update(delta);
            self.world.update(self.cam.pos(), self.window.device(), self.window.queue.clone(), &mut future);
            let mut done = false;
            self.queue.clone().poll(|ev| {
                self.cam.process(&ev);
                match ev {
                    Event::Resize(_, _) => recreate_swapchain = true,
                    Event::Quit => done = true,
                    _ => {}
                }
            });
            if done {
                break;
            }
        }
    }
}
