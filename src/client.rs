use crate::camera::*;
use crate::client_world::*;
use crate::common::*;
use crate::config::*;
use crate::event::*;
use crate::window::*;
use vulkano::command_buffer::DynamicState;

use std::sync::Arc;
use vulkano::buffer::{BufferUsage, CpuBufferPool, ImmutableBuffer};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBuffer};
use vulkano::descriptor::descriptor_set::{DescriptorSet, PersistentDescriptorSet};
use vulkano::descriptor::PipelineLayoutAbstract;
use vulkano::framebuffer::Subpass;
use vulkano::image::{AttachmentImage, ImageUsage};
use vulkano::pipeline::{
    vertex::BufferlessDefinition, vertex::BufferlessVertices, GraphicsPipeline,
};
use vulkano::sampler::{Filter, MipmapMode, Sampler, SamplerAddressMode};
use vulkano::sync::GpuFuture;

use specs::World;

const BEAM_RES_FAC: u32 = 8;

type BufferlessPipeline = GraphicsPipeline<
    BufferlessDefinition,
    Box<dyn PipelineLayoutAbstract + Send + Sync>,
    Arc<dyn vulkano::framebuffer::RenderPassAbstract + Send + Sync>,
>;

pub struct Client {
    tree_buffer: Arc<vulkano::buffer::DeviceLocalBuffer<[u32]>>,
    pipeline: Arc<BufferlessPipeline>,
    desc: Arc<dyn DescriptorSet + Send + Sync>,
    beam_pipeline: Arc<BufferlessPipeline>,
    beam_framebuffer: Arc<dyn vulkano::framebuffer::FramebufferAbstract + Send + Sync>,
    beam_state: DynamicState,
    beam_desc: Arc<dyn DescriptorSet + Send + Sync>,
    future: Box<dyn GpuFuture + Send + Sync>,
    pool: CpuBufferPool<u32>,
    recreate_swapchain: bool,
    origin: Vector3<f32>,
    root_size: f32,
    chunk_slots: HashMap<Vector3<i32>, (usize, usize)>,
    reader_id: ReaderId<Event>,
    tot: f64,
}

#[derive(SystemData)]
pub struct ClientData<'a> {
    time: Read<'a, Time>,
    i: Read<'a, FrameNum>,
    win: WriteExpect<'a, Window>,
    cam: WriteExpect<'a, Camera>,
    world: WriteExpect<'a, crate::world::World>,
    channel: Write<'a, EventChannel<Event>>,
}

impl<'a> System<'a> for Client {
    type SystemData = ClientData<'a>;

