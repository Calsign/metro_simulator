use std::time::Instant;

use egui_wgpu_backend::{RenderPass, ScreenDescriptor};
use egui_winit_platform::{Platform, PlatformDescriptor};
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget};
use winit::window::{Window, WindowBuilder};

use crate::app::App;

pub struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    platform: Platform,
    render_pass: RenderPass,
    last_frame_start: Instant,
}

impl State {
    fn new(window: &Window) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window) };
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .unwrap();

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                features: wgpu::Features::empty(),
                limits: wgpu::Limits::default(),
                label: None,
            },
            None,
        ))
        .unwrap();

        assert!(
            size.width > 0 && size.height > 0,
            "width and height must be non-zero: {}, {}",
            size.width,
            size.height
        );

        let format = surface.get_preferred_format(&adapter).unwrap();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
        };
        surface.configure(&device, &config);

        let platform = Platform::new(PlatformDescriptor {
            physical_width: size.width as u32,
            physical_height: size.height as u32,
            scale_factor: window.scale_factor(),
            font_definitions: egui::FontDefinitions::default(),
            style: Default::default(),
        });

        let render_pass = RenderPass::new(&device, format, 1);

        Self {
            surface,
            device,
            queue,
            config,
            size,
            platform,
            render_pass,
            last_frame_start: Instant::now(),
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        false
    }

    fn update(&mut self) {}

    fn render(&mut self, window: &Window, app: &mut App) -> Result<(), wgpu::SurfaceError> {
        let frame_start = Instant::now();
        let new_frame_rate = 1.0 / (frame_start - self.last_frame_start).as_secs_f64();
        // smooth so you can read it
        // TODO: this might not actually be correct?
        app.diagnostics.frame_rate = app.diagnostics.frame_rate * 0.5 + new_frame_rate * 0.5;
        self.last_frame_start = frame_start;

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.platform.begin_frame();

        app.draw(&self.platform.context());

        let full_output = self.platform.end_frame(Some(&window));
        let paint_jobs = self.platform.context().tessellate(full_output.shapes);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render encoder"),
            });

        let screen_descriptor = ScreenDescriptor {
            physical_width: self.config.width,
            physical_height: self.config.height,
            scale_factor: window.scale_factor() as f32,
        };

        self.render_pass
            .add_textures(&self.device, &self.queue, &full_output.textures_delta)
            .unwrap();
        self.render_pass
            .update_buffers(&self.device, &self.queue, &paint_jobs, &screen_descriptor);

        self.render_pass
            .execute(
                &mut encoder,
                &view,
                &paint_jobs,
                &screen_descriptor,
                Some(wgpu::Color::BLACK),
            )
            .unwrap();
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

fn create_state<T>(event_loop: &EventLoopWindowTarget<T>) -> (State, Window) {
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    let state = State::new(&window);
    (state, window)
}

pub fn bootstrap(mut app: App, wait_for_resume: bool) {
    match env_logger::try_init() {
        Ok(()) => (),
        Err(err) => eprintln!("Failed to initialize logger: {}", err),
    }

    let event_loop = EventLoop::new();
    let mut initialized = match wait_for_resume {
        true => None,
        false => Some(create_state(&event_loop)),
    };

    println!("starting native event loop");

    let start_time = Instant::now();

    event_loop.run(
        move |event, event_loop, control_flow| match &mut initialized {
            None => match event {
                Event::Resumed => match &initialized {
                    None => {
                        println!("received resume; transitioning to initialized");
                        initialized = Some(create_state(&event_loop));
                        *control_flow = ControlFlow::Poll;
                    }
                    Some(event) => (),
                },
                event => (),
            },
            Some((state, window)) => {
                state.platform.handle_event(&event);

                match event {
                    Event::Suspended => {
                        println!("received suspend; transitioning to uninitialized");
                        initialized = None;
                        *control_flow = ControlFlow::Wait;
                    }
                    Event::WindowEvent {
                        ref event,
                        window_id,
                    } if window_id == window.id() && !state.input(event) => match event {
                        WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                        WindowEvent::Resized(physical_size) => state.resize(*physical_size),
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            state.resize(**new_inner_size)
                        }
                        _ => (),
                    },
                    Event::RedrawRequested(window_id) if window_id == window.id() => {
                        state
                            .platform
                            .update_time(start_time.elapsed().as_secs_f64());

                        app.update();
                        state.update();
                        match state.render(window, &mut app) {
                            Ok(_) => (),
                            Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                            Err(wgpu::SurfaceError::Outdated) => {
                                // apparently this can happen on Windows when the app is minimized
                            }
                            Err(wgpu::SurfaceError::OutOfMemory) => {
                                *control_flow = ControlFlow::Exit
                            }
                            Err(e) => eprintln!("Render failed: {:?}", e),
                        }
                    }
                    Event::MainEventsCleared => window.request_redraw(),
                    _ => (),
                }
            }
        },
    );
}
