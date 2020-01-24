use crate::camera::*;
use crate::client_world::*;
use crate::common::*;
use crate::config::*;
use crate::event::*;
use crate::window::*;

use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBuffer};
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::descriptor::PipelineLayoutAbstract;
use vulkano::framebuffer::Subpass;
use vulkano::pipeline::{
    vertex::BufferlessDefinition, vertex::BufferlessVertices, GraphicsPipeline,
};
use vulkano::sync::GpuFuture;
use vulkano::image::{AttachmentImage, ImageUsage};
use vulkano::sampler::{Filter, MipmapMode, Sampler, SamplerAddressMode};
use vulkano::buffer::{ImmutableBuffer, BufferUsage};

use std::sync::mpsc::*;
use std::sync::Arc;

const BEAM_RES_FAC: u32 = 8;

type BufferlessPipeline = GraphicsPipeline<
    BufferlessDefinition,
    Box<dyn PipelineLayoutAbstract + Send + Sync>,
    Arc<dyn vulkano::framebuffer::RenderPassAbstract + Send + Sync>,
>;

pub struct Client {
    world: (Sender<ClientMessage>, Receiver<ClientMessage>),
    tree_buffer: Arc<vulkano::buffer::DeviceLocalBuffer<[u32]>>,
    window: Window,
    cam: Camera,
    queue: EventQueue,
    pipeline: Arc<BufferlessPipeline>,
}

