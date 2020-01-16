use vulkano::buffer::CpuBufferPool;
use vulkano::command_buffer::AutoCommandBufferBuilder;
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::framebuffer::Subpass;
use vulkano::image::{Dimensions, ImageUsage, StorageImage};
use vulkano::pipeline::{vertex::BufferlessVertices, GraphicsPipeline};
use vulkano::sampler::{Filter, MipmapMode, Sampler, SamplerAddressMode};
use vulkano::sync::GpuFuture;

use std::sync::Arc;

mod event;
mod window;

fn main() {
    let queue = event::EventQueue::new();

    let mut win = window::Window::new("Quanta", queue.clone());

    mod vs {
        vulkano_shaders::shader! {
            ty: "vertex",
            path: "src/blank.vert"
        }
    }

    mod fs {
        vulkano_shaders::shader! {
            ty: "fragment",
            path: "src/main.frag"
        }
    }

    let vs = vs::Shader::load(win.device()).unwrap();
    let fs = fs::Shader::load(win.device()).unwrap();

    let pipeline = Arc::new(
        GraphicsPipeline::start()
            .vertex_shader(vs.main_entry_point(), ())
            .fragment_shader(fs.main_entry_point(), ())
            .triangle_strip()
            .viewports_dynamic_scissors_irrelevant(1)
            .render_pass(Subpass::from(win.rpass.clone(), 0).unwrap())
            .build(win.device())
            .unwrap(),
    );

    // Most implementations don't support 16 bit RGB, but do support RGBA
    let chunks = StorageImage::with_usage(
        win.device(),
        Dimensions::Dim3d {
            width: 16,
            height: 16,
            depth: 64,
        },
        vulkano::format::R16G16B16A16Uint,
        ImageUsage {
            transfer_destination: true,
            sampled: true,
            ..ImageUsage::none()
        },
        win.device().active_queue_families(),
    )
    .unwrap();
    let s_chunks = Sampler::new(
        win.device(),
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
    .unwrap();
    let blocks = StorageImage::with_usage(
        win.device(),
        Dimensions::Dim3d {
            width: 256,
            height: 256,
            depth: 1024,
        },
        vulkano::format::R16G16B16A16Uint,
        ImageUsage {
            transfer_destination: true,
            sampled: true,
            ..ImageUsage::none()
        },
        win.device().active_queue_families(),
    )
    .unwrap();
    let s_blocks = Sampler::new(
        win.device(),
        Filter::Linear,
        Filter::Linear,
        MipmapMode::Nearest,
        SamplerAddressMode::ClampToEdge,
        SamplerAddressMode::ClampToEdge,
        SamplerAddressMode::ClampToEdge,
        0.0,
        1.0,
        0.0,
        0.0,
    )
    .unwrap();

    let desc = Arc::new(
        PersistentDescriptorSet::start(pipeline.clone(), 0)
            .add_sampled_image(chunks.clone(), s_chunks)
            .unwrap()
            .add_sampled_image(blocks.clone(), s_blocks)
            .unwrap()
            .build()
            .unwrap(),
    );

    let mut recreate_swapchain = false;
    let clear_values = vec![[0.0, 0.0, 0.0, 1.0].into(), 1.0.into()];
    let mut future: Box<dyn GpuFuture> = Box::new(vulkano::sync::now(win.device()));

    loop {
        future.cleanup_finished();
        if recreate_swapchain {
            if !win.recreate() {
                continue;
            }
            recreate_swapchain = false;
        }

        let frame = match win.frame() {
            Ok(r) => r,
            Err(vulkano::swapchain::AcquireError::OutOfDate) => {
                recreate_swapchain = true;
                continue;
            }
            Err(err) => panic!("{:?}", err),
        };

        let res = win.size();

        let pc = fs::ty::PushConstants {
            resolution: [res.0 as f32, res.1 as f32],
            camera_pos: [0.0, 0.0, 0.0],
            camera_dir: [0.0, 0.0, 1.0],
            camera_up: [0.0, 1.0, 0.0],
            _dummy0: [0; 8],
            _dummy1: [0; 4],
            _dummy2: [0; 4],
        };

        let command_buffer =
            AutoCommandBufferBuilder::primary_one_time_submit(win.device(), win.queue.family())
                .unwrap()
                .begin_render_pass(frame.framebuffer, false, clear_values.clone())
                .unwrap()
                .draw(
                    pipeline.clone(),
                    &win.dynamic_state,
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
            .then_execute(win.queue.clone(), command_buffer)
            .unwrap()
            .then_swapchain_present(win.queue.clone(), win.swapchain.clone(), frame.image_num)
            .then_signal_fence_and_flush();
        match f {
            Ok(f) => {
                future = Box::new(f) as Box<_>;
            }
            Err(vulkano::sync::FlushError::OutOfDate) => {
                recreate_swapchain = true;
                future = Box::new(vulkano::sync::now(win.device())) as Box<_>;
            }
            Err(err) => {
                // We'll keep going, it's probably not a big deal
                println!("{:?}", err);
                future = Box::new(vulkano::sync::now(win.device())) as Box<_>;
            }
        }

        win.update();
        let mut done = false;
        queue.poll(|ev| match ev {
            event::Event::Resize(_, _) => recreate_swapchain = true,
            event::Event::Quit => done = true,
            _ => {}
        });
        if done {
            break;
        }
    }
}
