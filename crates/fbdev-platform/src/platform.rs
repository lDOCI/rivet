//! The Slint `Platform` and its event loop.

use std::{
    cell::{Cell, RefCell},
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use anyhow::Context;
use slint::{
    platform::{
        software_renderer::{MinimalSoftwareWindow, RenderingRotation, RepaintBufferType},
        EventLoopProxy, Platform, PointerEventButton, WindowAdapter, WindowEvent,
    },
    EventLoopError, LogicalPosition, PhysicalSize, PlatformError,
};

use crate::{framebuffer::Framebuffer, touch::Touch, touch_state::TouchAction, Config, Rotation};

/// Upper bound on idle sleep so a stuck timer can't freeze the UI forever.
const MAX_IDLE: Duration = Duration::from_secs(1);
/// Frame interval while animations run.
const FRAME: Duration = Duration::from_millis(16);

type Job = Box<dyn FnOnce() + Send>;

struct Shared {
    jobs: Mutex<Vec<Job>>,
    quit: AtomicBool,
    wake_write: OwnedFd,
}

impl Shared {
    fn wake(&self) {
        let byte = 1u8;
        // SAFETY: writing one byte to our own non-blocking pipe. A full pipe
        // (EAGAIN) already guarantees a pending wakeup, so the result is ignored.
        unsafe {
            libc::write(self.wake_write.as_raw_fd(), (&byte as *const u8).cast(), 1);
        }
    }
}

struct Proxy(Arc<Shared>);

impl EventLoopProxy for Proxy {
    fn quit_event_loop(&self) -> Result<(), EventLoopError> {
        self.0.quit.store(true, Ordering::Release);
        self.0.wake();
        Ok(())
    }

    fn invoke_from_event_loop(&self, job: Job) -> Result<(), EventLoopError> {
        self.0.jobs.lock().unwrap().push(job);
        self.0.wake();
        Ok(())
    }
}

struct FbPlatform {
    window: Rc<MinimalSoftwareWindow>,
    framebuffer: RefCell<Framebuffer>,
    touch: RefCell<Option<Touch>>,
    rotation: Rotation,
    rotation_applied: Cell<bool>,
    shared: Arc<Shared>,
    wake_read: OwnedFd,
}

/// Open the framebuffer and touch device and install the platform.
/// Call before creating any Slint component.
pub fn init(config: Config) -> anyhow::Result<()> {
    let framebuffer = Framebuffer::open(&config.framebuffer)
        .with_context(|| format!("open framebuffer {}", config.framebuffer.display()))?;

    let (ui_w, ui_h) = if config.rotation.is_transposed() {
        (framebuffer.height, framebuffer.width)
    } else {
        (framebuffer.width, framebuffer.height)
    };

    // A missing touch panel shouldn't keep the status display from working.
    let touch = match Touch::open(
        config.touch.as_deref(),
        ui_w as f32,
        ui_h as f32,
        config.touch_transform,
    ) {
        Ok(t) => Some(t),
        Err(err) => {
            tracing::warn!(%err, "touch input unavailable");
            None
        }
    };

    let mut fds = [0; 2];
    // SAFETY: pipe2 fills two fds on success.
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_NONBLOCK | libc::O_CLOEXEC) } < 0 {
        return Err(std::io::Error::last_os_error()).context("create wake pipe");
    }
    // SAFETY: fresh fds from pipe2, owned exclusively from here on.
    let (wake_read, wake_write) =
        unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };

    // The previous frame stays in video memory, so only dirty regions are drawn.
    let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    window.set_size(PhysicalSize::new(ui_w, ui_h));

    let platform = FbPlatform {
        window,
        framebuffer: RefCell::new(framebuffer),
        touch: RefCell::new(touch),
        rotation: config.rotation,
        rotation_applied: Cell::new(false),
        shared: Arc::new(Shared {
            jobs: Mutex::default(),
            quit: AtomicBool::new(false),
            wake_write,
        }),
        wake_read,
    };
    slint::platform::set_platform(Box::new(platform))
        .map_err(|e| anyhow::anyhow!("set Slint platform: {e}"))?;
    Ok(())
}

impl FbPlatform {
    fn dispatch_touch(&self) {
        let mut touch = self.touch.borrow_mut();
        let Some(dev) = touch.as_mut() else { return };
        let window = self.window.clone();
        let result = dev.read(|action| {
            let event = match action {
                TouchAction::Pressed { x, y } => WindowEvent::PointerPressed {
                    position: LogicalPosition::new(x, y),
                    button: PointerEventButton::Left,
                },
                TouchAction::Moved { x, y } => WindowEvent::PointerMoved {
                    position: LogicalPosition::new(x, y),
                },
                TouchAction::Released { x, y } => WindowEvent::PointerReleased {
                    position: LogicalPosition::new(x, y),
                    button: PointerEventButton::Left,
                },
            };
            window.window().dispatch_event(event);
            if matches!(action, TouchAction::Released { .. }) {
                window.window().dispatch_event(WindowEvent::PointerExited);
            }
        });
        if let Err(err) = result {
            tracing::error!(%err, "touch device lost");
            *touch = None;
        }
    }

    fn draw(&self) {
        self.window.draw_if_needed(|renderer| {
            if !self.rotation_applied.replace(true) {
                renderer.set_rendering_rotation(match self.rotation {
                    Rotation::Deg0 => RenderingRotation::NoRotation,
                    Rotation::Deg90 => RenderingRotation::Rotate90,
                    Rotation::Deg180 => RenderingRotation::Rotate180,
                    Rotation::Deg270 => RenderingRotation::Rotate270,
                });
            }
            self.framebuffer.borrow_mut().render(renderer);
        });
    }

    fn wait(&self, timeout: Option<Duration>) {
        let touch_fd = self.touch.borrow().as_ref().map(Touch::fd);
        let mut fds = vec![libc::pollfd {
            fd: self.wake_read.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        }];
        if let Some(fd) = touch_fd {
            fds.push(libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            });
        }
        let timeout_ms = timeout.map_or(-1, |d| d.as_millis().min(i32::MAX as u128) as i32);
        // SAFETY: `fds` is a valid array of pollfd for the duration of the call.
        unsafe {
            libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout_ms);
        }
        // Drain wakeup bytes.
        let mut buf = [0u8; 64];
        // SAFETY: reading into a local buffer from our non-blocking pipe.
        while unsafe {
            libc::read(
                self.wake_read.as_raw_fd(),
                buf.as_mut_ptr().cast(),
                buf.len(),
            )
        } > 0
        {}
    }
}

impl Platform for FbPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.window.clone())
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
        Some(Box::new(Proxy(self.shared.clone())))
    }

    fn run_event_loop(&self) -> Result<(), PlatformError> {
        loop {
            slint::platform::update_timers_and_animations();

            let jobs = std::mem::take(&mut *self.shared.jobs.lock().unwrap());
            for job in jobs {
                job();
            }
            if self.shared.quit.swap(false, Ordering::AcqRel) {
                return Ok(());
            }

            self.dispatch_touch();
            self.draw();

            let timeout = if self.window.window().has_active_animations() {
                Some(FRAME)
            } else {
                Some(
                    slint::platform::duration_until_next_timer_update()
                        .map_or(MAX_IDLE, |d| d.min(MAX_IDLE)),
                )
            };
            self.wait(timeout);
        }
    }
}