impl Client {
    pub fn new(queue: EventQueue, conn: Connection, config: Arc<ClientConfig>) -> Self {
        let window = Window::new("Quanta", queue.clone());

        let cam = Camera::new(window.size());

        let (send, r1) = channel();
        let (s1, recv) = channel();

        let world = ClientWorld::new(
            window.device(),
            window.queue.clone(),
            (s1, r1),
            conn,
            Vector3::zeros(),
            config,
        );
        let tree_buffer = world.tree_buffer.clone();
        std::thread::spawn(move || world.run());

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
            world: (send, recv),
            tree_buffer,
            queue,
            pipeline,
        }
    }

    pub fn game_loop(mut self) {
        let size = [
            self.window.size().0 as u32 / BEAM_RES_FAC,
            self.window.size().1 as u32 / BEAM_RES_FAC,
        ];

        let viewport = vulkano::pipeline::viewport::Viewport {
            origin: [0.0, 0.0],
            dimensions: [size[0] as f32, size[1] as f32],
            depth_range: 0.0..1.0,
        };
        let dynamic_state = vulkano::command_buffer::DynamicState {
            viewports: Some(vec![viewport]),
            ..Default::default()
        };

        let beam_image = AttachmentImage::with_usage(
            self.window.device(),
            size,
            vulkano::format::R16Sfloat,
            ImageUsage {
                sampled: true,
                color_attachment: true,
                ..ImageUsage::none()
            },
        )
        .unwrap();

        let rpass = Arc::new(
            vulkano::single_pass_renderpass! {
                self.window.device(),
                attachments: {
                    color: {
                        load: Clear,
                        store: Store,
                        format: vulkano::format::Format::R16Sfloat,
                        samples: 1,
                    }
                },
                pass: {
                    color: [color],
                    depth_stencil: {}
                }
            }
            .unwrap(),
        ) as Arc<dyn vulkano::framebuffer::RenderPassAbstract + Send + Sync>;

        let vs = crate::shaders::Vertex::load(self.window.device()).unwrap();
        let fs = crate::shaders::Beam::load(self.window.device()).unwrap();

        let pipeline = Arc::new(
            GraphicsPipeline::start()
                .vertex_shader(vs.main_entry_point(), ())
                .fragment_shader(fs.main_entry_point(), ())
                .triangle_strip()
                .viewports_dynamic_scissors_irrelevant(1)
                .render_pass(Subpass::from(rpass.clone(), 0).unwrap())
                .build(self.window.device())
                .unwrap(),
        );
        let framebuffer = Arc::new(
            vulkano::framebuffer::Framebuffer::start(Arc::clone(&rpass))
                .add(beam_image.clone())
                .unwrap()
                .build()
                .unwrap(),
        );

        let beam_desc = Arc::new(
            PersistentDescriptorSet::start(
                pipeline.layout().descriptor_set_layout(0).unwrap().clone(),
            )
            .add_buffer(self.tree_buffer.clone())
            .unwrap()
            .build()
            .unwrap(),
        );

        let (mat_buf, future) = ImmutableBuffer::from_iter(crate::material::Material::all().into_iter(), BufferUsage {
            storage_buffer: true,
            ..BufferUsage::none()
        }, self.window.queue.clone()).unwrap();

        let mut future: Box<dyn GpuFuture> = Box::new(future);

        // This shouldn't be necessary
        future
            .then_signal_fence_and_flush()
            .unwrap()
            .wait(None)
            .unwrap();
        future = Box::new(vulkano::sync::now(self.window.device()));

        let desc = Arc::new(
            PersistentDescriptorSet::start(
                self.pipeline
                    .layout()
                    .descriptor_set_layout(0)
                    .unwrap()
                    .clone(),
            )
            .add_buffer(self.tree_buffer.clone())
            .unwrap()
            .add_sampled_image(
                beam_image,
                Sampler::new(
                    self.window.device(),
                    Filter::Nearest,
                    Filter::Nearest,
                    MipmapMode::Nearest,
                    SamplerAddressMode::ClampToEdge,
                    SamplerAddressMode::ClampToEdge,
                    SamplerAddressMode::ClampToEdge,
                    0.0,
                    1.0,
                    0.0,
                    0.0,
                )
                .unwrap(),
            )
            .unwrap()
            .add_buffer(mat_buf)
            .unwrap()
            .build()
            .unwrap(),
        );

        let mut recreate_swapchain = false;
        let clear_values = vec![[0.0, 0.0, 0.0, 1.0].into()];

        let mut timer = stopwatch::Stopwatch::start_new();

        let mut origin = self.cam.pos().map(|x| x % CHUNK_SIZE);
        let mut root_size = 0.0;

        let mut time = 0.0;
        let mut i = 0;
        let mut tot = 0.0;
        loop {
            let delta = timer.elapsed().as_secs_f64();
            time += delta;
            tot += delta;
            i += 1;
            // Average FPS over last 30 frames
            if i % 30 == 0 {
                println!(
                    "Main loop at {:.1} Mpixels/s ({:.1} FPS)",
                    self.window.size().0 * self.window.size().1 * (30.0 / tot) / 1_000_000.0,
                    (30.0 / tot)
                );
                tot = 0.0;
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

            // days / second
            let sun_speed = 1.0 / (24.0 * 60.0); // a day is 24 minutes
            let sun_dir = Vector3::new(
                (time * sun_speed * std::f64::consts::PI * 2.0).sin() as f32,
                (time * sun_speed * std::f64::consts::PI * 2.0).cos() as f32,
                0.1,
            )
            .normalize();

            let pc = self.cam.push(origin.into(), root_size, sun_dir.into());
            let pc_beam = crate::shaders::BeamConstants {
                fov: pc.fov,
                resolution: [
                    (pc.resolution[0] / BEAM_RES_FAC as f32).floor(),
                    (pc.resolution[1] / BEAM_RES_FAC as f32).floor(),
                ],
                camera_pos: pc.camera_pos,
                camera_dir: pc.camera_dir,
                camera_up: pc.camera_up,
                origin: pc.origin,
                root_size,
                _dummy0: pc._dummy0,
                _dummy1: pc._dummy1,
                _dummy2: pc._dummy2,
            };

            let command_buffer = AutoCommandBufferBuilder::primary_one_time_submit(
                self.window.device(),
                self.window.queue.family(),
            )
            .unwrap()
            .begin_render_pass(framebuffer.clone(), false, vec![[0.0].into()])
            .unwrap()
            .draw(
                pipeline.clone(),
                &dynamic_state,
                BufferlessVertices {
                    vertices: 4,
                    instances: 1,
                },
                beam_desc.clone(),
                pc_beam,
            )
            .unwrap()
            .end_render_pass()
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
                .then_swapchain_present(
                    self.window.queue.clone(),
                    self.window.swapchain.clone(),
                    frame.image_num,
                )
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

            self.world
                .0
                .send(ClientMessage::PlayerMove(self.cam.pos()))
                .unwrap();
            match self.world.1.try_recv() {
                Ok(ClientMessage::Submit(cmd, o, r)) => {
                    // This shouldn't be necessary
                    future
                        .then_signal_fence_and_flush()
                        .unwrap()
                        .wait(None)
                        .unwrap();
                    future = Box::new(cmd.execute(self.window.queue.clone()).unwrap());
                    // future = Box::new(future.then_execute(self.window.queue.clone(), cmd).unwrap());
                    origin = o;
                    root_size = r;
                    // This shouldn't be necessary either
                    future
                        .then_signal_fence_and_flush()
                        .unwrap()
                        .wait(None)
                        .unwrap();
                    future = Box::new(vulkano::sync::now(self.window.device()));
                }
                Err(TryRecvError::Empty) => (),
                _ => panic!("Unknown message from client_world, or it panicked"),
            }

            self.window.update();
            self.cam.update(delta);
            // self.world.update(self.cam.pos(), self.window.device(), self.window.queue.clone(), &mut future);
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

        self.world.0.send(ClientMessage::Done).unwrap();
        while let Ok(x) = self.world.1.recv() {
            if let ClientMessage::Done = x {
                break;
            }
        }
    }
}
