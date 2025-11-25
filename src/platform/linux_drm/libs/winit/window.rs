// Much of this code is adapted from the `winit` source code.

use super::dpi::{PhysicalPosition, PhysicalSize};
use super::error::ExternalError;
use super::event_loop::EventLoop;

use anyhow::{Context, bail};
use drm::Device as DrmDevice;
use drm::control::{Device as DrmControlDevice, PageFlipFlags};
use gbm::AsRaw;
use glutin::api::egl;
use std::sync::{Arc, Mutex, mpsc};

/// Currently a 32x32 cursor from [https://www.kenney.nl/assets/cursor-pack].
static CURSOR_PNG_BYTES: &[u8] = include_bytes!("Mouse Pointer.png");
static CURSOR_HOTSPOT: (i16, i16) = (4, 4);

struct DrmEglDevice(std::fs::File);

impl std::os::fd::AsFd for DrmEglDevice {
    fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl DrmDevice for DrmEglDevice {}

impl DrmControlDevice for DrmEglDevice {}

pub struct WindowContext<'window> {
    window: &'window Window,
}

impl WindowContext<'_> {
    #[tracing::instrument(skip(self))]
    pub unsafe fn flip_page(self, wait_for_vsync: bool) {
        let window = self.window;
        let window_data = self.window.data.lock().unwrap();
        // Render software cursor.
        if window_data.cursor_visible {
            let span = tracing::trace_span!("render_software_cursor");
            let _enter = span.enter();
            unsafe {
                use crate::client::graphics::gl;
                use gl::array::{TextureCoordPointerType, VertexPointerType};
                use gl::buffer::BufferType;
                use gl::client_state::ClientArrayType;
                use gl::fragment::AlphaTestFunc;
                use gl::matrix::MatrixMode;
                use gl::texture::TexTarget;
                use gl::{EnableComponent, ShapeMode};
                gl::disable(EnableComponent::ScissorTest);
                gl::disable(EnableComponent::FaceCulling);
                gl::disable(EnableComponent::DepthTest);
                gl::enable(EnableComponent::Blending);
                gl::enable(EnableComponent::AlphaTesting);
                gl::enable(EnableComponent::Texture2D);
                gl::fragment::set_alpha_test_function(AlphaTestFunc::Greater, 0.0);
                gl::client_state::disable(ClientArrayType::ColorArray);
                gl::client_state::enable(ClientArrayType::VertexArray);
                gl::client_state::enable(ClientArrayType::TextureCoordArray);
                gl::matrix::switch_mode(MatrixMode::Texture);
                gl::matrix::load_identity();
                gl::matrix::switch_mode(MatrixMode::ModelView);
                gl::matrix::load_identity();
                let size = window_data.window_size;
                let screen_projection_matrix = nalgebra::Orthographic3::new(
                    0.0,
                    size.width as f32,
                    size.height as f32,
                    0.0,
                    0.0,
                    1.0,
                );
                gl::matrix::switch_mode(MatrixMode::Projection);
                gl::matrix::load_f32_matrix(screen_projection_matrix.as_matrix().as_ref());
                gl::texture::bind(TexTarget::Texture2D, Some(window.cursor_gl_texture));
                let (cursor_width, cursor_height) = window.cursor_size;
                let cursor_x = window_data.cursor_pos.x.round() as i16;
                let cursor_y = window_data.cursor_pos.y.round() as i16;
                let min_x = cursor_x - CURSOR_HOTSPOT.0;
                let min_y = cursor_y - CURSOR_HOTSPOT.1;
                let max_x = min_x + cursor_width;
                let max_y = min_y + cursor_height;
                let vertices: [[i16; 2]; 4] = [
                    [min_x, min_y],
                    [max_x, min_y],
                    [min_x, max_y],
                    [max_x, max_y],
                ];
                static UVS: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
                // Unbind any array buffer, if one was bound.
                gl::buffer::bind(BufferType::ArrayBuffer, None);
                gl::array::vertex_pointer(
                    2, // 2D vertices
                    VertexPointerType::I16,
                    0, // Packed stride
                    vertices.as_ptr().addr(),
                );
                gl::array::texture_coord_pointer(
                    2, // 2D texture coordinates
                    TextureCoordPointerType::F32,
                    0, // Packed stride
                    UVS.as_ptr().addr(),
                );
                gl::array::draw(
                    ShapeMode::TriangleStrip,
                    0,
                    vertices.len().try_into().unwrap(),
                );
            }
        }
        // HACK: If we don't wait for all rendering to finish, it seems like frames get presented
        //       before they're finished rendering.
        //       Doing this absolutely murders performance though, so we should work out a way to
        //       get the frame presentation to be delayed until the frame's finished rendering.
        if !wait_for_vsync {
            unsafe {
                crate::client::graphics::gl::finish();
            }
        }
        let (new_front_buffer, new_front_framebuffer) = {
            let span = tracing::trace_span!("get_new_front_buffer");
            let _enter = span.enter();
            {
                let span = tracing::trace_span!("swap_buffers");
                let _enter = span.enter();
                window
                    .egli_display
                    .swap_buffers(&window.egli_surface)
                    .unwrap();
            }
            let mut new_front_buffer = unsafe {
                let span = tracing::trace_span!("lock_gbm_front_buffer");
                let _enter = span.enter();
                window.gbm_surface.lock_front_buffer().unwrap()
            };
            let new_front_framebuffer = new_front_buffer
                .userdata()
                .map(|&handle| handle)
                .unwrap_or_else(
                    // If we haven't made a framebuffer from this buffer yet, make one.
                    || {
                        let span = tracing::trace_span!("register_surface_framebuffer");
                        let _enter = span.enter();
                        let framebuffer = window
                            .gbm_device
                            .add_framebuffer(&new_front_buffer, 24, 32)
                            .context(concat!(
                                "Failed to make a DRM framebuffer",
                                " from current GBM surface front buffer",
                            ))
                            .unwrap();
                        new_front_buffer.set_userdata(framebuffer);
                        framebuffer
                    },
                );
            (new_front_buffer, new_front_framebuffer)
        };
        {
            let span = tracing::trace_span!("send_frame");
            let _enter = span.enter();
            window
                .frame_send_channel
                .send(QueuedFrame {
                    buffer: new_front_buffer,
                    framebuffer: new_front_framebuffer,
                    wait_for_vsync,
                })
                .unwrap();
        }
    }
}

