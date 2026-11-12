//! Memory-mapped Linux framebuffer (`/dev/fbN`).

use std::{
    fs::{File, OpenOptions},
    io,
    os::fd::AsRawFd,
    path::Path,
    ptr::NonNull,
};

use slint::platform::software_renderer::{PhysicalRegion, Rgb565Pixel, SoftwareRenderer};

use crate::pixel::{Bgrx8888, PixelFormat, Rgbx8888};

const FBIOGET_VSCREENINFO: u32 = 0x4600;
const FBIOGET_FSCREENINFO: u32 = 0x4602;
const FBIOPAN_DISPLAY: u32 = 0x4606;
/// `_IOW('F', 0x20, __u32)`.
const FBIO_WAITFORVSYNC: u32 = 0x4004_4620;

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

/// Pixel rectangle, `x1`/`y1` exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rect {
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
}

impl Rect {
    fn union(self, o: Rect) -> Rect {
        Rect {
            x0: self.x0.min(o.x0),
            y0: self.y0.min(o.y0),
            x1: self.x1.max(o.x1),
            y1: self.y1.max(o.y1),
        }
    }
}

/// Slint renders into a buffer in ordinary RAM (reading back from video
/// memory is very slow and shows up as banding). Only the changed rectangle is
/// then copied to the framebuffer. When the device exposes two pages
/// (`yres_virtual >= 2 * yres`) the copy goes to the hidden page and the pages
/// are flipped, so the screen never shows a half-drawn frame.
pub struct Framebuffer {
    file: File,
    map: NonNull<u8>,
    map_len: usize,
    var: FbVarScreeninfo,
    pub width: u32,
    pub height: u32,
    stride_bytes: usize,
    pub format: PixelFormat,
    /// Software frame, `stride_bytes * height` bytes (u32 for alignment).
    back: Vec<u32>,
    /// Page flipping available (2 pages, pan works).
    double_buffered: bool,
    /// Page currently on screen.
    front: u32,
    /// Region changed by the previous frame; the page we now write to is one
    /// frame older, so it must be refreshed too.
    prev_dirty: Option<Rect>,
    /// Copy the whole frame every time (used when the dirty region may be in
    /// different coordinates than the buffer, i.e. with rotation).
    full_copy: bool,
}

impl Framebuffer {
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        let fd = file.as_raw_fd();

