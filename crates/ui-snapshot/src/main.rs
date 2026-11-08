//! `ui-snapshot <out-dir> [screen…]` renders each screen of the app at 800x480
//! with the software renderer (the one the printer uses) and writes PNGs.

use std::{fs::File, io::BufWriter, path::PathBuf, rc::Rc};

use anyhow::Context;
use rivet::AppWindow;
use slint::{
    platform::{
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
        Platform, WindowAdapter,
    },
    ComponentHandle, PhysicalSize, PlatformError, Rgb8Pixel,
};

const WIDTH: u32 = 800;
const HEIGHT: u32 = 480;
const ALL: &[&str] = &["home", "control", "settings", "fans", "tune"];

struct Headless(Rc<MinimalSoftwareWindow>);

impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.0.clone())
    }
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().unwrap_or_else(|| "snapshots".into()));
    let screens: Vec<String> = args.collect();
    let screens: Vec<&str> = if screens.is_empty() {
        ALL.to_vec()
    } else {
        screens.iter().map(String::as_str).collect()
    };
    std::fs::create_dir_all(&out)?;

    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    window.set_size(PhysicalSize::new(WIDTH, HEIGHT));
    slint::platform::set_platform(Box::new(Headless(window.clone())))
        .map_err(|e| anyhow::anyhow!("set platform: {e}"))?;

    let ui = AppWindow::new()?;
    ui.set_link_state("ready".into());
    ui.set_extruder_temp(187.0);
    ui.set_extruder_target(240.0);
    ui.set_bed_temp(23.0);
    ui.show()?;

    let mut buf = vec![Rgb8Pixel::default(); (WIDTH * HEIGHT) as usize];
    for screen in screens {
        ui.set_screen(screen.into());
        slint::platform::update_timers_and_animations();
        window.request_redraw();
        window.draw_if_needed(|renderer| {
            renderer.render(&mut buf, WIDTH as usize);
        });
        let path = out.join(format!("{screen}.png"));
        write_png(&path, &buf).with_context(|| format!("write {}", path.display()))?;
        println!("{}", path.display());
    }
    Ok(())
}

fn write_png(path: &PathBuf, buf: &[Rgb8Pixel]) -> anyhow::Result<()> {
    let mut encoder = png::Encoder::new(BufWriter::new(File::create(path)?), WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    let bytes: Vec<u8> = buf.iter().flat_map(|p| [p.r, p.g, p.b]).collect();
    writer.write_image_data(&bytes)?;
    Ok(())
}
