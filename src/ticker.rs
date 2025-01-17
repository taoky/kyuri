use std::{
    sync::{Arc, Condvar, Mutex, Weak},
    thread,
};

use crate::ManagerInner;

pub(crate) struct Ticker {
    // thread join requires ownership of the thread, so an Option is used to take it out
    thread: Option<thread::JoinHandle<()>>,
    condvar: Arc<(Mutex<bool>, Condvar)>,
    // manager: Weak<ManagerInner>,
}

impl Ticker {
    pub(crate) fn new(manager: Weak<ManagerInner>) -> Self {
        let condvar = Arc::new((Mutex::new(false), Condvar::new()));

        let condvar2 = Arc::clone(&condvar);
        let manager2 = Weak::clone(&manager);
        let thread = thread::spawn(move || {
            while let Some(manager) = manager2.upgrade() {
                let interval = manager.interval;
                let (lock, cvar) = &*condvar2;
                let done = cvar
                    .wait_timeout_while(lock.lock().unwrap(), interval, |stopped| !*stopped)
                    .unwrap();
                if !done.1.timed_out() {
                    break;
                }
                manager.draw(false);
            }
        });
        Self {
            thread: Some(thread),
            condvar,
            // manager,
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
