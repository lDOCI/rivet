//! evdev touch device: discovery, axis ranges and non-blocking reads.

use std::{
    fs::{File, OpenOptions},
    io::{self, Read},
    os::{
        fd::{AsRawFd, RawFd},
        unix::fs::OpenOptionsExt,
    },
    path::{Path, PathBuf},
};

use crate::touch_state::{
    AxisRange, TouchAction, TouchState, TouchTransform, ABS_MT_POSITION_X, ABS_MT_POSITION_Y,
    ABS_X, ABS_Y, EV_ABS,
};

/// `struct input_event`. The kernel keeps `__kernel_ulong_t` seconds here even
/// for 32-bit time64 userspace, so `c_ulong` is right on both armv7 and 64-bit.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct InputEvent {
    sec: libc::c_ulong,
    usec: libc::c_ulong,
    kind: u16,
    code: u16,
    value: i32,
}

/// `struct input_absinfo`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct AbsInfo {
    value: i32,
    minimum: i32,
    maximum: i32,
    fuzz: i32,
    flat: i32,
    resolution: i32,
}

const IOC_READ: u32 = 2;

const fn ioc_read(nr: u32, size: usize) -> u32 {
    (IOC_READ << 30) | ((size as u32) << 16) | ((b'E' as u32) << 8) | nr
}

fn eviocgname(fd: RawFd) -> String {
    let mut buf = [0u8; 128];
    // SAFETY: buffer length matches the size encoded in the request.
    let n = unsafe { libc::ioctl(fd, ioc_read(0x06, buf.len()) as _, buf.as_mut_ptr()) };
    if n <= 0 {
        return String::new();
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

/// Bitmask of supported absolute axes (`EVIOCGBIT(EV_ABS)`).
fn abs_bits(fd: RawFd) -> u64 {
    let mut bits = [0u8; 8];
    // SAFETY: buffer length matches the size encoded in the request.
    let n = unsafe {
        libc::ioctl(
            fd,
            ioc_read(0x20 + EV_ABS as u32, bits.len()) as _,
            bits.as_mut_ptr(),
        )
    };
    if n < 0 {
        return 0;
    }
    u64::from_le_bytes(bits)
}

fn abs_range(fd: RawFd, axis: u16) -> Option<AxisRange> {
    let mut info = AbsInfo::default();
    // SAFETY: `AbsInfo` matches `struct input_absinfo`.
    let r = unsafe {
        libc::ioctl(
            fd,
            ioc_read(0x40 + axis as u32, std::mem::size_of::<AbsInfo>()) as _,
            &mut info,
        )
    };
    (r >= 0 && info.maximum > info.minimum).then_some(AxisRange {
        min: info.minimum,
        max: info.maximum,
    })
}

fn has_axis(bits: u64, axis: u16) -> bool {
    bits & (1 << axis) != 0
}

fn open_nonblocking(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
}

/// First touch-capable device, preferring tslib's calibrated `ts_uinput`.
fn find_device() -> io::Result<PathBuf> {
    let mut fallback = None;
    for i in 0..32 {
        let path = PathBuf::from(format!("/dev/input/event{i}"));
        let Ok(file) = open_nonblocking(&path) else {
            continue;
        };
        let fd = file.as_raw_fd();
        let bits = abs_bits(fd);
        if !(has_axis(bits, ABS_X) || has_axis(bits, ABS_MT_POSITION_X)) {
            continue;
        }
        let name = eviocgname(fd);
        tracing::debug!(device = %path.display(), %name, "touch candidate");
        if name.contains("ts_uinput") || name.contains("tslib") {
            return Ok(path);
        }
        fallback.get_or_insert(path);
    }
    fallback.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no touch device found"))
}

pub struct Touch {
    file: File,
    state: TouchState,
}

impl Touch {
    /// Open `path` or autodetect. `width`/`height` are the UI's logical size.
    pub fn open(
        path: Option<&Path>,
        width: f32,
        height: f32,
        transform: TouchTransform,
    ) -> io::Result<Self> {
        let path = match path {
            Some(p) => p.to_path_buf(),
            None => find_device()?,
        };
        let file = open_nonblocking(&path)?;
        let fd = file.as_raw_fd();
        let bits = abs_bits(fd);
        let pick = |single, multi| {
            let axis = if has_axis(bits, single) {
                single
            } else {
                multi
            };
            abs_range(fd, axis)
        };
        let range_x = pick(ABS_X, ABS_MT_POSITION_X).unwrap_or(AxisRange {
            min: 0,
            max: width as i32,
        });
        let range_y = pick(ABS_Y, ABS_MT_POSITION_Y).unwrap_or(AxisRange {
            min: 0,
            max: height as i32,
        });

        tracing::info!(
            device = %path.display(),
            name = %eviocgname(fd),
            ?range_x,
            ?range_y,
            ?transform,
            "touch opened"
        );

        Ok(Self {
            file,
            state: TouchState::new(range_x, range_y, width, height, transform),
        })
    }

    pub fn fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    /// Drain pending events. Returns an error if the device went away.
    pub fn read(&mut self, mut on_action: impl FnMut(TouchAction)) -> io::Result<()> {
        const EVENT_SIZE: usize = std::mem::size_of::<InputEvent>();
        let mut buf = [0u8; EVENT_SIZE * 64];
        loop {
            let n = match self.file.read(&mut buf) {
                Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            for chunk in buf[..n].chunks_exact(EVENT_SIZE) {
                // SAFETY: chunk is exactly one `InputEvent`; read_unaligned
                // copes with the byte buffer's alignment.
                let ev: InputEvent =
                    unsafe { std::ptr::read_unaligned(chunk.as_ptr().cast::<InputEvent>()) };
                if let Some(action) = self.state.process(ev.kind, ev.code, ev.value) {
                    on_action(action);
                }
            }
        }
    }
}
