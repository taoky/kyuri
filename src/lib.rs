use std::{
    collections::BTreeMap,
    sync::{atomic::AtomicUsize, Arc, Mutex},
};

mod template;
use template::{Template, TemplatePart};

const CLEAR_ANSI: &str = "\r\x1b[K";
const UP_ANSI: &str = "\x1b[F";

pub(crate) struct BarState {
    len: u64,
    pos: u64,
    message: String,
    template: Template,
    created_at: std::time::Instant,
}

fn duration_to_human(duration: std::time::Duration) -> String {
    let elapsed = duration.as_secs();
    let hours = elapsed / 3600;
    let minutes = (elapsed % 3600) / 60;
    let seconds = elapsed % 60;
    format!("{}:{:02}:{:02}", hours, minutes, seconds)
}

fn bytes_to_human(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes < KB {
        format!("{} B", bytes)
    } else if bytes < MB {
        format!("{:.2} KiB", bytes as f64 / KB as f64)
    } else if bytes < GB {
        format!("{:.2} MiB", bytes as f64 / MB as f64)
    } else if bytes < TB {
        format!("{:.2} GiB", bytes as f64 / GB as f64)
    } else {
        format!("{:.2} TiB", bytes as f64 / TB as f64)
    }
}

impl BarState {
    pub fn render(&self) -> String {
        let mut result = String::new();
        let elapsed = std::time::Instant::now() - self.created_at;
        let bytes_per_second = self.pos as f64 / elapsed.as_secs_f64();
        for part in self.template.parts.iter() {
            match part {
                TemplatePart::Text(text) => {
                    result.push_str(text);
                }
                TemplatePart::Newline => {
                    result.push('\n');
                }
                TemplatePart::Message => {
                    result.push_str(&self.message);
                }
                TemplatePart::Elapsed => {
                    result.push_str(&duration_to_human(elapsed));
                }
                TemplatePart::Bytes => {
                    result.push_str(&bytes_to_human(self.pos));
                }
                TemplatePart::Pos => {
                    result.push_str(&self.pos.to_string());
                }
                TemplatePart::TotalBytes => {
                    result.push_str(&bytes_to_human(self.len));
                }
                TemplatePart::Total => {
                    result.push_str(&self.len.to_string());
                }
                TemplatePart::BytesPerSecond => {
                    result.push_str(&format!("{}/s", bytes_to_human(bytes_per_second as u64)));
                }
                TemplatePart::Eta => {
                    if self.pos == 0 {
                        result.push_str("Unknown");
                    } else {
                        let eta = (self.len - self.pos) as f64 / bytes_per_second;
                        result.push_str(&duration_to_human(std::time::Duration::from_secs(
                            eta as u64,
                        )));
                    }
                }
                TemplatePart::Bar(size) => {
                    let filled = (self.pos as f64 / self.len as f64 * *size as f64) as usize;
                    let empty = *size - filled;
                    result.push('[');
                    for _ in 0..filled {
                        result.push('=');
                    }
                    for _ in 0..empty {
                        result.push(' ');
                    }
                    result.push(']');
                }
            }
        }
        result
    }
}

pub struct Bar {
    id: usize,
    state: Arc<Mutex<BarState>>,
    manager: Manager,
}

struct ManagerInner {
    states: Mutex<BTreeMap<usize, Arc<Mutex<BarState>>>>,
    next_id: AtomicUsize,
    interval: std::time::Duration,
    out: Mutex<Box<dyn Out>>,
    last_draw: Mutex<std::time::Instant>,
    last_lines: AtomicUsize,
    ansi: Mutex<Option<bool>>,
}

pub trait Out: std::io::Write + std::io::IsTerminal + Send + Sync {}
impl<T: std::io::Write + std::io::IsTerminal + Send + Sync> Out for T {}

#[derive(Clone)]
pub struct Manager {
    inner: Arc<ManagerInner>,
}

impl Manager {
    pub fn new(interval: std::time::Duration) -> Self {
        Manager {
            inner: Arc::new(ManagerInner {
                states: Mutex::new(BTreeMap::new()),
                next_id: AtomicUsize::new(0),
                interval,
                out: Mutex::new(Box::new(std::io::stdout())),
                last_draw: Mutex::new(std::time::Instant::now() - interval),
                last_lines: AtomicUsize::new(0),
                ansi: Mutex::new(None),
            }),
        }
    }

    pub fn with_stdout(&self) -> Self {
        *self.inner.out.lock().unwrap() = Box::new(std::io::stdout());
        self.clone()
    }

    pub fn with_stderr(&self) -> Self {
        *self.inner.out.lock().unwrap() = Box::new(std::io::stderr());
        self.clone()
    }

    pub fn with_file(&self, file: std::fs::File) -> Self {
        *self.inner.out.lock().unwrap() = Box::new(file);
        self.clone()
    }

    pub fn auto_ansi(&self) -> Self {
        *self.inner.ansi.lock().unwrap() = None;
        self.clone()
    }

    pub fn force_ansi(&self, force: bool) -> Self {
        *self.inner.ansi.lock().unwrap() = Some(force);
        self.clone()
    }

