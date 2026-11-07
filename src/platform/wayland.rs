// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Wayland layer-shell host.
//!
//! GTK previously provided three things: a layer-shell window, Pango text and a
//! GStreamer video sink. This module replaces only the window/surface part with
//! `smithay-client-toolkit` + `wgpu`; text uses `cosmic-text` and video is
//! handled by the media module.

use std::num::NonZeroU32;
use std::ptr::NonNull;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;
use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_registry,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
    },
};
use wayland_client::{
    Connection, Proxy, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_output, wl_surface},
};

use crate::app::Runtime;
use crate::css::Viewport;
use crate::render::GpuRenderer;

pub fn run(config_path: Option<std::path::PathBuf>) -> Result<()> {
    let connection = Connection::connect_to_env()
        .context("cannot connect to Wayland; widdgyy requires a wlr-layer-shell compositor")?;
    let (globals, event_queue) = registry_queue_init(&connection)?;
    let qh = event_queue.handle();

    let compositor_state =
        CompositorState::bind(&globals, &qh).context("wl_compositor is not available")?;
    let layer_shell = LayerShell::bind(&globals, &qh)
        .context("wlr-layer-shell is not available on this compositor")?;
    let output_state = OutputState::new(&globals, &qh);

    let surface = compositor_state.create_surface(&qh);
    let layer =
        layer_shell.create_layer_surface(&qh, surface, Layer::Background, Some("widdgyy"), None);
    layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    layer.set_exclusive_zone(-1);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer.set_size(0, 0);
    layer.commit();

    let runtime = match config_path {
        Some(path) => Runtime::new(path, Viewport::default())?,
        None => Runtime::from_default_config(Viewport::default())?,
    };

    let mut instance_descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    instance_descriptor.backends =
        wgpu::Backends::from_env().unwrap_or(wgpu::Backends::VULKAN | wgpu::Backends::GL);
    instance_descriptor.flags = wgpu::InstanceFlags::from_build_config().with_env();
    let instance = wgpu::Instance::new(instance_descriptor);

    let display_ptr = connection.backend().display_ptr();
    let surface_ptr = layer.wl_surface().id().as_ptr();
    let raw_display_handle = RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
        NonNull::new(display_ptr as *mut _).context("Wayland display pointer is null")?,
    ));
    let raw_window_handle = RawWindowHandle::Wayland(WaylandWindowHandle::new(
        NonNull::new(surface_ptr as *mut _).context("Wayland surface pointer is null")?,
    ));
    let wgpu_surface = unsafe {
        instance
            .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(raw_display_handle),
                raw_window_handle,
            })
            .context("creating wgpu surface from the layer surface")?
    };

    let width = runtime.viewport().width as u32;
    let height = runtime.viewport().height as u32;
    let gpu = GpuRenderer::new(&instance, wgpu_surface, width, height)?;

    let mut state = WaylandState {
        registry_state: RegistryState::new(&globals),
        output_state,
        _layer: layer,
        gpu: Some(gpu),
        runtime,
        instance,
        width,
        height,
        exit: false,
        last_frame: Instant::now(),
    };

    let mut event_loop: EventLoop<WaylandState> = EventLoop::try_new()?;
    WaylandSource::new(connection, event_queue)
        .insert(event_loop.handle())
        .context("integrating the Wayland event queue")?;

    loop {
        let timeout = if state.runtime.needs_animation() {
            Duration::from_millis(16)
        } else {
            Duration::from_millis(200)
        };
        event_loop
            .dispatch(timeout, &mut state)
            .context("dispatching Wayland events")?;
        if state.exit {
            break;
        }
        let now = Instant::now();
        if state.runtime.tick(now) {
            state.render();
        }
    }
    Ok(())
}

struct WaylandState {
    registry_state: RegistryState,
    output_state: OutputState,
    _layer: LayerSurface,
    gpu: Option<GpuRenderer>,
    runtime: Runtime,
    instance: wgpu::Instance,
    width: u32,
    height: u32,
    exit: bool,
    last_frame: Instant,
}

impl WaylandState {
    fn render(&mut self) {
        let plan = self.runtime.prepare_frame();
        let Some(gpu) = &mut self.gpu else {
            return;
        };
        if plan.fast {
            if plan.overlay_updated
                && let Err(error) = gpu.render_overlay(self.runtime.overlay_scene())
            {
                log::error!("overlay render failed: {error:#}");
            }
            if plan.overlay_updated
                && !self.runtime.shadow_items().is_empty()
                && let Err(error) = gpu.render_shadow_mask(self.runtime.shadow_mask())
            {
                log::error!("shadow mask render failed: {error:#}");
            }
            if plan.shadows_updated
                && let Err(error) = gpu.render_shadows(self.runtime.shadow_items())
            {
                log::error!("shadow render failed: {error:#}");
            }
            if let Err(error) = gpu.render_composite(plan.video.as_ref()) {
                log::error!("video composite failed: {error:#}");
            }
        } else if plan.filtered {
            if let Err(error) = gpu.render_layers(self.runtime.render_layers()) {
                log::error!("filtered render failed: {error:#}");
            }
        } else if let Err(error) = gpu.render(self.runtime.overlay_scene()) {
            log::error!("render failed: {error:#}");
        }
        self.last_frame = Instant::now();
    }
}

impl Drop for WaylandState {
    fn drop(&mut self) {
        // The wgpu surface references the wl_surface owned by the layer. Drop
        // it first so no texture is alive when the layer goes away.
        self.gpu.take();
        let _ = &self.instance;
    }
}

impl CompositorHandler for WaylandState {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for WaylandState {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for WaylandState {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let width = NonZeroU32::new(configure.new_size.0)
            .map(NonZeroU32::get)
            .unwrap_or(self.width);
        let height = NonZeroU32::new(configure.new_size.1)
            .map(NonZeroU32::get)
            .unwrap_or(self.height);
        self.width = width;
        self.height = height;
        self.runtime.resize(width, height);
        if let Some(gpu) = &mut self.gpu {
            gpu.resize(width, height);
        }
        self.render();
    }
}

impl ProvidesRegistryState for WaylandState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}

delegate_registry!(WaylandState);
smithay_client_toolkit::delegate_dispatch2!(WaylandState);
