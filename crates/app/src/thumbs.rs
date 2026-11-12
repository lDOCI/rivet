//! Model thumbnails: a small HTTP GET to Moonraker, PNG decode, a box-filter
//! downscale and a cache with a byte limit. No HTTP client crate: the files
//! are plain static GETs on the local network.

use std::{
    collections::{HashMap, VecDeque},
    io::Cursor,
    time::Duration,
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

/// Widest image kept: a card is ~200 px, the file screen 264 px.
pub const MAX_WIDTH: u32 = 264;
/// Skip anything bigger than this on the wire or decoded: the printer has
/// ~38 MB free.
const MAX_BODY: usize = 1_500_000;
const MAX_PIXELS: u64 = 1_000_000;
const TIMEOUT: Duration = Duration::from_secs(10);

/// Decoded image, 8-bit RGBA.
#[derive(Debug, Clone, PartialEq)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// GET `path` from `host` (`ip:port`) and return the body.
pub async fn http_get(host: &str, path: &str) -> Result<Vec<u8>, String> {
    let fetch = async {
        let mut stream = TcpStream::connect(host).await.map_err(|e| e.to_string())?;
        let request = format!(
            "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nAccept: */*\r\n\r\n"
        );
        stream
            .write_all(request.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        let mut raw = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            let n = stream.read(&mut chunk).await.map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            raw.extend_from_slice(&chunk[..n]);
            if raw.len() > MAX_BODY + 4096 {
                return Err("response too large".into());
            }
        }
        parse_response(&raw)
    };
    tokio::time::timeout(TIMEOUT, fetch)
        .await
        .map_err(|_| "timed out".to_owned())?
}

/// Body of an HTTP/1.1 response read to the end; 200 only.
pub fn parse_response(raw: &[u8]) -> Result<Vec<u8>, String> {
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("no header end")?;
    let head = String::from_utf8_lossy(&raw[..split]).to_ascii_lowercase();
    let body = &raw[split + 4..];
    let status = head.lines().next().unwrap_or_default();
    if status.split_whitespace().nth(1) != Some("200") {
        return Err(format!("status: {status}"));
    }
    if head.contains("transfer-encoding: chunked") {
        return dechunk(body);
    }
    Ok(body.to_vec())
}

fn dechunk(mut body: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    loop {
        let eol = body
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or("bad chunk")?;
        let size_text = String::from_utf8_lossy(&body[..eol]);
        let size = usize::from_str_radix(size_text.split(';').next().unwrap_or("").trim(), 16)
            .map_err(|_| "bad chunk size")?;
        body = &body[eol + 2..];
        if size == 0 {
            return Ok(out);
        }
        if body.len() < size + 2 {
            return Err("short chunk".into());
        }
        out.extend_from_slice(&body[..size]);
        body = &body[size + 2..];
    }
}

/// Percent-encode a path, keeping `/`.
pub fn encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub fn decode_png(bytes: &[u8]) -> Option<Rgba> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let (w, h) = (reader.info().width, reader.info().height);
    if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return None;
    }
    let mut buf = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut buf).ok()?;
    let src = &buf[..frame.buffer_size()];
    let data = match frame.color_type {
        png::ColorType::Rgba => src.to_vec(),
        png::ColorType::Rgb => src
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => src
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => src.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    Some(Rgba {
        width: w,
        height: h,
        data,
    })
}

/// Shrink by a whole factor with a box filter so the image is at most
/// `max_width` wide. Smaller images are kept as they are.
/// Everything a fetched preview goes through before the screen shows it: fitted to
/// MAX_WIDTH, its empty margins cut, small ones enlarged, a shadow put under it.
pub fn prepare(img: Rgba) -> Rgba {
    shadow(enlarge(trim(downscale(img, MAX_WIDTH))))
}

