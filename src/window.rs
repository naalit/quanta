use crate::event::*;
use std::sync::{Arc, RwLock};
use vulkano_win::VkSurfaceBuild;

pub struct Window {
    pub swapchain: Arc<vulkano::swapchain::Swapchain<winit::Window>>,
    images: Vec<Arc<vulkano::image::SwapchainImage<winit::Window>>>,
    surface: Arc<vulkano::swapchain::Surface<winit::Window>>,
    // TODO remove dynamic viewport (https://computergraphics.stackexchange.com/questions/5742/vulkan-best-way-of-updating-pipeline-viewport)
    pub dynamic_state: vulkano::command_buffer::DynamicState,
    pub rpass: Arc<dyn vulkano::framebuffer::RenderPassAbstract + Send + Sync>,
    framebuffers: Vec<Arc<dyn vulkano::framebuffer::FramebufferAbstract + Send + Sync>>,
    last_frame: Arc<RwLock<Box<dyn vulkano::sync::GpuFuture>>>,
    pub evloop: winit::EventsLoop,
    size: winit::dpi::PhysicalSize,
    device: Arc<vulkano::device::Device>,
    pub queue: Arc<vulkano::device::Queue>,
    event_queue: EventQueue,
}

pub struct Frame {
    pub image_num: usize,
    pub acquire: vulkano::swapchain::SwapchainAcquireFuture<winit::Window>,
    pub framebuffer: Arc<dyn vulkano::framebuffer::FramebufferAbstract + Send + Sync>,
}

impl Window {
    pub fn device(&self) -> Arc<vulkano::device::Device> {
        Arc::clone(&self.device)
    }
    pub fn frame(&self) -> Result<Frame, vulkano::swapchain::AcquireError> {
        let (image_num, acquire) =
            vulkano::swapchain::acquire_next_image(Arc::clone(&self.swapchain), None)?;
        let framebuffer = Arc::clone(&self.framebuffers[image_num]);
        Ok(Frame {
            image_num,
            acquire,
            framebuffer,
        })
    }
    pub fn new(title: &str, event_queue: EventQueue) -> Self {
        let instance =
            vulkano::instance::Instance::new(None, &vulkano_win::required_extensions(), None)
                .expect("Vulkan is not available on your system!");

        let evloop = winit::EventsLoop::new();
        let surface = winit::WindowBuilder::new()
            .with_title(title)
            .build_vk_surface(&evloop, Arc::clone(&instance))
            .unwrap();
        let window = surface.window();
        if window.grab_cursor(true).is_err() {
            println!("Failed to grab cursor. If you're on wayland, try setting the environment variable WINIT_UNIX_BACKEND=x11.\nLaunching without grabbed cursor...");
        }
        window.hide_cursor(true);
        // window.set_fullscreen(Some(window.get_current_monitor()));

        let (device, queue, caps) =
            {
                let mut devices = vulkano::instance::PhysicalDevice::enumerate(&instance);
                let device = if devices.len() == 0 {
                    panic!("No hardware on your system supports Vulkan!")
                } else if devices.len() == 1 {
                    devices.next().unwrap()
                } else {
                    use std::io::Write;

                    println!("Available devices: \n");
                    for (i, device) in devices.enumerate() {
                        println!("\t{}. {}\n", i, device.name());
                    }
                    print!("Please select a device by index: ");
                    std::io::stdout().flush().unwrap();

                    let mut s = String::new();
                    std::io::stdin().read_line(&mut s).unwrap();
                    let i: usize = s.trim().parse().expect("That's not a valid number");
                    vulkano::instance::PhysicalDevice::from_index(&instance, i)
                        .expect("No device with that index")
                };

                println!("Selected device: {}", device.name());

                // TODO if no families support compute, pick a graphics one and disable graphics options that require compute shaders
                // TODO separate graphics, transfer, and maybe compute queues
                let queue_family = device
            .queue_families()
            .find(|&q| {
                q.supports_graphics()
                    && q.supports_compute()
                    && surface.is_supported(q).unwrap_or(false)
            })
            .expect("No queue families that support graphics, compute, and drawing to the window");

                let caps = surface.capabilities(device).unwrap();

                let (device, mut queues) = vulkano::device::Device::new(
                    device,
                    &vulkano::device::Features::none(),
                    &vulkano::device::DeviceExtensions {
                        khr_swapchain: true,
                        ..vulkano::device::DeviceExtensions::none()
                    },
                    [(queue_family, 0.5)].iter().cloned(),
                )
                .expect("Failed to create device");
                (device, queues.next().unwrap(), caps)
            };

        let (swapchain, images) = {
            let usage = caps.supported_usage_flags;
            let alpha = caps.supported_composite_alpha.iter().next().unwrap();
            let format = caps.supported_formats[0].0;

            let size: (u32, u32) = window
                .get_inner_size()
                .unwrap()
                .to_physical(window.get_hidpi_factor())
                .into();
            let size = [size.0, size.1];
            let size = caps.current_extent.unwrap_or(size);
            vulkano::swapchain::Swapchain::new(
                Arc::clone(&device),
                Arc::clone(&surface),
                caps.min_image_count,
                format,
                size,
                1,
                usage,
                &queue,
                vulkano::swapchain::SurfaceTransform::Identity,
                alpha,
                vulkano::swapchain::PresentMode::Fifo,
                true,
                None,
            )
            .unwrap()
        };

        let mut dynamic_state = vulkano::command_buffer::DynamicState::default();

        let rpass = Arc::new(
            vulkano::single_pass_renderpass! {
                device.clone(),
                attachments: {
                    color: {
                        load: Clear,
                        store: Store,
                        format: swapchain.format(),
                        samples: 1,
                    },
                    depth: {
                        load: Clear,
                        store: DontCare,
                        format: vulkano::format::Format::D32Sfloat,
                        samples: 1,
                    }
                },
                pass: {
                    color: [color],
                    depth_stencil: {depth}
                }
            }
            .unwrap(),
        ) as Arc<dyn vulkano::framebuffer::RenderPassAbstract + Send + Sync>;

        let framebuffers = Window::resize(
            Arc::clone(&device),
            &images,
            Arc::clone(&rpass),
            &mut dynamic_state,
        );

        Window {
            swapchain,
            images,
            surface: Arc::clone(&surface),
            dynamic_state,
            rpass,
            framebuffers,
            last_frame: Arc::new(RwLock::new(Box::new(vulkano::sync::now(Arc::clone(
                &device,
            ))))),
            evloop,
            size: window
                .get_inner_size()
                .unwrap()
                .to_physical(window.get_hidpi_factor()),
            device,
            queue,
            event_queue,
        }
    }