    pub fn create_bar(&self, len: u64, message: &str, template: &str) -> Bar {
        let id = self
            .inner
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let bar_state = Arc::new(Mutex::new(BarState {
            len,
            pos: 0,
            message: message.to_string(),
            template: Template::new(template),
            created_at: std::time::Instant::now(),
        }));

        self.inner
            .states
            .lock()
            .unwrap()
            .insert(id, bar_state.clone());

        self.draw(true);

        Bar {
            manager: self.clone(),
            id,
            state: bar_state,
        }
    }

    pub fn draw(&self, force: bool) {
        let now = std::time::Instant::now();
        let mut last_draw = self.inner.last_draw.lock().unwrap();
        if !force && now - *last_draw < self.inner.interval {
            return;
        }

        let states = self.inner.states.lock().unwrap();
        let ansi = self.inner.ansi.lock().unwrap();
        let mut out = self.inner.out.lock().unwrap();
        let is_terminal = match *ansi {
            None => out.is_terminal(),
            Some(force) => force,
        };
        if is_terminal && states.len() > 0 {
            // Don't clean output when no bars are present
            for _ in 0..self
                .inner
                .last_lines
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                let _ = out.write_all(format!("{}{}", UP_ANSI, CLEAR_ANSI).as_bytes());
            }
        }

        let mut newlines = 0;
        for state in states.values() {
            let state = state.lock().unwrap();
            let outstr = format!("{}\n", state.render());
            if is_terminal {
                newlines += outstr.chars().filter(|&c| c == '\n').count();
            }
            let _ = out.write_all(outstr.as_bytes());
        }
        if is_terminal {
            self.inner
                .last_lines
                .store(newlines, std::sync::atomic::Ordering::Relaxed);
        }

        *last_draw = now;
    }
}

impl Drop for Manager {
    fn drop(&mut self) {
        self.draw(true);
    }
}

impl Bar {
    pub fn inc(&self, n: u64) {
        let mut state = self.state.lock().unwrap();
        state.pos += n;
        self.manager.draw(false);
    }

    pub fn set_pos(&self, pos: u64) {
        self.state.lock().unwrap().pos = pos;
        if pos != 0 {
            let len = self.state.lock().unwrap().len;
            if pos == len {
                self.manager.draw(true);
                return;
            }
        }
        self.manager.draw(false);
    }

    pub fn set_len(&self, len: u64) {
        self.state.lock().unwrap().len = len;
        self.manager.draw(false);
    }

    pub fn get_pos(&self) -> u64 {
        self.state.lock().unwrap().pos
    }

    pub fn get_len(&self) -> u64 {
        self.state.lock().unwrap().len
    }

    pub fn finish(self) {
        self.set_pos(self.get_len());
        // Automatically drop
    }
}

impl Drop for Bar {
    fn drop(&mut self) {
        self.manager.inner.states.lock().unwrap().remove(&self.id);
        self.manager.draw(true);
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Seek};

    use super::*;

    #[test]
    fn basic_test() {
        let manager = Manager::new(std::time::Duration::from_secs(1));
        let bar_1 = manager.create_bar(
            100,
            "Downloading",
            "{msg}\n[{elapsed}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
        );
        let bar_2 = manager.create_bar(
            100,
            "Uploading",
            "{msg}\n[{elapsed}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
        );

        bar_1.set_pos(50);
        bar_2.set_pos(25);

        std::mem::drop(bar_1);
        std::mem::drop(bar_2);
    }

    #[test]
    fn dont_crash_when_zero() {
        let manager = Manager::new(std::time::Duration::from_secs(1));
        let bar = manager.create_bar(
            0,
            "Downloading",
            "{msg}\n[{elapsed}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
        );

        bar.set_pos(0);
        manager.draw(true);
    }

    #[test]
    fn test_pb_to_file() {
        const TEMPLATE_SIMPLE: &str = "{msg}\n{bytes}/{total_bytes}";
        let memfd_name = std::ffi::CString::new("test_pb_to_file").unwrap();
        let memfd_fd =
            nix::sys::memfd::memfd_create(&memfd_name, nix::sys::memfd::MemFdCreateFlag::empty())
                .unwrap();
        let memfd_writer: std::fs::File = memfd_fd.into();
        let mut memfd_writer_clone = memfd_writer.try_clone().unwrap();
        let progressbar_manager =
            Manager::new(std::time::Duration::from_secs(1)).with_file(memfd_writer);
        let pb1 = progressbar_manager.create_bar(
            10,
            "Downloading http://d1.example.com/",
            TEMPLATE_SIMPLE,
        );
        let pb2 = progressbar_manager.create_bar(
            10,
            "Downloading http://d2.example.com/",
            TEMPLATE_SIMPLE,
        );

        pb1.set_pos(2);
        pb2.set_pos(3);
        progressbar_manager.draw(true);
        pb1.set_pos(5);
        pb2.set_pos(7);

        std::mem::drop(progressbar_manager);
        memfd_writer_clone
            .seek(std::io::SeekFrom::Start(0))
            .unwrap();
        let mut output = String::new();
        memfd_writer_clone.read_to_string(&mut output).unwrap();
        assert_eq!(
            output,
            r#"Downloading http://d1.example.com/
0 B/10 B
Downloading http://d1.example.com/
0 B/10 B
Downloading http://d2.example.com/
0 B/10 B
Downloading http://d1.example.com/
2 B/10 B
Downloading http://d2.example.com/
3 B/10 B
Downloading http://d1.example.com/
5 B/10 B
Downloading http://d2.example.com/
7 B/10 B
"#
        );
    }
}