/// A soft oval shadow under the model, as the prototype's renders have. It is
/// painted into the bitmap (behind the model, in a few extra rows below it): the
/// printer's renderer has no blur, but draws alpha bitmaps fine. A preview with an
/// opaque background (no transparent corners) has nowhere to show it and stays as is.
pub fn shadow(img: Rgba) -> Rgba {
    let (w, h) = (img.width as usize, img.height as usize);
    if w < 8 || h < 8 {
        return img;
    }
    let alpha = |x: usize, y: usize| img.data[(y * w + x) * 4 + 3];
    let corners = [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)];
    if corners.iter().any(|&(x, y)| alpha(x, y) > 16) {
        return img;
    }
    // The oval: as wide as most of the model, flat, a little below its lowest edge
    // so its lower half shows, as a render's contact shadow.
    let rx = w as f32 * 0.44;
    let ry = (rx * 0.2).max(3.0);
    let cx = w as f32 / 2.0;
    let cy = h as f32 - 4.0 + ry * 0.35;
    let extra = (cy + ry + 1.0).ceil() as usize - h.min((cy + ry + 1.0).ceil() as usize);
    let nh = h + extra;
    const DARK: f32 = 0.7;
    let mut data = vec![0u8; w * nh * 4];
    for y in 0..nh {
        for x in 0..w {
            let (dx, dy) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
            let d2 = dx * dx + dy * dy;
            // Densest in the middle, fading to nothing at the oval's edge.
            let sa = if d2 < 1.0 {
                DARK * (1.0 - d2).powf(1.4)
            } else {
                0.0
            };
            let o = (y * w + x) * 4;
            let (r, g, b, a) = if y < h {
                let i = &img.data[o..o + 4];
                (
                    f32::from(i[0]),
                    f32::from(i[1]),
                    f32::from(i[2]),
                    f32::from(i[3]) / 255.0,
                )
            } else {
                (0.0, 0.0, 0.0, 0.0)
            };
            // The model over the (black) shadow, straight alpha.
            let oa = a + sa * (1.0 - a);
            if oa > 0.0 {
                data[o] = (r * a / oa).round() as u8;
                data[o + 1] = (g * a / oa).round() as u8;
                data[o + 2] = (b * a / oa).round() as u8;
                data[o + 3] = (oa * 255.0).round() as u8;
            }
        }
    }
    Rgba {
        width: w as u32,
        height: nh as u32,
        data,
    }
}

/// Cut the transparent margins around the model (slicer previews leave wide ones),
/// keeping a few pixels: the model then fills its place and a shadow can sit under
/// it. An opaque image stays as it is.
pub fn trim(img: Rgba) -> Rgba {
    let (w, h) = (img.width as usize, img.height as usize);
    let solid = |x: usize, y: usize| img.data[(y * w + x) * 4 + 3] > 16;
    let rows: Vec<usize> = (0..h).filter(|&y| (0..w).any(|x| solid(x, y))).collect();
    let cols: Vec<usize> = (0..w).filter(|&x| (0..h).any(|y| solid(x, y))).collect();
    let (Some(&top), Some(&bottom), Some(&left), Some(&right)) =
        (rows.first(), rows.last(), cols.first(), cols.last())
    else {
        return img;
    };
    const PAD: usize = 3;
    let (x0, y0) = (left.saturating_sub(PAD), top.saturating_sub(PAD));
    let (x1, y1) = ((right + PAD).min(w - 1), (bottom + PAD).min(h - 1));
    if x0 == 0 && y0 == 0 && x1 == w - 1 && y1 == h - 1 {
        return img;
    }
    let (nw, nh) = (x1 - x0 + 1, y1 - y0 + 1);
    let mut data = Vec::with_capacity(nw * nh * 4);
    for y in y0..=y1 {
        data.extend_from_slice(&img.data[(y * w + x0) * 4..(y * w + x1 + 1) * 4]);
    }
    Rgba {
        width: nw as u32,
        height: nh as u32,
        data,
    }
}

/// Slicer previews are small (often 140×110). Scaled up 1.5× with Catmull-Rom
/// bicubic filtering they keep their edges; bilinear blurs them and the screen's
/// own scaling (nearest pixel) makes blocks. Larger previews stay as they are.
pub fn enlarge(img: Rgba) -> Rgba {
    if img.width == 0 || img.height == 0 || img.width >= 200 {
        return img;
    }
    let width = (img.width * 3 / 2).min(MAX_WIDTH);
    let height = (img.height * width / img.width).max(1);
    resize(&img, width, height)
}

/// Scale to fill a `width` × `height` box keeping proportions, at most `max_scale`
/// times up (bicubic) — for a preview that has a big place of its own, such as
/// the Home card. Larger images are shrunk with a box filter first.
pub fn fit(img: Rgba, width: u32, height: u32, max_scale: f32) -> Rgba {
    if img.width == 0 || img.height == 0 {
        return img;
    }
    let img = downscale(img, width);
    let k = (width as f32 / img.width as f32)
        .min(height as f32 / img.height as f32)
        .min(max_scale);
    if k <= 1.01 {
        return img;
    }
    let (w, h) = (
        (img.width as f32 * k).round() as u32,
        (img.height as f32 * k).round() as u32,
    );
    resize(&img, w.max(1), h.max(1))
}

/// A preview for a box of its own: margins cut, fitted to the box (the shadow
/// takes the rows below), the shadow under it.
pub fn prepare_for(img: Rgba, width: u32, height: u32) -> Rgba {
    shadow(fit(trim(img), width, height * 9 / 10, 3.0))
}