        let mut var = FbVarScreeninfo::default();
        let mut fix = FbFixScreeninfo::default();
        if file.metadata()?.is_file() {
            // A plain file stands in for the screen (tests in an emulator with
            // no framebuffer): one 800×480 BGRX page, readable as a picture.
            let (w, h) = (800u32, 480u32);
            file.set_len(u64::from(w * h * 4))?;
            var.xres = w;
            var.yres = h;
            var.xres_virtual = w;
            var.yres_virtual = h;
            var.bits_per_pixel = 32;
            var.red.offset = 16;
            var.green.offset = 8;
            var.blue.offset = 0;
            fix.line_length = w * 4;
            fix.smem_len = w * h * 4;
        } else {
            // SAFETY: the structs match the kernel ABI and outlive the calls.
            unsafe {
                if libc::ioctl(fd, FBIOGET_VSCREENINFO as _, &mut var) < 0 {
                    return Err(io::Error::last_os_error());
                }
                if libc::ioctl(fd, FBIOGET_FSCREENINFO as _, &mut fix) < 0 {
                    return Err(io::Error::last_os_error());
                }
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
        if stride_bytes * var.yres as usize > map_len {
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

        let two_pages = var.yres_virtual >= 2 * var.yres
            && stride_bytes * 2 * var.yres as usize <= map_len
            && var.xres_virtual == var.xres;

        let mut fb = Self {
            file,
            map: NonNull::new(ptr.cast()).expect("mmap returned null"),
            map_len,
            var,
            width: var.xres,
            height: var.yres,
            stride_bytes,
            format,
            back: vec![0; stride_bytes * var.yres as usize / 4],
            double_buffered: false,
            front: 0,
            prev_dirty: None,
            full_copy: false,
        };
        // Start on page 0; if panning works we can flip between two pages.
        fb.double_buffered = two_pages && fb.pan(0).is_ok();

        tracing::info!(
            device = %path.display(),
            width = var.xres,
            height = var.yres,
            bpp = var.bits_per_pixel,
            stride = stride_bytes,
            ?format,
            double_buffered = fb.double_buffered,
            "framebuffer opened"
        );

        Ok(fb)
    }

    /// Copy the whole frame on every update (needed with rotation).
    pub fn set_full_copy(&mut self, full: bool) {
        self.full_copy = full;
    }

    /// Show `page` (0 or 1) and wait for the next vertical blank if supported.
    fn pan(&mut self, page: u32) -> io::Result<()> {
        let fd = self.file.as_raw_fd();
        let mut var = self.var;
        var.xoffset = 0;
        var.yoffset = page * self.height;
        // SAFETY: `var` matches the kernel ABI and outlives the call.
        if unsafe { libc::ioctl(fd, FBIOPAN_DISPLAY as _, &mut var) } < 0 {
            return Err(io::Error::last_os_error());
        }
        self.front = page;
        let mut arg = 0u32;
        // SAFETY: FBIO_WAITFORVSYNC takes a pointer to a u32. Failure just
        // means no vsync wait, which is fine.
        unsafe {
            libc::ioctl(fd, FBIO_WAITFORVSYNC as _, &mut arg);
        }
        Ok(())
    }

    fn full_rect(&self) -> Rect {
        Rect {
            x0: 0,
            y0: 0,
            x1: self.width as usize,
            y1: self.height as usize,
        }
    }

    fn rect_of(&self, region: &PhysicalRegion) -> Option<Rect> {
        let size = region.bounding_box_size();
        if size.width == 0 || size.height == 0 {
            return None;
        }
        let o = region.bounding_box_origin();
        let x0 = o.x.max(0) as usize;
        let y0 = o.y.max(0) as usize;
        let x1 = (o.x.max(0) as usize + size.width as usize).min(self.width as usize);
        let y1 = (o.y.max(0) as usize + size.height as usize).min(self.height as usize);
        (x0 < x1 && y0 < y1).then_some(Rect { x0, y0, x1, y1 })
    }

    /// Copy `rect` of the software frame into video memory page `page`.
    fn copy_to_page(&mut self, rect: Rect, page: u32) {
        let bpp = self.format.bytes_per_pixel();
        let page_off = page as usize * self.height as usize * self.stride_bytes;
        let row_bytes = (rect.x1 - rect.x0) * bpp;
        let src = self.back.as_ptr().cast::<u8>();
        for y in rect.y0..rect.y1 {
            let off = y * self.stride_bytes + rect.x0 * bpp;
            // SAFETY: rect is clamped to width/height, the mapping covers
            // `page` (checked in `open` for page 1), and the RAM frame has
            // `stride_bytes * height` bytes; the two never overlap.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    src.add(off),
                    self.map.as_ptr().add(page_off + off),
                    row_bytes,
                );
            }
        }
    }

    /// Render changed regions into the RAM frame, then show them.
    pub fn render(&mut self, renderer: &SoftwareRenderer) {
        let bpp = self.format.bytes_per_pixel();
        let pixel_stride = self.stride_bytes / bpp;
        let len = pixel_stride * self.height as usize;
        // SAFETY: `back` holds `stride_bytes * height` bytes, 4-byte aligned,
        // which covers `len` pixels of any supported format, and we hold
        // `&mut self` so nothing else aliases it.
        let region = unsafe {
            let base = self.back.as_mut_ptr();
            match self.format {
                PixelFormat::Bgrx8888 => renderer.render(
                    std::slice::from_raw_parts_mut(base.cast::<Bgrx8888>(), len),
                    pixel_stride,
                ),
                PixelFormat::Rgbx8888 => renderer.render(
                    std::slice::from_raw_parts_mut(base.cast::<Rgbx8888>(), len),
                    pixel_stride,
                ),
                PixelFormat::Rgb565 => renderer.render(
                    std::slice::from_raw_parts_mut(base.cast::<Rgb565Pixel>(), len),
                    pixel_stride,
                ),
            }
        };

        let dirty = if self.full_copy {
            Some(self.full_rect())
        } else {
            self.rect_of(&region)
        };
        let Some(dirty) = dirty else { return };

        if self.double_buffered {
            let target = 1 - self.front;
            let rect = self.prev_dirty.map_or(dirty, |p| dirty.union(p));
            self.copy_to_page(rect, target);
            match self.pan(target) {
                Ok(()) => self.prev_dirty = Some(dirty),
                Err(err) => {
                    // Flipping stopped working: fall back to drawing on the
                    // visible page directly from the RAM frame.
                    tracing::warn!(%err, "framebuffer page flip failed, using a single page");
                    self.double_buffered = false;
                    let full = self.full_rect();
                    let front = self.front;
                    self.copy_to_page(full, front);
                }
            }
        } else {
            let front = self.front;
            self.copy_to_page(dirty, front);
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
