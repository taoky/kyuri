use std::{
    collections::HashMap,
    sync::{atomic::AtomicUsize, Arc, Mutex},
};

pub struct BarState {
    len: u64,
    pos: u64,
    message: String,
    template: String,
}

pub struct Bar {
    id: usize,
    state: Arc<Mutex<BarState>>,
    manager: Manager,
}

pub struct ManagerInner {
    states: Mutex<HashMap<usize, Arc<Mutex<BarState>>>>,
    next_id: AtomicUsize,
    refresh_interval: std::time::Duration,
    out: Mutex<Box<dyn Out>>,
    last_draw: std::time::Instant,
}

pub trait Out: std::io::Write + std::io::IsTerminal {}
impl<T: std::io::Write + std::io::IsTerminal> Out for T {}

#[derive(Clone)]
pub struct Manager {
    inner: Arc<ManagerInner>,
}

impl Manager {
    pub fn new(refresh_interval: std::time::Duration, out: Box<dyn Out>) -> Self {
        Manager {
            inner: Arc::new(ManagerInner {
                states: Mutex::new(HashMap::new()),
                next_id: AtomicUsize::new(0),
                refresh_interval,
                out: Mutex::new(out),
                last_draw: std::time::Instant::now() - refresh_interval,
            }),
        }
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
            template: template.to_string(),
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
        if !force {
            let now = std::time::Instant::now();
            if now - self.inner.last_draw < self.inner.refresh_interval {
                return;
            }
        }
        let states = self.inner.states.lock().unwrap();
        let mut out = self.inner.out.lock().unwrap();
        for state in states.values() {
            let state = state.lock().unwrap();
            out.write_all(
                format!(
                    "{} {} {} {}",
                    state.len, state.pos, state.message, state.template
                )
                .as_bytes(),
            )
            .unwrap();
        }
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
        let manager = Manager::new(
            std::time::Duration::from_secs(1),
            Box::new(std::io::stdout()),
        );
        let bar_1 = manager.create_bar(100, "Downloading", "<placeholder>");
        let bar_2 = manager.create_bar(100, "Uploading", "<placeholder>");

        bar_1.set_pos(50);
        bar_2.set_pos(25);

        std::mem::drop(bar_1);
        std::mem::drop(bar_2);
    }
}