/// Catmull-Rom bicubic resize.
fn resize(img: &Rgba, width: u32, height: u32) -> Rgba {
    let (sw, sh) = (img.width as i64, img.height as i64);
    let at = |x: i64, y: i64, c: usize| -> f32 {
        let (x, y) = (x.clamp(0, sw - 1), y.clamp(0, sh - 1));
        f32::from(img.data[((y * sw + x) * 4) as usize + c])
    };
    // Catmull-Rom weights for the four taps around t in 0..1.
    let weights = |t: f32| {
        let (t2, t3) = (t * t, t * t * t);
        [
            0.5 * (-t3 + 2.0 * t2 - t),
            0.5 * (3.0 * t3 - 5.0 * t2 + 2.0),
            0.5 * (-3.0 * t3 + 4.0 * t2 + t),
            0.5 * (t3 - t2),
        ]
    };
    let (kx, ky) = (
        img.width as f32 / width as f32,
        img.height as f32 / height as f32,
    );
    let mut data = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        let fy = (y as f32 + 0.5) * ky - 0.5;
        let (y0, wy) = (fy.floor() as i64, weights(fy - fy.floor()));
        for x in 0..width {
            let fx = (x as f32 + 0.5) * kx - 0.5;
            let (x0, wx) = (fx.floor() as i64, weights(fx - fx.floor()));
            for c in 0..4 {
                let mut v = 0.0;
                for (j, wyj) in wy.iter().enumerate() {
                    for (i, wxi) in wx.iter().enumerate() {
                        v += wyj * wxi * at(x0 - 1 + i as i64, y0 - 1 + j as i64, c);
                    }
                }
                data.push(v.round().clamp(0.0, 255.0) as u8);
            }
        }
    }
    Rgba {
        width,
        height,
        data,
    }
}

pub fn downscale(img: Rgba, max_width: u32) -> Rgba {
    let f = img.width.div_ceil(max_width).max(1);
    if f == 1 {
        return img;
    }
    let (nw, nh) = (img.width / f, img.height / f);
    let mut data = Vec::with_capacity((nw * nh * 4) as usize);
    for y in 0..nh {
        for x in 0..nw {
            let mut sum = [0u32; 4];
            for dy in 0..f {
                for dx in 0..f {
                    let i = (((y * f + dy) * img.width + x * f + dx) * 4) as usize;
                    for (c, s) in sum.iter_mut().enumerate() {
                        *s += u32::from(img.data[i + c]);
                    }
                }
            }
            data.extend(sum.map(|s| (s / (f * f)) as u8));
        }
    }
    Rgba {
        width: nw,
        height: nh,
        data,
    }
}

/// Least recently used values, bounded by their total size in bytes.
pub struct Cache<T> {
    limit: usize,
    used: usize,
    map: HashMap<String, (T, usize)>,
    order: VecDeque<String>,
}

impl<T: Clone> Cache<T> {
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            used: 0,
            map: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<T> {
        let value = self.map.get(key)?.0.clone();
        self.touch(key);
        Some(value)
    }

    pub fn insert(&mut self, key: &str, value: T, size: usize) {
        if size > self.limit {
            return;
        }
        if let Some((_, old)) = self.map.remove(key) {
            self.used -= old;
            self.order.retain(|k| k != key);
        }
        while self.used + size > self.limit {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some((_, s)) = self.map.remove(&oldest) {
                self.used -= s;
            }
        }
        self.used += size;
        self.map.insert(key.to_owned(), (value, size));
        self.order.push_back(key.to_owned());
    }

