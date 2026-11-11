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
}
