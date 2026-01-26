# hypercubed

A (very WIP) Minecraft client implementation, written in Rust.

Currently connects to Minecraft: Java Edition 1.21.1 servers.

## Main Goals

- Provide a more stutter-free experience than the vanilla Java client.
- Run on a wider range of hardware, including older devices the vanilla client no longer supports.
- In the future, provide some level of support for [NeoForge](https://neoforged.net) mods.

## Screenshots

![hypercubed on Windows connected to a vanilla 1.21.1 server](images/Windows%20Vulkan.jpg)
*hypercubed running on Windows, using the `winit` platform and the Vulkan graphics backend.*

![hypercubed on an Early 2006 iMac Core Duo connected to a vanilla 1.21.1 server](images/iMac%20Core%20Duo.JPEG)
*hypercubed running on an Early 2006 iMac Core Duo, using the Linux DRM platform and the OpenGL
graphics backend.*

## Graphics Backends

- Vulkan - Requires Vulkan 1.2 with support for multi-draw indirect.
- [`wgpu`](https://wgpu.rs) - Not functional on WebGL2, but support is planned.
- OpenGL - Requires OpenGL 1.2 + `GL_ARB_vertex_buffer_object`.
- Software - Based around Larrabee-style rasterisation, works best with wide SIMD instruction sets.

Variants of these backends with lower requirements are planned, as are variants which make use of
newer hardware features.

## Platforms

- [`winit`](https://github.com/rust-windowing/winit) - Intended to be the "default" backend for
Windows, Linux and macOS.
- Linux KMS/DRM - Uses a mix of DRM, GBM, EGL, [`libseat`](https://github.com/kennylevinsen/seatd),
and [`libinput`](https://wayland.freedesktop.org/libinput/doc/latest/index.html), and is intended
for cases where running an X server or a Wayland compositor is impractical.  
Currently only supports the OpenGL graphics backend.