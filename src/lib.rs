use std::{
    collections::HashMap,
    sync::{atomic::AtomicUsize, Arc, Mutex},
};

mod template;
use template::{Template, TemplatePart};

const CLEAR_ANSI: &str = "\r\x1b[K";
const UP_ANSI: &str = "\x1b[F";
const DOWN_ANSI: &str = "\x1b[E";

pub struct BarState {
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
    states: Mutex<HashMap<usize, Arc<Mutex<BarState>>>>,
    next_id: AtomicUsize,
    refresh_interval: std::time::Duration,
    out: Mutex<Box<dyn Out>>,
    last_draw: Mutex<std::time::Instant>,
}

pub trait Out: std::io::Write + std::io::IsTerminal + Send + Sync {}
impl<T: std::io::Write + std::io::IsTerminal + Send + Sync> Out for T {}

#[derive(Clone)]
pub struct Manager {
    inner: Arc<ManagerInner>,
}

impl Manager {
    pub fn new(refresh_interval: std::time::Duration) -> Self {
        Manager {
            inner: Arc::new(ManagerInner {
                states: Mutex::new(HashMap::new()),
                next_id: AtomicUsize::new(0),
                refresh_interval,
                out: Mutex::new(Box::new(std::io::stdout())),
                last_draw: Mutex::new(std::time::Instant::now() - refresh_interval),
            }),
        }
    }

    pub fn with_stderr(&self) -> Self {
        *self.inner.out.lock().unwrap() = Box::new(std::io::stderr());
        self.clone()
    }

    pub fn with_file(&self, file: std::fs::File) -> Self {
        *self.inner.out.lock().unwrap() = Box::new(file);
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
            // message must not contain control characters like \n
            message: message.chars().filter(|c| !c.is_control()).collect(),
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
        if !force && now - *last_draw < self.inner.refresh_interval {
            return;
        }

        let states = self.inner.states.lock().unwrap();
        let mut out = self.inner.out.lock().unwrap();
        for state in states.values() {
            let state = state.lock().unwrap();
            out.write_all(format!("{}\n", state.render()).as_bytes())
                .unwrap();
        }
        *last_draw = now;
    }
}

impl Bar {
    pub fn set_pos(&self, pos: u64) {
        {
            self.state.lock().unwrap().pos = pos;
        }
        self.manager.draw(false);
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
    use super::*;

    #[test]
    fn basic_test() {
        let manager = Manager::new(std::time::Duration::from_secs(1));
        let bar_1 = manager.create_bar(100, "Downloading", "<placeholder>");
        let bar_2 = manager.create_bar(100, "Uploading", "<placeholder>");

        bar_1.set_pos(50);
        bar_2.set_pos(25);

        std::mem::drop(bar_1);
        std::mem::drop(bar_2);
    }
}