impl Drop for WindowContext<'_> {
    fn drop(&mut self) {
        _ = self.window.egli_display.make_not_current();
    }
}

/// A GBM buffer object, with an associated DRM framebuffer handle.
type GbmSurfaceBuffer = gbm::BufferObject<drm::control::framebuffer::Handle>;

pub struct WindowData {
    pub(super) window_size: PhysicalSize<u32>,
    pub(super) cursor_grab_mode: CursorGrabMode,
    pub(super) cursor_visible: bool,
    pub(super) cursor_pos: PhysicalPosition<f64>,
    old_frame_recv_channel: mpsc::Receiver<GbmSurfaceBuffer>,
}

struct QueuedFrame {
    pub buffer: GbmSurfaceBuffer,
    pub framebuffer: drm::control::framebuffer::Handle,
    pub wait_for_vsync: bool,
}

// NOTE: The drop order matters for these fields (testing shows that we can segfault if we drop the
//       GBM fields before the EGL fields).
pub struct Window {
    pub(super) data: Mutex<WindowData>,
    frame_send_channel: mpsc::Sender<QueuedFrame>,
    cursor_gl_texture: crate::client::graphics::gl::texture::TextureHandle,
    cursor_size: (i16, i16),
    egli_surface: egli::Surface,
    egli_context: egli::Context,
    egli_display: egli::Display,
    drm_crtc: drm::control::crtc::Info,
    gbm_surface: gbm::Surface<drm::control::framebuffer::Handle>,
    gbm_device: gbm::Device<DrmEglDevice>,
}

unsafe impl Send for Window {}

unsafe impl Sync for Window {}