    fn run(&mut self, data: Self::SystemData) {
        let ClientData {
            time,
            i,
            mut win,
            mut cam,
            mut world,
            mut channel,
        } = data;

        let size = win.size();

        let delta = time.delta.as_secs_f64();
        self.tot += delta;
        let time = time.total.as_secs_f64();

        // Average FPS over last 30 frames
        if i.0 % 30 == 0 {
            println!(
                "Main loop at {:.1} Mpixels/s ({:.1} FPS)",
                size.0 * size.1 * (30.0 / self.tot) / 1_000_000.0,
                (30.0 / self.tot)
            );
            self.tot = 0.0;
            println!("Camera at {:?}", cam.pos);
        }

        self.future.cleanup_finished();
        if self.recreate_swapchain {
            if !win.recreate() {
                // continue
                return;
            }
            self.recreate_swapchain = false;
        }

        let frame = match win.frame() {
            Ok(r) => r,
            Err(vulkano::swapchain::AcquireError::OutOfDate) => {
                self.recreate_swapchain = true;
                // continue
                return;
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

        let pc = cam.push(self.origin.into(), self.root_size, sun_dir.into());
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
            root_size: self.root_size,
            _dummy0: pc._dummy0,
            _dummy1: pc._dummy1,
            _dummy2: pc._dummy2,
        };

        let command_buffer =
            AutoCommandBufferBuilder::primary_one_time_submit(win.device(), win.queue.family())
                .unwrap()
                .begin_render_pass(self.beam_framebuffer.clone(), false, vec![[0.0].into()])
                .unwrap()
                .draw(
                    self.beam_pipeline.clone(),
                    &self.beam_state,
                    BufferlessVertices {
                        vertices: 4,
                        instances: 1,
                    },
                    self.beam_desc.clone(),
                    pc_beam,
                )
                .unwrap()
                .end_render_pass()
                .unwrap()
                .begin_render_pass(frame.framebuffer, false, vec![[0.0, 0.0, 0.0, 1.0].into()])
                .unwrap()
                .draw(
                    self.pipeline.clone(),
                    &win.dynamic_state,
                    BufferlessVertices {
                        vertices: 4,
                        instances: 1,
                    },
                    self.desc.clone(),
                    pc,
                )
                .unwrap()
                .end_render_pass()
                .unwrap()
                .build()
                .unwrap();

        let mut f: Box<dyn GpuFuture + Send + Sync> = Box::new(vulkano::sync::now(win.device()));
        std::mem::swap(&mut f, &mut self.future);
        let f = f
            .join(frame.acquire)
            .then_execute(win.queue.clone(), command_buffer)
            .unwrap()
            .then_swapchain_present(win.queue.clone(), win.swapchain.clone(), frame.image_num)
            .then_signal_fence_and_flush();

        match f {
            Ok(f) => {
                self.future = Box::new(f) as Box<_>;
            }
            Err(vulkano::sync::FlushError::OutOfDate) => {
                self.recreate_swapchain = true;
                self.future = Box::new(vulkano::sync::now(win.device())) as Box<_>;
            }
            Err(err) => {
                // We'll keep going, it's probably not a big deal
                println!("{:?}", err);
                self.future = Box::new(vulkano::sync::now(win.device())) as Box<_>;
            }
        }

        channel.single_write(Event::PlayerMove(cam.pos()));

        cam.update(delta);

        for ev in channel.read(&mut self.reader_id) {
            cam.process(&ev);

            match ev {
                Event::Submit(once) => {
                    let (cmd, origin, root_size, chunk_slots) =
                        once.get().expect("Somebody took the stuff out of Submit!");

                    // This shouldn't be necessary
                    let mut f: Box<dyn GpuFuture + Send + Sync> =
                        Box::new(vulkano::sync::now(win.device()));
                    std::mem::swap(&mut f, &mut self.future);
                    f.then_signal_fence_and_flush().unwrap().wait(None).unwrap();
                    self.future = Box::new(cmd.execute(win.queue.clone()).unwrap());
                    // future = Box::new(future.then_execute(self.window.queue.clone(), cmd).unwrap());

                    self.origin = origin;
                    self.root_size = root_size;
                    self.chunk_slots = chunk_slots;
                }
                Event::Resize(_, _) => self.recreate_swapchain = true,
                Event::Quit => (),
                // Left-click
                Event::Button(1) => {
                    println!("You clicked!");
                    let cast = world.raycast(
                        cam.pos(),
                        cam.dir.map(|x| if x.abs() < 0.0001 { 0.0001 } else { x }),
                        12.0,
                    );
                    println!("Found {:?}", cast);
                    if let Some(RayCast { t, .. }) = cast {
                        let pos = cam.pos() + cam.dir * (t[0] + 0.05);
                        world.set_block(pos, Material::Air);
                        let loc = world_to_chunk(pos);
                        let chunk = self
                            .pool
                            .chunk(world.chunk(loc).unwrap().0.clone())
                            .unwrap();

                        let slot = self.chunk_slots.get(&loc).unwrap();
                        let view = vulkano::buffer::BufferSlice::from_typed_buffer_access(
                            self.tree_buffer.clone(),
                        )
                        .slice(slot.0..slot.1)
                        .unwrap();
                        let cmd =
                            AutoCommandBufferBuilder::primary(win.device(), win.queue.family())
                                .unwrap()
                                .copy_buffer(chunk, view)
                                .unwrap()
                                .build()
                                .unwrap();

                        // This shouldn't be necessary
                        let mut f: Box<dyn GpuFuture + Send + Sync> =
                            Box::new(vulkano::sync::now(win.device()));
                        std::mem::swap(&mut f, &mut self.future);
                        f.then_signal_fence_and_flush().unwrap().wait(None).unwrap();

                        self.future = Box::new(cmd.execute(win.queue.clone()).unwrap());
                    }
                }
                _ => {}
            }
        }
    }
}

impl Client {
    pub fn new(
        window: &Window,
        cam: &Camera,
        conn: Connection,
        config: Arc<ClientConfig>,
        events: &mut EventChannel<Event>,
    ) -> (Self, ClientWorld) {
        let c = ClientWorld::new(
            window.device(),
            window.queue.clone(),
            conn,
            Vector3::zeros(),
            config,
            events.register_reader(),
        );
        let tree_buffer = c.tree_buffer.clone();

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

        let size = [
            window.size().0 as u32 / BEAM_RES_FAC,
            window.size().1 as u32 / BEAM_RES_FAC,
        ];

        let viewport = vulkano::pipeline::viewport::Viewport {
            origin: [0.0, 0.0],
            dimensions: [size[0] as f32, size[1] as f32],
            depth_range: 0.0..1.0,
        };
        let beam_state = DynamicState {
            viewports: Some(vec![viewport]),
            ..Default::default()
        };

        let beam_image = AttachmentImage::with_usage(
            window.device(),
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
                window.device(),
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

        let fs_beam = crate::shaders::Beam::load(window.device()).unwrap();

        let beam_pipeline = Arc::new(
            GraphicsPipeline::start()
                .vertex_shader(vs.main_entry_point(), ())
                .fragment_shader(fs_beam.main_entry_point(), ())
                .triangle_strip()
                .viewports_dynamic_scissors_irrelevant(1)
                .render_pass(Subpass::from(rpass.clone(), 0).unwrap())
                .build(window.device())
                .unwrap(),
        );
        let beam_framebuffer = Arc::new(
            vulkano::framebuffer::Framebuffer::start(Arc::clone(&rpass))
                .add(beam_image.clone())
                .unwrap()
                .build()
                .unwrap(),
        );

        let beam_desc = Arc::new(
            PersistentDescriptorSet::start(
                beam_pipeline
                    .layout()
                    .descriptor_set_layout(0)
                    .unwrap()
                    .clone(),
            )
            .add_buffer(tree_buffer.clone())
            .unwrap()
            .build()
            .unwrap(),
        );

        let (mat_buf, future) = ImmutableBuffer::from_iter(
            crate::material::Material::all().into_iter(),
            BufferUsage {
                storage_buffer: true,
                ..BufferUsage::none()
            },
            window.queue.clone(),
        )
        .unwrap();

        let mut future: Box<dyn GpuFuture + Send + Sync> = Box::new(future);

        // This shouldn't be necessary
        future
            .then_signal_fence_and_flush()
            .unwrap()
            .wait(None)
            .unwrap();
        future = Box::new(vulkano::sync::now(window.device()));

        let desc = Arc::new(
            PersistentDescriptorSet::start(
                pipeline.layout().descriptor_set_layout(0).unwrap().clone(),
            )
            .add_buffer(tree_buffer.clone())
            .unwrap()
            .add_sampled_image(
                beam_image,
                Sampler::new(
                    window.device(),
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

        let pool = vulkano::buffer::CpuBufferPool::upload(window.device());

        (
            Client {
                tree_buffer,
                pipeline,
                desc,
                beam_pipeline,
                beam_framebuffer,
                beam_state,
                beam_desc,
                future,
                pool,
                chunk_slots: HashMap::new(),
                reader_id: events.register_reader(),
                origin: cam.pos().map(|x| x % CHUNK_SIZE),
                root_size: 0.0,
                recreate_swapchain: false,
                tot: 0.0,
            },
            c,
        )
    }
}
