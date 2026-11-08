//! Memory-mapped Linux framebuffer (`/dev/fbN`).

use std::{
    fs::{File, OpenOptions},
    io,
    os::fd::AsRawFd,
    path::Path,
    ptr::NonNull,
};

use slint::platform::software_renderer::{Rgb565Pixel, SoftwareRenderer};

use crate::pixel::{Bgrx8888, PixelFormat, Rgbx8888};

const FBIOGET_VSCREENINFO: u32 = 0x4600;
const FBIOGET_FSCREENINFO: u32 = 0x4602;

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct FbBitfield {
    offset: u32,
    length: u32,
    msb_right: u32,
}

/// `struct fb_var_screeninfo` from `<linux/fb.h>`.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct FbVarScreeninfo {
    xres: u32,
    yres: u32,
    xres_virtual: u32,
    yres_virtual: u32,
    xoffset: u32,
    yoffset: u32,
    bits_per_pixel: u32,
    grayscale: u32,
    red: FbBitfield,
    green: FbBitfield,
    blue: FbBitfield,
    transp: FbBitfield,
    nonstd: u32,
    activate: u32,
    height: u32,
    width: u32,
    accel_flags: u32,
    pixclock: u32,
    left_margin: u32,
    right_margin: u32,
    upper_margin: u32,
    lower_margin: u32,
    hsync_len: u32,
    vsync_len: u32,
    sync: u32,
    vmode: u32,
    rotate: u32,
    colorspace: u32,
    reserved: [u32; 4],
}

/// `struct fb_fix_screeninfo` from `<linux/fb.h>`.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct FbFixScreeninfo {
    id: [u8; 16],
    smem_start: libc::c_ulong,
    smem_len: u32,
    type_: u32,
    type_aux: u32,
    visual: u32,
    xpanstep: u16,
    ypanstep: u16,
    ywrapstep: u16,
    line_length: u32,
    mmio_start: libc::c_ulong,
    mmio_len: u32,
    accel: u32,
    capabilities: u16,
    reserved: [u16; 2],
}

pub struct Framebuffer {
    _file: File,
    map: NonNull<u8>,
    map_len: usize,
    /// Byte offset of the visible page inside the mapping.
    offset: usize,
    pub width: u32,
    pub height: u32,
    stride_bytes: usize,
    pub format: PixelFormat,
}

impl Framebuffer {
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        let fd = file.as_raw_fd();

        let mut var = FbVarScreeninfo::default();
        let mut fix = FbFixScreeninfo::default();
        // SAFETY: the structs match the kernel ABI and outlive the calls.
        unsafe {
            if libc::ioctl(fd, FBIOGET_VSCREENINFO as _, &mut var) < 0 {
                return Err(io::Error::last_os_error());
            }
            if libc::ioctl(fd, FBIOGET_FSCREENINFO as _, &mut fix) < 0 {
                return Err(io::Error::last_os_error());
            }
        }

        let format = PixelFormat::detect(var.bits_per_pixel, var.red.offset, var.blue.offset)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::Unsupported,
                    format!(
                        "unsupported framebuffer format: {} bpp, red@{}, blue@{}",
                        var.bits_per_pixel, var.red.offset, var.blue.offset
                    ),
                )
            })?;

        let stride_bytes = fix.line_length as usize;
        let map_len = fix.smem_len as usize;
        let offset =
            var.yoffset as usize * stride_bytes + var.xoffset as usize * format.bytes_per_pixel();
        if offset + stride_bytes * var.yres as usize > map_len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "visible framebuffer area exceeds mapped memory",
            ));
        }

        // SAFETY: mapping a device fd we own; checked for MAP_FAILED below.
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                map_len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }

        tracing::info!(
            device = %path.display(),
            width = var.xres,
            height = var.yres,
            bpp = var.bits_per_pixel,
            stride = stride_bytes,
            ?format,
            "framebuffer opened"
        );

        Ok(Self {
            _file: file,
            map: NonNull::new(ptr.cast()).expect("mmap returned null"),
            map_len,
            offset,
            width: var.xres,
            height: var.yres,
            stride_bytes,
            format,
        })
    }

    /// Render dirty regions straight into video memory.
    pub fn render(&mut self, renderer: &SoftwareRenderer) {
        let bpp = self.format.bytes_per_pixel();
        let pixel_stride = self.stride_bytes / bpp;
        let len = pixel_stride * self.height as usize;
        // SAFETY: `offset + stride * height <= map_len` was checked in `open`,
        // the mapping is page aligned and `offset` is a multiple of `bpp`,
        // and we hold `&mut self`, so nothing else aliases the slice.
        unsafe {
            let base = self.map.as_ptr().add(self.offset);
            match self.format {
                PixelFormat::Bgrx8888 => {
                    let buf = std::slice::from_raw_parts_mut(base.cast::<Bgrx8888>(), len);
                    renderer.render(buf, pixel_stride);
                }
                PixelFormat::Rgbx8888 => {
                    let buf = std::slice::from_raw_parts_mut(base.cast::<Rgbx8888>(), len);
                    renderer.render(buf, pixel_stride);
                }
                PixelFormat::Rgb565 => {
                    let buf = std::slice::from_raw_parts_mut(base.cast::<Rgb565Pixel>(), len);
                    renderer.render(buf, pixel_stride);
                }
            }
        }
    }
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        // SAFETY: unmapping exactly what `open` mapped.
        unsafe {
            libc::munmap(self.map.as_ptr().cast(), self.map_len);
        }
    }
}
