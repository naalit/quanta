use std::sync::Arc;
use vulkano_win::VkSurfaceBuild;
use winit::window::Window as RawWindow;

pub struct Window {
    pub swapchain: Arc<vulkano::swapchain::Swapchain<RawWindow>>,
    images: Vec<Arc<vulkano::image::SwapchainImage<RawWindow>>>,
    surface: Arc<vulkano::swapchain::Surface<RawWindow>>,
    // TODO remove dynamic viewport (https://computergraphics.stackexchange.com/questions/5742/vulkan-best-way-of-updating-pipeline-viewport)
    pub dynamic_state: vulkano::command_buffer::DynamicState,
    pub rpass: Arc<dyn vulkano::framebuffer::RenderPassAbstract + Send + Sync>,
    framebuffers: Vec<Arc<dyn vulkano::framebuffer::FramebufferAbstract + Send + Sync>>,
    size: winit::dpi::PhysicalSize<u32>,
    device: Arc<vulkano::device::Device>,
    pub queue: Arc<vulkano::device::Queue>,
}

pub struct Frame {
    pub image_num: usize,
    pub acquire: vulkano::swapchain::SwapchainAcquireFuture<winit::window::Window>,
    pub framebuffer: Arc<dyn vulkano::framebuffer::FramebufferAbstract + Send + Sync>,
}

impl Window {
    pub fn device(&self) -> Arc<vulkano::device::Device> {
        Arc::clone(&self.device)
    }

    pub fn frame(&self) -> Result<Frame, vulkano::swapchain::AcquireError> {
        // TODO do something with suboptimal
        let (image_num, suboptimal, acquire) =
            vulkano::swapchain::acquire_next_image(Arc::clone(&self.swapchain), None)?;
        let framebuffer = Arc::clone(&self.framebuffers[image_num]);
        Ok(Frame {
            image_num,
            acquire,
            framebuffer,
        })
    }

    pub fn new(title: &str) -> (Self, winit::event_loop::EventLoop<()>) {
        // We can set this to None for release builds
        let layers = vec!["VK_LAYER_LUNARG_standard_validation"];
        let instance =
            vulkano::instance::Instance::new(None, &vulkano_win::required_extensions(), layers)
                .expect("Vulkan is not available on your system!");

        let evloop = winit::event_loop::EventLoop::new();
        let surface = winit::window::WindowBuilder::new()
            .with_fullscreen(Some(winit::window::Fullscreen::Borderless(
                evloop.primary_monitor(),
            )))
            .with_title(title)
            .build_vk_surface(&evloop, Arc::clone(&instance))
            .unwrap();
        let window = surface.window();
        if window.set_cursor_grab(true).is_err() {
            println!("Failed to grab cursor. If you're on wayland, try setting the environment variable WINIT_UNIX_BACKEND=x11.\nLaunching without grabbed cursor...");
        }
        window.set_cursor_visible(false);

        // window.set_fullscreen(Some(window.get_current_monitor()));

        let (device, queue, caps) = {
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
                .expect(
                    "No queue families that support graphics, compute, and drawing to the window",
                );

            let caps = surface.capabilities(device).unwrap();

            let (device, mut queues) = vulkano::device::Device::new(
                device,
                &vulkano::device::Features {
                    fragment_stores_and_atomics: true,
                    ..vulkano::device::Features::none()
                },
                &vulkano::device::DeviceExtensions {
                    khr_swapchain: true,
                    khr_storage_buffer_storage_class: true,
                    ..vulkano::device::DeviceExtensions::none()
                },
                [(queue_family, 0.5)].iter().cloned(),
            )
            .expect("Failed to create device");
            (device, queues.next().unwrap(), caps)
        };

        let (swapchain, images) = {
            let mut usage = caps.supported_usage_flags;
            // Validation layers are complaining
            usage.storage = false;
            let alpha = caps.supported_composite_alpha.iter().next().unwrap();
            let format = caps.supported_formats[0].0;

            let size: (u32, u32) = window.inner_size().into();
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
                vulkano::swapchain::ColorSpace::SrgbNonLinear,
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
                    }
                },
                pass: {
                    color: [color],
                    depth_stencil: {}
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

        (
            Window {
                swapchain,
                images,
                surface: Arc::clone(&surface),
                dynamic_state,
                rpass,
                framebuffers,
                size: window.inner_size(),
                device,
                queue,
            },
            evloop,
        )
    }

    pub fn size(&self) -> (f64, f64) {
        self.size.into()
    }

    /// Returns whether to render this frame. `continue` if it returns false
    pub fn recreate(&mut self) -> bool {
        self.size = self.surface.window().inner_size();
        let size = self.size();
        let size = [size.0 as u32, size.1 as u32];
        let (new_swapchain, new_images) = match self.swapchain.recreate_with_dimensions(size) {
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

    fn resize(
        _device: Arc<vulkano::device::Device>,
        images: &[Arc<vulkano::image::SwapchainImage<RawWindow>>],
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
                        // .add(
                        //     vulkano::image::AttachmentImage::transient(
                        //         Arc::clone(&device),
                        //         size,
                        //         vulkano::format::Format::D32Sfloat,
                        //     )
                        //     .unwrap(),
                        // )
                        // .unwrap()
                        .build()
                        .unwrap(),
                )
                    as Arc<dyn vulkano::framebuffer::FramebufferAbstract + Send + Sync>
            })
            .collect()
    }
}