impl Window {
    pub fn new(event_loop: &EventLoop<()>) -> anyhow::Result<Arc<Self>> {
        let egl_devices =
            egl::device::Device::query_devices().context("Error while querying EGL devices")?;
        for egl_device in egl_devices {
            println!();
            // We need the full DRM device (e.g. "/dev/dri/card0") to be able to use the screen for
            // KMS, instead of just the render device (e.g. "/dev/dri/renderD128").
            // Unfortunately this means that the program has to either be running as a user in the
            // "video" group, or has to be running as root.
            // I don't think there's a way around this?
            let Some(drm_path) = egl_device.drm_device_node_path() else {
                continue;
            };
            drop(egl_device);
            let fd = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(drm_path)
                .context("Error while opening DRM device path")?;
            let drm_egl_device = DrmEglDevice(fd);
            let gbm_device = match gbm::Device::new(drm_egl_device) {
                Ok(device) => device,
                Err(err) => {
                    log::warn!(
                        "Couldn't use DRM device {} as GBM device - {err}",
                        drm_path.display(),
                    );
                    continue;
                }
            };
            if let Ok(driver_info) = gbm_device.get_driver() {
                log::info!(
                    "Using DRM/GBM device \"{}\" - {driver_info:?}",
                    drm_path.display(),
                );
            }
            let resource_handles = match gbm_device.resource_handles() {
                Ok(handles) => handles,
                Err(err) => {
                    log::warn!(
                        "Failed to open resource handles on DRM device {} - {err}",
                        drm_path.display(),
                    );
                    continue;
                }
            };
            // Find a CRTC.
            let Some(crtc) = resource_handles
                .crtcs()
                .iter()
                .find_map(|&crtc| gbm_device.get_crtc(crtc).ok())
            else {
                log::warn!(
                    "Failed to find a valid CRTC for DRM device {}",
                    drm_path.display(),
                );
                continue;
            };
            // Find a connected connector with at least one mode available.
            let Some(connector) = resource_handles
                .connectors()
                .iter()
                .flat_map(|&con| gbm_device.get_connector(con, true))
                .find(|con| {
                    con.state() == drm::control::connector::State::Connected
                        && !con.modes().is_empty()
                })
            else {
                log::warn!(
                    "Failed to find connected connector for DRM device {}",
                    drm_path.display(),
                );
                continue;
            };
            // Pick the highest resolution, highest refresh rate mode.
            let mode = *connector
                .modes()
                .iter()
                .max_by_key(|mode| (mode.size(), mode.vrefresh()))
                .expect("No modes found for connector");
            // XXX: DEBUG
            // let mode = *connector.modes().first().unwrap();
            let (display_width, display_height) = mode.size();
            log::info!("Found connector with resolution {display_width}x{display_height}");
            let egl_gbm_native_display = gbm_device.as_raw() as egli::egl::EGLNativeDisplayType;
            let egli_display = egli::Display::from_display_id(egl_gbm_native_display);
            let egli_display = match egli_display {
                Ok(display) => display,
                Err(err) => {
                    log::warn!(
                        "Failed to create EGL display for DRM device {} - {err:?}",
                        drm_path.display(),
                    );
                    continue;
                }
            };
            if let Err(err) = egli_display.initialize_and_get_version() {
                log::warn!(
                    "Failed to initialise EGL display for DRM device {} - {err:?}",
                    drm_path.display(),
                );
                continue;
            }
            let egli_configs = egli_display
                .config_filter()
                .with_depth_size(24)
                .with_renderable_type(egli::RenderableType::OPENGL)
                .with_native_renderable(Some(true))
                .choose_configs();
            let egli_configs = match egli_configs {
                Ok(configs) => configs,
                Err(err) => {
                    log::warn!(
                        "Failed to get EGL configs for DRM device {} - {err:?}",
                        drm_path.display(),
                    );
                    continue;
                }
            };
            let egli_config = egli_configs.into_iter().find(|config| {
                config
                    .surface_type()
                    .unwrap()
                    .contains(egli::SurfaceType::WINDOW)
                    && config.buffer_size().unwrap_or(0) == 32
                    && config.red_size().unwrap_or(0) == 8
                    && config.green_size().unwrap_or(0) == 8
                    && config.blue_size().unwrap_or(0) == 8
            });
            let Some(egli_config) = egli_config else {
                log::warn!("No valid EGL configs for DRM device {}", drm_path.display());
                continue;
            };
            if let Err(err) = egli::egl::bind_api(egli::egl::EGL_OPENGL_API) {
                log::warn!(
                    "Failed to bind OpenGL on DRM device {} - {err:?}",
                    drm_path.display()
                );
                continue;
            }
            // Now that we've bound an OpenGL API, we can load in all the functions we use.
            unsafe {
                crate::client::graphics::gl::load_with(|name| {
                    egli::egl::get_proc_address(name) as *const ()
                });
            }
            let egli_context = match egli_display.create_context(egli_config) {
                Ok(context) => context,
                Err(err) => {
                    log::warn!(
                        "Failed to create EGL context for DRM device {} - {err:?}",
                        drm_path.display(),
                    );
                    continue;
                }
            };
            // Create GBM and EGL surfaces.
            // We want a standard XRGB8888 pixel format.
            let pixel_format = drm::buffer::DrmFourcc::Xrgb8888;
            // Create a GBM surface for the display.
            let gbm_surface = match gbm_device.create_surface::<drm::control::framebuffer::Handle>(
                display_width.into(),
                display_height.into(),
                pixel_format,
                gbm::BufferObjectFlags::SCANOUT | gbm::BufferObjectFlags::RENDERING,
            ) {
                Ok(surface) => surface,
                Err(err) => {
                    log::warn!(
                        "Failed to create a GBM display surface for DRM device {} - {err}",
                        drm_path.display(),
                    );
                    continue;
                }
            };
            let egli_surface = egli_display.create_window_surface(
                egli_config,
                gbm_surface.as_raw() as egli::egl::EGLNativeWindowType,
            );
            let egli_surface = match egli_surface {
                Ok(surface) => surface,
                Err(err) => {
                    let egl_error = egli::egl::get_error();
                    log::warn!(
                        "Failed to create an EGL surface for DRM device {} - {:?}, {:#X}",
                        drm_path.display(),
                        err,
                        egl_error,
                    );
                    continue;
                }
            };
            assert!(gbm_surface.has_free_buffers());
            if let Err(err) = egli_display.make_current(&egli_surface, &egli_surface, &egli_context)
            {
                let egl_error = egli::egl::get_error();
                log::warn!(
                    "Failed to make EGL context current on DRM device {} - {:?}, {:#X}",
                    drm_path.display(),
                    err,
                    egl_error,
                );
                continue;
            }
            // Load a cursor image for software cursor rendering.
            // TODO: Implement a hardware cursor.
            //       Think this needs atomic modesetting (see above).
            let (cursor_gl_texture, cursor_size) = unsafe {
                use crate::client::graphics::gl;
                use gl::texture::{
                    TexEnvMode, TexEnvTarget, TexFilterMode, TexTarget, TexWrapMode,
                    Texture2dFormat, Texture2dTarget, TextureDataType, TextureInternalFormat,
                };
                let cursor_rgba_image =
                    image::load_from_memory_with_format(CURSOR_PNG_BYTES, image::ImageFormat::Png)
                        .context("Error while parsing cursor PNG")?
                        .to_rgba8();
                let [texture] = gl::texture::gen_textures();
                gl::texture::bind(TexTarget::Texture2D, Some(texture));
                gl::texture::set_env_mode(TexEnvTarget::TextureEnv, TexEnvMode::Modulate);
                gl::texture::set_wrap_s(TexTarget::Texture2D, TexWrapMode::Clamp);
                gl::texture::set_wrap_t(TexTarget::Texture2D, TexWrapMode::Clamp);
                gl::texture::set_mag_filter(TexTarget::Texture2D, TexFilterMode::Nearest);
                gl::texture::set_min_filter(TexTarget::Texture2D, TexFilterMode::Nearest);
                let cursor_width: i16 = cursor_rgba_image.width().try_into().unwrap();
                let cursor_height: i16 = cursor_rgba_image.height().try_into().unwrap();
                gl::texture::set_image_2d(
                    Texture2dTarget::Texture,
                    0,
                    TextureInternalFormat::Rgba,
                    cursor_width.into(),
                    cursor_height.into(),
                    0,
                    Texture2dFormat::Rgba,
                    TextureDataType::U8,
                    cursor_rgba_image.as_ptr() as *const (),
                );
                gl::texture::bind(TexTarget::Texture2D, None);
                (texture, (cursor_width, cursor_height))
            };
            // Swap EGL buffers, so we can modeset.
            if let Err(err) = egli_display.swap_buffers(&egli_surface) {
                let egl_error = egli::egl::get_error();
                log::warn!(
                    "Failed to swap EGL buffers on DRM device {} - {:?}, {:#X}",
                    drm_path.display(),
                    err,
                    egl_error,
                );
                continue;
            }
            let mut initial_front_buffer = unsafe {
                match gbm_surface.lock_front_buffer() {
                    Ok(buf) => buf,
                    Err(err) => {
                        log::warn!(
                            "Failed to lock front GBM buffer on DRM device {} - {err}",
                            drm_path.display(),
                        );
                        continue;
                    }
                }
            };
            let initial_front_framebuffer = initial_front_buffer
                .userdata()
                .map(|&handle| handle)
                .unwrap_or_else(
                    // If we haven't made a framebuffer from this buffer yet, make one.
                    || {
                        let framebuffer = gbm_device
                            .add_framebuffer(&initial_front_buffer, 24, 32)
                            .context(concat!(
                                "Failed to make a DRM framebuffer",
                                " from current GBM surface front buffer",
                            ))
                            .unwrap();
                        initial_front_buffer.set_userdata(framebuffer);
                        framebuffer
                    },
                );
            // TODO: Use atomic modesetting instead of legacy modesetting.
            if let Err(err) = gbm_device.set_crtc(
                crtc.handle(),
                Some(initial_front_framebuffer),
                (0, 0),
                &[connector.handle()],
                Some(mode),
            ) {
                log::warn!(
                    "Failed to set CRTC for DRM device {} - {err}",
                    drm_path.display(),
                );
                continue;
            }
            let cursor_pos = PhysicalPosition {
                x: display_width as f64 / 2.0,
                y: display_height as f64 / 2.0,
            };
            let (frame_send_channel, frame_recv_channel) = mpsc::channel();
            let (old_frame_send_channel, old_frame_recv_channel) = mpsc::channel();
            let new_self = Arc::new(Self {
                data: Mutex::new(WindowData {
                    window_size: PhysicalSize {
                        width: display_width.into(),
                        height: display_height.into(),
                    },
                    cursor_grab_mode: CursorGrabMode::None,
                    cursor_visible: true,
                    cursor_pos,
                    old_frame_recv_channel,
                }),
                frame_send_channel,
                cursor_gl_texture,
                cursor_size,
                gbm_device,
                gbm_surface,
                drm_crtc: crtc,
                egli_display,
                egli_context,
                egli_surface,
            });
            // Start the page flipping thread.
            {
                let frame_recv_channel = frame_recv_channel;
                let old_frame_send_channel = old_frame_send_channel;
                let window = new_self.clone();
                let connector_handle = connector.handle();
                let mode = mode;
                let mut current_front_buffer = initial_front_buffer;
                std::thread::spawn(move || {
                    #[cfg(feature = "tracy")]
                    tracing_tracy::client::set_thread_name!("Page Flipping Thread");
                    for frame in frame_recv_channel.iter() {
                        let QueuedFrame {
                            buffer: new_front_buffer,
                            framebuffer: new_front_framebuffer,
                            wait_for_vsync,
                        } = frame;
                        if wait_for_vsync {
                            let span = tracing::trace_span!("vsync_page_flip");
                            let _enter = span.enter();
                            window
                                .gbm_device
                                .page_flip(
                                    window.drm_crtc.handle(),
                                    new_front_framebuffer,
                                    PageFlipFlags::EVENT,
                                    None,
                                )
                                .unwrap();
                            for event in window.gbm_device.receive_events().unwrap() {
                                if matches!(event, drm::control::Event::PageFlip(_)) {
                                    break;
                                }
                            }
                        } else {
                            let span = tracing::trace_span!("no_vsync_set_crtc");
                            let _enter = span.enter();
                            // FIXME: Still seems to be VSync in VirtualBox? Might just be a VM
                            //        issue.
                            // TODO: Figure out async page flipping, previous attempt just crashed
                            //       with "invalid argument".
                            window
                                .gbm_device
                                .set_crtc(
                                    window.drm_crtc.handle(),
                                    Some(new_front_framebuffer),
                                    (0, 0),
                                    &[connector_handle],
                                    Some(mode),
                                )
                                .unwrap();
                        }
                        // Replace the front buffer, send the old one back to the render thread for
                        // freeing.
                        // GBM surface buffers seem to dislike being released on a different thread
                        // than the one they were claimed on, so we have to do this.
                        // This also saves some extra synchronisation between the two threads.
                        let old_front_buffer =
                            core::mem::replace(&mut current_front_buffer, new_front_buffer);
                        old_frame_send_channel.send(old_front_buffer).unwrap();
                    }
                });
            }
            // Attach the window to the provided event loop.
            {
                let mut attached_window = event_loop.attached_window.borrow_mut();
                assert!(
                    attached_window.is_none(),
                    "The provided event loop already has an attached window",
                );
                *attached_window = Some(new_self.clone());
                event_loop
                    .window_send_channel
                    .send(new_self.clone())
                    .unwrap();
            }
            return Ok(new_self);
        }
        bail!("Native DRM window couldn't be created, no suitable devices found")
    }

