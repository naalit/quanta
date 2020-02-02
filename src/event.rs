use crate::camera::Camera;
use crate::client::Client;
use crate::common::*;
/// The event system for both client and server
use crate::config::*;
use crate::window::Window;
use std::sync::Arc;
use std::time::Duration;
use winit::event as we;
use winit::event::{DeviceEvent, WindowEvent};
use winit::event_loop::*;

#[derive(Default)]
pub struct Time {
    pub total: Duration,
    pub delta: Duration,
}

#[derive(Default)]
pub struct FrameNum(pub usize);

pub fn run_client_loop(conn: Connection, config: Arc<ClientConfig>) -> ! {
    let (window, evloop) = Window::new("Quanta");

    let mut w = World::new();

    let mut e: EventChannel<Event> = EventChannel::new();

    let cam = Camera::new(window.size());
    let (client, client_world) = Client::new(&window, &cam, conn, config, &mut e);

    w.insert(e);
    w.insert(cam);
    w.insert(window);
    w.insert(crate::world::World::new());

    let mut d = DispatcherBuilder::new()
        .with(client, "", &[])
        .with(client_world, "", &[])
        .build();

    let timer = stopwatch::Stopwatch::start_new();
    let mut i = 0;
    let mut time = Duration::from_secs(0);

    evloop.run(move |event, _target, _flow| {
        let mut e: specs::shred::FetchMut<EventChannel<Event>> = w.fetch_mut();

        match event {
            we::Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                e.single_write(Event::Resize(size.width.into(), size.height.into()));
            }
            we::Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                e.single_write(Event::Quit);
                *_flow = ControlFlow::Exit;
            }
            we::Event::DeviceEvent { event, .. } => {
                // println!("Device event_a: {:?}", event);
                match event {
                    DeviceEvent::MouseMotion { delta } => {
                        e.single_write(Event::Mouse(delta.0, delta.1));
                    }
                    DeviceEvent::Key(we::KeyboardInput {
                        scancode,
                        state: we::ElementState::Pressed,
                        ..
                    }) => {
                        e.single_write(Event::KeyPressed(scancode));
                    }
                    DeviceEvent::Key(we::KeyboardInput {
                        scancode,
                        state: we::ElementState::Released,
                        ..
                    }) => {
                        e.single_write(Event::KeyReleased(scancode));
                    }
                    DeviceEvent::Button {
                        state: we::ElementState::Pressed,
                        button,
                    } => {
                        e.single_write(Event::Button(button));
                    }
                    _ => {}
                }
            }
            we::Event::RedrawEventsCleared => {
                drop(e);

                let cur = timer.elapsed();
                let delta = cur - time;
                time = cur;
                i += 1;
                w.insert(Time { total: time, delta });
                w.insert(FrameNum(i));

                d.dispatch_par(&w);
                w.maintain();

                // Keep the cursor in the window
                // window.surface
                //     .window()
                //     .set_cursor_position(winit::dpi::LogicalPosition::new(0.0, 0.0))
                //     .unwrap();
                //
                // if let Some(size) = resize {
                //     event_queue.push(Event::Resize(size.width.into(), size.height.into()));
                // }
                // if !ret {
                //     event_queue.push(Event::Quit);
                //     *_control_flow = winit::event_loop::ControlFlow::Exit;
                // }
            }
            _ => {}
        }
    })
}

pub enum Event {
    /// The player moved
    PlayerMove(Vector3<f32>),
    Submit(
        Once<(
            vulkano::command_buffer::AutoCommandBuffer,
            Vector3<f32>,
            f32,
            HashMap<Vector3<i32>, (usize, usize)>,
        )>,
    ),
    /// A press of a mouse button with this id
    Button(u32),
    /// A key press with this scan code
    KeyPressed(u32),
    KeyReleased(u32),
    /// A change in mouse position
    Mouse(f64, f64),
    /// A window resize, with new width and height
    Resize(f64, f64),
    /// The application needs to close, so do any destruction necessary
    Quit,
}