    fn touch(&mut self, key: &str) {
        self.order.retain(|k| k != key);
        self.order.push_back(key.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_bytes(w: u32, h: u32, color: png::ColorType, pixel: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(color);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().unwrap();
        writer
            .write_image_data(&pixel.repeat((w * h) as usize))
            .unwrap();
        drop(writer);
        out
    }

    #[test]
    fn decodes_rgb_and_rgba_png() {
        let rgb = decode_png(&png_bytes(2, 2, png::ColorType::Rgb, &[10, 20, 30])).unwrap();
        assert_eq!((rgb.width, rgb.height), (2, 2));
        assert_eq!(&rgb.data[..4], [10, 20, 30, 255]);
        let rgba = decode_png(&png_bytes(1, 1, png::ColorType::Rgba, &[1, 2, 3, 4])).unwrap();
        assert_eq!(rgba.data, [1, 2, 3, 4]);
        assert!(decode_png(b"not a png").is_none());
    }

    #[test]
    fn shadow_only_under_transparent_previews() {
        let opaque = Rgba {
            width: 40,
            height: 20,
            data: vec![255; 40 * 20 * 4],
        };
        assert_eq!(shadow(opaque).height, 20);
        let clear = Rgba {
            width: 100,
            height: 20,
            data: vec![0; 100 * 20 * 4],
        };
        let s = shadow(clear);
        assert!(s.height > 20);
        let a = |x: usize, y: usize| s.data[(y * 100 + x) * 4 + 3];
        // Dark under the middle, nothing at the sides.
        assert!(a(50, 18) > 100);
        assert_eq!(a(0, 18), 0);
        assert!(s.data.chunks(4).all(|p| p[..3] == [0, 0, 0]));
    }

    #[test]
    fn trim_cuts_transparent_margins() {
        // 6×4, one solid pixel at (3, 2).
        let mut data = vec![0u8; 6 * 4 * 4];
        data[(2 * 6 + 3) * 4 + 3] = 255;
        let t = trim(Rgba {
            width: 6,
            height: 4,
            data,
        });
        assert_eq!((t.width, t.height), (6, 4));
        let mut data = vec![0u8; 20 * 20 * 4];
        data[(10 * 20 + 10) * 4 + 3] = 255;
        let t = trim(Rgba {
            width: 20,
            height: 20,
            data,
        });
        assert_eq!((t.width, t.height), (7, 7));
    }

    #[test]
    fn enlarge_small_previews_only() {
        let small = Rgba {
            width: 2,
            height: 2,
            data: vec![255; 16],
        };
        let big = enlarge(small);
        assert_eq!((big.width, big.height), (3, 3));
        assert!(big.data.iter().all(|&v| v == 255));
        let large = Rgba {
            width: 300,
            height: 1,
            data: vec![0; 1200],
        };
        assert_eq!(enlarge(large).width, 300);
    }

    #[test]
    fn downscale_averages_blocks() {
        let img = Rgba {
            width: 4,
            height: 2,
            data: [[0, 0, 0, 255], [100, 100, 100, 255]]
                .iter()
                .cycle()
                .take(8)
                .flatten()
                .copied()
                .collect(),
        };
        let small = downscale(img.clone(), 2);
        assert_eq!((small.width, small.height), (2, 1));
        assert_eq!(&small.data[..4], [50, 50, 50, 255]);
        // Already small enough: untouched.
        assert_eq!(downscale(img.clone(), 8), img);
    }

    #[test]
    fn http_body_plain_chunked_and_errors() {
        let plain = b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nabc";
        assert_eq!(parse_response(plain).unwrap(), b"abc");
        let chunked = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n";
        assert_eq!(parse_response(chunked).unwrap(), b"abcde");
        assert!(parse_response(b"HTTP/1.1 404 Not Found\r\n\r\n").is_err());
        assert!(parse_response(b"garbage").is_err());
    }

    #[test]
    fn paths_are_percent_encoded() {
        assert_eq!(
            encode_path("sub dir/.thumbs/a b-300x300.png"),
            "sub%20dir/.thumbs/a%20b-300x300.png"
        );
        assert_eq!(
            encode_path("Корпус.png"),
            "%D0%9A%D0%BE%D1%80%D0%BF%D1%83%D1%81.png"
        );
    }

    #[test]
    fn cache_evicts_least_recently_used_by_bytes() {
        let mut c = Cache::new(10);
        c.insert("a", 1, 4);
        c.insert("b", 2, 4);
        assert_eq!(c.get("a"), Some(1)); // a is now newer than b
        c.insert("c", 3, 4); // 12 > 10: b goes
        assert_eq!(c.get("b"), None);
        assert_eq!(c.get("a"), Some(1));
        assert_eq!(c.get("c"), Some(3));
        // Too big for the cache at all: not stored.
        c.insert("huge", 9, 11);
        assert_eq!(c.get("huge"), None);
    }

    #[tokio::test]
    async fn fetches_from_a_local_server() {
        use tokio::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut req = [0u8; 1024];
            let n = sock.read(&mut req).await.unwrap();
            let req = String::from_utf8_lossy(&req[..n]).into_owned();
            assert!(req.starts_with("GET /server/files/gcodes/a.png HTTP/1.1"));
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nhi")
                .await
                .unwrap();
        });
        let body = http_get(&addr, "/server/files/gcodes/a.png").await.unwrap();
        assert_eq!(body, b"hi");
    }

    #[test]
    fn fit_fills_a_wide_box() {
        let img = Rgba {
            width: 100,
            height: 80,
            data: vec![200; 100 * 80 * 4],
        };
        // Limited by the height: 80 → 160 (2×, the cap is 2).
        let big = fit(img, 372, 232, 2.0);
        assert_eq!((big.width, big.height), (200, 160));
        let wide = Rgba {
            width: 300,
            height: 100,
            data: vec![200; 300 * 100 * 4],
        };
        // Limited by the width: 300 → 372.
        let f = fit(wide, 372, 232, 2.0);
        assert_eq!(f.width, 372);
    }
}
