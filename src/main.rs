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
mod common;
mod event;
mod gen;
mod shaders;
mod window;
use common::*;

fn main() {
    let queue = event::EventQueue::new();

    let mut win = window::Window::new("Quanta", queue.clone());

    let vs = shaders::Vertex::load(win.device()).unwrap();
    let fs = shaders::Fragment::load(win.device()).unwrap();

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
        vulkano::format::R8Unorm,
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

    let mut future: Box<dyn GpuFuture> = Box::new(vulkano::sync::now(win.device()));

    let block_buf = CpuBufferPool::upload(win.device());
    for x in 0..16 {
        for z in 0..16 {
            for y in 0..16 {
                let v = Vector3::new(x - 8, y - 8, z - 8);
                let c = gen::gen_chunk(v);
                let b = block_buf.chunk(c).unwrap();
                let cmd = AutoCommandBufferBuilder::primary_one_time_submit(
                    win.device(),
                    win.queue.family(),
                )
                .unwrap()
                .copy_buffer_to_image_dimensions(
                    b,
                    blocks.clone(),
                    [x as u32 * 16, y as u32 * 16, z as u32 * 16],
                    [16, 16, 16],
                    0,
                    1,
                    0,
                )
                .unwrap()
                .build()
                .unwrap();
                future = Box::new(future.then_execute(win.queue.clone(), cmd).unwrap());
            }
            future
                .then_signal_fence_and_flush()
                .unwrap()
                .wait(None)
                .unwrap();
            future = Box::new(vulkano::sync::now(win.device()));
        }
    }

    let chunk_buf = CpuBufferPool::upload(win.device());
    let v: Vec<_> = (0..16)
        .flat_map(|x| (0..16).map(move |y| (x, y)))
        .flat_map(|(x, y)| (0..16).map(move |z| (x, y, z)))
        .flat_map(|(x, y, z)| {
            vec![
                (x * 16) as u16 + 1,
                (y * 16) as u16 + 1,
                (z * 16) as u16 + 1,
                0,
            ]
        })
        .collect();
    let b = chunk_buf.chunk(v).unwrap();
    let cmd = AutoCommandBufferBuilder::primary_one_time_submit(win.device(), win.queue.family())
        .unwrap()
        // Overwrite the whole first cascade
        .copy_buffer_to_image_dimensions(b, chunks.clone(), [0, 0, 0], [16, 16, 16], 0, 1, 0)
        .unwrap()
        .build()
        .unwrap();
    future = Box::new(future.then_execute(win.queue.clone(), cmd).unwrap());
    future
        .then_signal_fence_and_flush()
        .unwrap()
        .wait(None)
        .unwrap();
    future = Box::new(vulkano::sync::now(win.device()));

    let desc = Arc::new(
        PersistentDescriptorSet::start(pipeline.clone(), 0)
            .add_sampled_image(chunks.clone(), s_chunks)
            .unwrap()
            .add_sampled_image(blocks.clone(), s_blocks)
            .unwrap()
            .build()
            .unwrap(),
    );

    let mut cam = camera::Camera::new(win.size());

    let mut recreate_swapchain = false;
    let clear_values = vec![[0.0, 0.0, 0.0, 1.0].into(), 1.0.into()];

    let mut timer = stopwatch::Stopwatch::start_new();

    let mut i = 0;
    loop {
        let delta = timer.elapsed().as_secs_f64();
        i = (i + 1) % 30;
        if i == 0 {
            println!(
                "Main loop at {} Mrays/s",
                win.size().0 * win.size().1 * (1.0 / delta) / 1_000_000.0
            );
            println!("Camera at {:?}", cam.pos);
        }
        timer.restart();

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

        let pc = cam.push();

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
        cam.update(delta);
        let mut done = false;
        queue.poll(|ev| {
            cam.process(&ev);
            match ev {
                event::Event::Resize(_, _) => recreate_swapchain = true,
                event::Event::Quit => done = true,
                _ => {}
            }
        });
        if done {
            break;
        }
    }
}