    #[tracing::instrument(skip_all)]
    pub unsafe fn get_context_blocking(&self) -> WindowContext<'_> {
        let data = self.data.lock().unwrap();
        // Free pending old buffers that the page flipping thread has returned to us.
        for old_buffer in data.old_frame_recv_channel.try_iter() {
            drop(old_buffer);
        }
        // If the surface has no free buffers available, then we need to wait for the page flipping
        // thread to return one to us.
        if !self.gbm_surface.has_free_buffers() {
            let span = tracing::trace_span!("gbm_surface_free_buffer_wait");
            let _enter = span.enter();
            while !self.gbm_surface.has_free_buffers() {
                // Wait for an old buffer to be returned, and release it.
                drop(data.old_frame_recv_channel.recv().unwrap());
            }
        }
        // Start the OpenGL context, claiming the next free buffer on the surface.
        self.egli_display
            .make_current(&self.egli_surface, &self.egli_surface, &self.egli_context)
            .unwrap();
        WindowContext { window: self }
    }
}

impl Window {
    pub fn has_focus(&self) -> bool {
        true
    }

    pub fn id(&self) -> WindowId {
        WindowId(())
    }

    pub fn inner_size(&self) -> PhysicalSize<u32> {
        self.data.lock().unwrap().window_size
    }

    pub fn scale_factor(&self) -> f64 {
        1.0
    }

    pub fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), ExternalError> {
        self.data.lock().unwrap().cursor_grab_mode = mode;
        Ok(())
    }

    pub fn set_cursor_position(
        &self,
        position: PhysicalPosition<i32>,
    ) -> Result<(), ExternalError> {
        self.data.lock().unwrap().cursor_pos = PhysicalPosition {
            x: position.x as f64,
            y: position.y as f64,
        };
        Ok(())
    }

    pub fn set_cursor_visible(&self, visible: bool) {
        self.data.lock().unwrap().cursor_visible = visible;
    }

    pub fn set_fullscreen(&self, _fullscreen: Option<Fullscreen>) {}

    pub fn set_title(&self, _title: &str) {}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId(pub ());

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fullscreen {
    Borderless(Option<usize>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorGrabMode {
    None,
    Confined,
    Locked,
}
