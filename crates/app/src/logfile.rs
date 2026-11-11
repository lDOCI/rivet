//! A log file that rotates by size, so logs never fill the printer's flash:
//! `rivet.log` is renamed to `rivet.log.1` (and `.1` to `.2`, …) when it would
//! grow past the limit; the oldest copy is dropped.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

pub struct RotatingFile {
    path: PathBuf,
    max_bytes: u64,
    keep: usize,
    state: Mutex<State>,
}

struct State {
    file: File,
    size: u64,
}

impl RotatingFile {
    /// Open (append) `path`. `keep` is how many rotated copies to keep.
    pub fn open(path: &Path, max_bytes: u64, keep: usize) -> io::Result<Self> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            fs::create_dir_all(dir)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let size = file.metadata()?.len();
        Ok(Self {
            path: path.to_owned(),
            max_bytes,
            keep,
            state: Mutex::new(State { file, size }),
        })
    }

    fn rotated(&self, n: usize) -> PathBuf {
        let mut name = self.path.clone().into_os_string();
        name.push(format!(".{n}"));
        PathBuf::from(name)
    }

    fn rotate(&self, state: &mut State) -> io::Result<()> {
        // .keep is dropped, the others move up by one, the log becomes .1
        if self.keep == 0 {
            fs::remove_file(&self.path)?;
        } else {
            for n in (1..self.keep).rev() {
                let from = self.rotated(n);
                if from.exists() {
                    fs::rename(&from, self.rotated(n + 1))?;
                }
            }
            fs::rename(&self.path, self.rotated(1))?;
        }
        state.file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        state.size = 0;
        Ok(())
    }

    fn write_line(&self, buf: &[u8]) -> io::Result<usize> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.size > 0 && state.size + buf.len() as u64 > self.max_bytes {
            // A failed rotation must not stop logging.
            let _ = self.rotate(&mut state);
        }
        let n = state.file.write(buf)?;
        state.size += n as u64;
        Ok(n)
    }
}

impl Write for &RotatingFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write_line(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .file
            .flush()
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for RotatingFile {
    type Writer = &'a RotatingFile;

    fn make_writer(&'a self) -> Self::Writer {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rivet-log-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn rotates_by_size_and_keeps_a_limited_history() {
        let dir = temp_dir("rotate");
        let path = dir.join("rivet.log");
        let log = RotatingFile::open(&path, 20, 2).unwrap();
        // Each line is 10 bytes: two fit, the third rotates.
        for i in 0..7 {
            (&log)
                .write_all(format!("line {i}   \n").as_bytes())
                .unwrap();
        }
        let read = |p: &Path| fs::read_to_string(p).unwrap_or_default();
        assert_eq!(read(&path), "line 6   \n");
        assert_eq!(read(&log.rotated(1)), "line 4   \nline 5   \n");
        assert_eq!(read(&log.rotated(2)), "line 2   \nline 3   \n");
        // Only two rotated copies: the oldest lines are gone.
        assert!(!log.rotated(3).exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn appends_to_an_existing_file_and_counts_its_size() {
        let dir = temp_dir("append");
        let path = dir.join("sub/rivet.log");
        {
            let log = RotatingFile::open(&path, 100, 1).unwrap();
            (&log).write_all(b"first\n").unwrap();
        }
        let log = RotatingFile::open(&path, 100, 1).unwrap();
        (&log).write_all(b"second\n").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "first\nsecond\n");
        let _ = fs::remove_dir_all(dir);
    }
}