    pub fn aspect(&self) -> f32 {
        self.size.width as f32 / self.size.height as f32
    }

    pub fn size(&self) -> (f64, f64) {
        self.size.into()
    }

    /// Returns whether to render this frame. `continue` if it returns false
    pub fn recreate(&mut self) -> bool {
        self.size = self
            .surface
            .window()
            .get_inner_size()
            .unwrap()
            .to_physical(self.surface.window().get_hidpi_factor());
        let size = self.size();
        let size = [size.0 as u32, size.1 as u32];
        let (new_swapchain, new_images) = match self.swapchain.recreate_with_dimension(size) {
            Ok(r) => r,
            // Apparently this error sometimes happens when the window is being resized, just try again
            Err(vulkano::swapchain::SwapchainCreationError::UnsupportedDimensions) => return false,
            Err(err) => panic!("Swapchain recreation error: {:?}", err),
        };

        self.swapchain = new_swapchain;
        self.framebuffers = Window::resize(
            self.device(),
            &new_images,
            Arc::clone(&self.rpass),
            &mut self.dynamic_state,
        );
        true
    }

    pub fn update(&mut self) {
        let mut ret = true;
        let mut resize = None;
        let mut device_events = Vec::new();
        self.evloop.poll_events(|event| {
            // println!("EVENT: {:?}", event);
            match event {
                winit::Event::WindowEvent {
                    event: winit::WindowEvent::Resized(size),
                    ..
                } => {
                    // println!("RESIZE");
                    resize = Some(size);
                }
                winit::Event::WindowEvent {
                    event: winit::WindowEvent::CloseRequested,
                    ..
                } => {
                    ret = false;
                }
                winit::Event::DeviceEvent { event, .. } => {
                    // println!("Device event_a: {:?}", event);
                    device_events.push(event);
                }
                _ => {}
            }
        });

        for event in device_events {
            // println!("Device event: {:?}", event);
            match event {
                winit::DeviceEvent::MouseMotion { delta } => {
                    self.event_queue.push(Event::Mouse(delta.0, delta.1));
                }
                winit::DeviceEvent::Key(winit::KeyboardInput {
                    scancode,
                    state: winit::ElementState::Pressed,
                    ..
                }) => {
                    self.event_queue.push(Event::KeyPressed(scancode));
                }
                winit::DeviceEvent::Key(winit::KeyboardInput {
                    scancode,
                    state: winit::ElementState::Released,
                    ..
                }) => {
                    self.event_queue.push(Event::KeyReleased(scancode));
                }
                winit::DeviceEvent::Button {
                    state: winit::ElementState::Pressed,
                    button,
                } => {
                    self.event_queue.push(Event::Button(button));
                }
                _ => {}
            }
        }

        if let Some(size) = resize {
            let size = size.to_physical(self.surface.window().get_hidpi_factor());
            self.event_queue
                .push(Event::Resize(size.width, size.height));
        }
        if !ret {
            self.event_queue.push(Event::Quit);
        }
    }

    fn resize(
        device: Arc<vulkano::device::Device>,
        images: &[Arc<vulkano::image::SwapchainImage<winit::Window>>],
        rpass: Arc<dyn vulkano::framebuffer::RenderPassAbstract + Send + Sync>,
        dynamic_state: &mut vulkano::command_buffer::DynamicState,
    ) -> Vec<Arc<dyn vulkano::framebuffer::FramebufferAbstract + Send + Sync>> {
        let size = images[0].dimensions();
        let viewport = vulkano::pipeline::viewport::Viewport {
            origin: [0.0, 0.0],
            dimensions: [size[0] as f32, size[1] as f32],
            depth_range: 0.0..1.0,
        };
        dynamic_state.viewports = Some(vec![viewport]);
        images
            .iter()
            .map(|image| {
                Arc::new(
                    vulkano::framebuffer::Framebuffer::start(Arc::clone(&rpass))
                        .add(Arc::clone(&image))
                        .unwrap()
                        .add(
                            vulkano::image::AttachmentImage::transient(
                                Arc::clone(&device),
                                size,
                                vulkano::format::Format::D32Sfloat,
                            )
                            .unwrap(),
                        )
                        .unwrap()
                        .build()
                        .unwrap(),
                )
                    as Arc<dyn vulkano::framebuffer::FramebufferAbstract + Send + Sync>
            })
            .collect()
    }
}
