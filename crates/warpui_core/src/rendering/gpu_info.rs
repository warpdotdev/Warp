//! Module containing types to report GPU information that can be useful debug purposes.

use std::fmt::{Display, Formatter};

/// Function called when a GPU device is first selected upon constructing a window.
pub type OnGPUDeviceSelected = dyn Fn(GPUDeviceInfo) + 'static + Send + Sync;

/// Physical GPU device types.
/// This is a direct fork of wgpu's `DeviceType` struct. However, we redefine it to avoid a direct
/// dependency on wgpu in cases where we don't rely on the wgpu rendering backend.
///
/// See <https://docs.rs/wgpu/latest/wgpu/enum.DeviceType.html> for more details.
#[derive(Debug, Copy, Clone)]
pub enum GPUDeviceType {
    /// Other or Unknown.
    Other,
    /// Integrated GPU with shared CPU/GPU memory.
    IntegratedGpu,
    /// Discrete GPU with separate CPU/GPU memory.
    DiscreteGpu,
    /// Virtual / Hosted.
    VirtualGpu,
    /// Cpu / Software Rendering.
    Cpu,
}

/// The GPU backend that is being renderer to.
/// This is a direct fork of wgpu's `Backend` struct. However, we redefine it to avoid a direct
/// dependency on wgpu in cases where we don't rely on the wgpu rendering backend.
///
/// See <https://docs.rs/wgpu/latest/wgpu/enum.Backend.html> for more details.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GPUBackend {
    /// Dummy backend, used for testing.
    Empty,
    /// Vulkan API
    Vulkan,
    /// Metal API (Apple platforms)
    Metal,
    /// Direct3D-12 (Windows)
    Dx12,
    /// OpenGL ES-3 (Linux, Android)
    Gl,
    /// WebGPU in the browser
    BrowserWebGpu,
}

impl Display for GPUBackend {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            GPUBackend::Empty => write!(f, "Empty"),
            GPUBackend::Vulkan => write!(f, "Vulkan"),
            GPUBackend::Metal => write!(f, "Metal"),
            GPUBackend::Dx12 => write!(f, "Dx12"),
            GPUBackend::Gl => write!(f, "Gl"),
            GPUBackend::BrowserWebGpu => write!(f, "BrowserWebGpu"),
        }
    }
}

/// Information about the GPU device a given window is rendering to.
#[derive(Debug)]
pub struct GPUDeviceInfo {
    /// The type of the device we are rendering to (e.g. integrated vs discrete).
    pub device_type: GPUDeviceType,
    /// The name of the GPU _device_ we are rendering to.
    pub device_name: String,
    /// The name of the GPU _driver_ that the OS is using to connect to the given GPU device.
    pub driver_name: String,
    /// Any additional information about the driver that the OS is using to connect to the given
    /// GPU device.
    pub driver_info: String,
    /// The backend (e.g. Metal vs Vulkan vs OpenGL) we using when rendering.
    pub backend: GPUBackend,
    /// Whether the device exposes dual-source blending. Reported here so
    /// the application can update its [`super::Config::lcd_subpixel_supported`]
    /// flag when a window's renderer is created; the LCD subpixel glyph
    /// path requires dual-source blending to composite per-channel coverage.
    pub supports_dual_source_blending: bool,
    /// Whether the rendering surface composites with a non-opaque alpha
    /// (translucent windows / alpha-aware compositors). Reported alongside
    /// [`Self::supports_dual_source_blending`] so the application can
    /// populate [`super::Config::surface_is_transparent`]; LCD subpixel
    /// rendering must fall back to grayscale on transparent surfaces or
    /// the per-channel coverage corrupts the compositor's alpha channel.
    pub surface_is_transparent: bool,
}

impl Display for GPUDeviceType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            GPUDeviceType::Other => write!(f, "Other"),
            GPUDeviceType::IntegratedGpu => write!(f, "Integrated"),
            GPUDeviceType::DiscreteGpu => write!(f, "Discrete"),
            GPUDeviceType::VirtualGpu => write!(f, "Virtual"),
            GPUDeviceType::Cpu => write!(f, "Cpu"),
        }
    }
}
