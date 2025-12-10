mod renderer;
mod engine;
mod algorithms;

use anyhow::Result;
use engine::Engine;
use renderer::Renderer;
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, Event, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

fn main() -> Result<()> {
    pollster::block_on(run())
}

async fn run() -> Result<()> {
    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("Algo Native - wgpu")
        .with_inner_size(PhysicalSize::new(1280, 720))
        .build(&event_loop)?;

    // Leak the window to satisfy the surface lifetime; acceptable for single-window app
    let window: &'static _ = Box::leak(Box::new(window));

    let mut renderer = Renderer::new(window).await?;
    // Fewer bars for a focused scene
    let mut engine = Engine::new(12);

    let mut paused = false;
    let window_ref = window;
    let mut last_time = std::time::Instant::now();

    Ok(event_loop.run(move |event, target| {
        let window = window_ref;
        match event {
            Event::WindowEvent { event, .. } => {
                renderer.handle_input(window, &event);
                match event {
                WindowEvent::CloseRequested => target.exit(),
                WindowEvent::Resized(size) => {
                    renderer.resize(size);
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    if event.state == ElementState::Released {
                        match event.physical_key {
                            PhysicalKey::Code(KeyCode::Space) => paused = !paused,
                            PhysicalKey::Code(KeyCode::KeyR) => engine.reset(),
                            _ => {}
                        }
                    }
                }
                WindowEvent::RedrawRequested => {
                    let now = std::time::Instant::now();
                    let dt = now - last_time;
                    last_time = now;

                    if !paused {
                        engine.step(dt);
                    }
                    let (bars, max_val) = engine.bars();
                    if let Err(err) = renderer.render(bars, max_val, engine.comparisons, engine.operations, engine.time_elapsed, engine.current_memory, engine.peak_memory, engine.current_animation.clone(), &engine.temp_array, dt, window) {
                        eprintln!("Render error: {err:?}");
                        target.exit();
                    }
                }
                _ => {}
            }},
            Event::AboutToWait => {
                window.request_redraw();
            }
            _ => {}
        }
    })?)
}
