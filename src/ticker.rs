use std::{
    sync::{Arc, Condvar, Mutex},
    thread,
};

use crate::ManagerInner;

pub(crate) struct Ticker {
    // thread join requires ownership of the thread, so an Option is used to take it out
    thread: Option<thread::JoinHandle<()>>,
    condvar: Arc<(Mutex<bool>, Condvar)>,
}

impl Ticker {
    pub(crate) fn new(manager: Arc<ManagerInner>) -> Self {
        let condvar = Arc::new((Mutex::new(false), Condvar::new()));

        let condvar2 = Arc::clone(&condvar);
        let manager = Arc::downgrade(&manager);
        let thread = thread::spawn(move || {
            while let Some(manager) = manager.upgrade() {
                let interval = manager.interval;
                let (lock, cvar) = &*condvar2;
                let done = cvar
                    .wait_timeout_while(lock.lock().unwrap(), interval, |stopped| !*stopped)
                    .unwrap();
                if !done.1.timed_out() {
                    break;
                }
                // When ticker is on, unforced draw is ignored.
                manager.draw(true);
            }
        });
        Self {
            thread: Some(thread),
            condvar,
        }
    }

    pub(crate) fn stop(&self) {
        let (lock, cvar) = &*self.condvar;
        *lock.lock().unwrap() = true;
        cvar.notify_one();
    }
}

impl Drop for Ticker {
    fn drop(&mut self) {
        self.stop();
        if let Some(t) = self.thread.take() {
            t.join().unwrap();
        }
    }
}
