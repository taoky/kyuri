//! The modules contains `KyuriWriter`, a wrapper used with other libraries.

use std::sync::{Arc, Mutex, Weak};

use crate::{ManagerInner, Out};

/// A writer wrapping the output writer, that can be used to write to the output.
///
/// When the manager is dropped, the writer will continue to write to the original output writer.
pub struct KyuriWriter {
    manager: Weak<ManagerInner>,
    // A copy of the output writer, to use when the manager is dropped
    out: Arc<Mutex<Box<dyn Out>>>,
}

impl KyuriWriter {
    pub(crate) fn new(manager: Arc<ManagerInner>) -> Self {
        KyuriWriter {
            manager: Arc::downgrade(&manager),
            out: manager.out.clone(),
        }
    }
}

impl std::io::Write for KyuriWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Some(manager) = self.manager.upgrade() {
            manager.suspend(|out| out.write(buf))
        } else {
            self.out.lock().unwrap().write(buf)
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if let Some(manager) = self.manager.upgrade() {
            manager.suspend(|out| out.flush())
        } else {
            self.out.lock().unwrap().flush()
        }
    }

    fn write_vectored(&mut self, bufs: &[std::io::IoSlice<'_>]) -> std::io::Result<usize> {
        if let Some(manager) = self.manager.upgrade() {
            manager.suspend(|out| out.write_vectored(bufs))
        } else {
            self.out.lock().unwrap().write_vectored(bufs)
        }
    }

    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        if let Some(manager) = self.manager.upgrade() {
            manager.suspend(|out| out.write_all(buf))
        } else {
            self.out.lock().unwrap().write_all(buf)
        }
    }

    fn write_fmt(&mut self, fmt: std::fmt::Arguments<'_>) -> std::io::Result<()> {
        if let Some(manager) = self.manager.upgrade() {
            manager.suspend(|out| out.write_fmt(fmt))
        } else {
            self.out.lock().unwrap().write_fmt(fmt)
        }
    }
}
