//! A simple progress display library.
//!
//! Kyuri is a simple progress display library. Different from [indicatif](https://github.com/console-rs/indicatif), it:
//! - Depends on std only when terminal support is unnecessary.
//!   - Custom features `console_width` and `unicode` are available for ANSI mode terminal width detection and Unicode width calculation.
//! - The `Manager` (like `MultiProgress` in indicatif) manages all progress bar management and rendering.
//! - Friendly to writing to files.
//! - Predictable about when it would draw.
//! - Custom integrations with other libraries (an example: examples/tracing.rs)
//!
//! ## Examples
//!
//! ```
//! use kyuri::Manager;
//!
//! const TEMPLATE: &str = "{msg}: {bar} ({pos}/{len})";
//! let manager = Manager::new(std::time::Duration::from_secs(1));
//!
//! let bar = manager.create_bar(100, "Processing", TEMPLATE, true);
//! for i in 0..=100 {
//!     bar.set_pos(i);
//!     std::thread::sleep(std::time::Duration::from_millis(1));
//! }
//! bar.finish_and_drop();
//! ```
//!
//! ## Template
//!
//! The template in Kyuri looks like the one in indicatif. However, only a very small subset is implemented, and some have different meanings.
//!
//! Tags in template looks like `{something}`. Supported tags:
//! - `{msg}`, `{message}`: The message of the bar.
//! - `{elapsed}`, `{elapsed_precise}`: The elapsed time (H:MM:SS).
//! - `{bytes}`: The current position in bytes (power-of-two, `KiB`, `MiB`, ...).
//! - `{pos}`: The current position.
//! - `{total_bytes}`: The total length in bytes (power-of-two, `KiB`, `MiB`, ...).
//! - `{total}`, `{len}`: The total length.
//! - `{bytes_per_sec}`, `{bytes_per_second}`: The current speed in bytes per second.
//! - `{eta}`: The estimated time of arrival (H:MM:SS).
//! - `{bar}`, `{barNUM}`: The progress bar. The `NUM` is the size of the bar, default is 20.
//! - `{state_emoji}`: The state emoji of the bar. ✅ for finished, 🆕 for new, 💥 for overflowed, ⏳ for in progress.
//!
//! Doubled `{` and `}` would not be interpreted as tags.

#![warn(missing_docs)]

use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, AtomicUsize},
        Arc, Mutex, Weak,
    },
};

mod template;
mod ticker;
pub mod writer;
use template::{Template, TemplatePart};
use termsize::get_width;
use ticker::Ticker;
mod termsize;

const CLEAR_ANSI: &str = "\r\x1b[K";
const UP_ANSI: &str = "\x1b[F";

pub(crate) struct BarState {
    len: u64,
    pos: u64,
    message: String,
    template: Template,
    created_at: std::time::Instant,
    visible: bool,
    /// Note that need_redraw for individual bars would only be respected when output is not a terminal.
    need_redraw: bool,
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

fn string_width(s: &str) -> usize {
    #[cfg(feature = "unicode")]
    {
        unicode_width::UnicodeWidthStr::width(s)
    }

    #[cfg(not(feature = "unicode"))]
    {
        s.chars().count()
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
                    if *size >= filled {
                        let empty = *size - filled;
                        result.push('[');
                        for _ in 0..filled {
                            result.push('=');
                        }
                        for _ in 0..empty {
                            result.push(' ');
                        }
                        result.push(']');
                    } else {
                        let overflowed = filled - *size;
                        result.push('[');
                        for _ in 0..*size {
                            result.push('=');
                        }
                        for _ in 0..overflowed {
                            result.push('!');
                        }
                    }
                }
                TemplatePart::StateEmoji => {
                    if self.pos == self.len {
                        result.push_str("✅");
                    } else if self.pos == 0 {
                        result.push_str("🆕");
                    } else if self.pos > self.len {
                        result.push_str("💥");
                    } else {
                        // 0 < self.pos < self.len
                        result.push_str("⏳");
                    }
                }
            }
        }
        result
    }
}

/// A handle for users to control a progress bar created by `Manager`.
pub struct Bar {
    id: usize,
    manager: Weak<ManagerInner>,
}

/// Lock order:
/// - last_draw
/// - out
/// - states
pub(crate) struct ManagerInner {
    states: Mutex<BTreeMap<usize, Arc<Mutex<BarState>>>>,
    ansi: Mutex<Option<bool>>,
    interval: std::time::Duration,
    pub(crate) out: Arc<Mutex<Box<dyn Out>>>,
    ticker: Mutex<Option<Ticker>>,

    // interval states
    next_id: AtomicUsize,
    last_draw: Mutex<std::time::Instant>,
    last_lines: AtomicUsize,
    need_redraw: AtomicBool,
}

impl ManagerInner {
    pub(crate) fn is_ticker_enabled(&self) -> bool {
        self.ticker.lock().unwrap().is_some()
    }

    /// This is expected to be called only when it's ANSI mode.
    pub(crate) fn clear_existing(&self, out: &mut Box<dyn Out>) {
        for _ in 0..self.last_lines.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = out.write_all(format!("{}{}", UP_ANSI, CLEAR_ANSI).as_bytes());
        }
    }

    pub(crate) fn is_terminal(&self, out: &mut Box<dyn Out>) -> bool {
        let ansi = self.ansi.lock().unwrap();
        match *ansi {
            None => out.is_terminal(),
            Some(force) => force,
        }
    }

    pub(crate) fn draw_inner(
        &self,
        states: &BTreeMap<usize, Arc<Mutex<BarState>>>,
        out: &mut Box<dyn Out>,
        is_terminal: bool,
    ) {
        let mut newlines = 0;
        for state in states.values() {
            let mut state = state.lock().unwrap();
            if !state.visible {
                continue;
            }
            if !is_terminal && !state.need_redraw {
                continue;
            }
            let outstr = format!("{}\n", state.render());
            if is_terminal {
                let splits = outstr.split('\n');
                let term_col = get_width(out.as_ref()) as usize;
                for i in splits {
                    let width = string_width(i);
                    newlines += width / term_col;
                    if width % term_col != 0 {
                        newlines += 1;
                    }
                }
            }
            let _ = out.write_all(outstr.as_bytes());
            state.need_redraw = false;
        }
        if is_terminal {
            self.last_lines
                .store(newlines, std::sync::atomic::Ordering::Relaxed);
        }
    }

    pub(crate) fn mark_redraw(&self) {
        self.need_redraw
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub(crate) fn draw(&self, force: bool) {
        if !force && self.is_ticker_enabled() {
            return;
        }
        let now = std::time::Instant::now();
        let mut last_draw = self.last_draw.lock().unwrap();
        if !force && now - *last_draw < self.interval {
            return;
        }

        if !self
            .need_redraw
            .swap(false, std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }
        let mut out = self.out.lock().unwrap();
        let states = self.states.lock().unwrap();
        let is_terminal = self.is_terminal(&mut out);
        if is_terminal && states.len() > 0 {
            // Don't clean output when no bars are present
            self.clear_existing(&mut out);
        }

        self.draw_inner(&states, &mut out, is_terminal);

        *last_draw = now;
    }

    pub(crate) fn suspend<F: FnOnce(&mut Box<dyn Out>) -> R, R>(&self, f: F) -> R {
        let mut out = self.out.lock().unwrap();
        let is_terminal = self.is_terminal(&mut out);
        if is_terminal {
            self.clear_existing(&mut out);
        }
        let result = f(&mut out);
        if is_terminal {
            let states = self.states.lock().unwrap();
            self.draw_inner(&states, &mut out, is_terminal);
        }
        result
    }
}

/// Trait for progress output streams, requires Unix file descriptor support.
/// `std::io::stdout`, `std::io::stderr` and `std::fs::File` implement this trait.
#[cfg(all(unix, feature = "console_width"))]
pub trait Out: std::io::Write + std::io::IsTerminal + std::os::fd::AsRawFd + Send + Sync {}
#[cfg(all(unix, feature = "console_width"))]
impl<T: std::io::Write + std::io::IsTerminal + std::os::fd::AsRawFd + Send + Sync> Out for T {}

/// Trait for progress output streams, requires Windows HANDLE support.
/// `std::io::stdout`, `std::io::stderr` and `std::fs::File` implement this trait.
#[cfg(all(windows, feature = "console_width"))]
pub trait Out:
    std::io::Write + std::io::IsTerminal + std::os::windows::io::AsRawHandle + Send + Sync
{
}
#[cfg(all(windows, feature = "console_width"))]
impl<T: std::io::Write + std::io::IsTerminal + std::os::windows::io::AsRawHandle + Send + Sync> Out
    for T
{
}

/// Trait for progress output streams.
/// `std::io::stdout`, `std::io::stderr` and `std::fs::File` implement this trait.
#[cfg(not(any(
    all(windows, feature = "console_width"),
    all(unix, feature = "console_width")
)))]
pub trait Out: std::io::Write + std::io::IsTerminal + Send + Sync {}
#[cfg(not(any(
    all(windows, feature = "console_width"),
    all(unix, feature = "console_width")
)))]
impl<T: std::io::Write + std::io::IsTerminal + Send + Sync> Out for T {}

/// The manager for progress bars. It's expected for users to create a `Manager`, create progress bars from it,
/// and drop it when all work has been done.
///
/// When manager is dropped, it would force a draw. After that bars would not be able to be interacted with.
pub struct Manager {
    inner: Arc<ManagerInner>,
}

impl Manager {
    /// Create a new `Manager` to stdout.
    ///
    /// The `interval` parameter specifies the minimum interval between two unforced draws.
    pub fn new(interval: std::time::Duration) -> Self {
        Manager {
            inner: Arc::new(ManagerInner {
                states: Mutex::new(BTreeMap::new()),
                next_id: AtomicUsize::new(0),
                interval,
                out: Arc::new(Mutex::new(Box::new(std::io::stdout()))),
                last_draw: Mutex::new(std::time::Instant::now() - interval),
                last_lines: AtomicUsize::new(0),
                ansi: Mutex::new(None),
                need_redraw: AtomicBool::new(false),
                ticker: Mutex::new(None),
            }),
        }
    }

    fn mark_redraw(&self) {
        self.inner.mark_redraw();
    }

    /// Set the `Manager` to write to stdout.
    pub fn with_stdout(self) -> Self {
        *self.inner.out.lock().unwrap() = Box::new(std::io::stdout());
        self.mark_redraw();
        self
    }

    /// Set the `Manager` to write to stderr.
    pub fn with_stderr(self) -> Self {
        *self.inner.out.lock().unwrap() = Box::new(std::io::stderr());
        self.mark_redraw();
        self
    }

    /// Set the `Manager` to write to a file.
    pub fn with_file(self, file: std::fs::File) -> Self {
        *self.inner.out.lock().unwrap() = Box::new(file);
        self.mark_redraw();
        self
    }

    /// Let `Manager` automatically detect whether it's writing to a terminal and use ANSI or not.
    pub fn auto_ansi(self) -> Self {
        *self.inner.ansi.lock().unwrap() = None;
        self.mark_redraw();
        self
    }

    /// Force `Manager` to use ANSI escape codes or not.
    pub fn force_ansi(self, force: bool) -> Self {
        *self.inner.ansi.lock().unwrap() = Some(force);
        self.mark_redraw();
        self
    }

    /// Ticker enables a background thread to draw progress bars at a fixed interval.
    ///
    /// When ticker is enabled, unforced draw would be ignored.
    pub fn set_ticker(&self, set_ticker: bool) {
        let mut ticker = self.inner.ticker.lock().unwrap();
        if set_ticker && ticker.is_none() {
            *ticker = Some(Ticker::new(self.inner.clone()));
        } else if !set_ticker && ticker.is_some() {
            *ticker = None;
        }
    }

    /// Create a new progress bar.
    ///
    /// - `len`: The total length of the progress bar.
    /// - `message`: The message of the bar. Use `{msg}` in the template to refer to this.
    /// - `template`: The template of the bar.
    /// - `visible`: Whether the bar is visible.
    ///
    /// This makes a forced draw when visible is true.
    pub fn create_bar(&self, len: u64, message: &str, template: &str, visible: bool) -> Bar {
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
            visible,
            need_redraw: true,
        }));

        self.inner
            .states
            .lock()
            .unwrap()
            .insert(id, bar_state.clone());

        if visible {
            self.mark_redraw();
            self.draw(true);
        }

        Bar {
            manager: Arc::downgrade(&self.inner),
            id,
        }
    }

    /// Draw all progress bars. In most cases it's not necessary to call this manually.
    ///
    /// If nothing changed, it would not draw no matter what.
    ///
    /// If ticker is enabled, unforced draw would be ignored. Otherwise, it would only draw when the interval has passed.
    ///
    /// Progress bars would be drawn by the order of `Bar` creation. In ANSI mode, it would clear the previous output.
    ///
    /// Finally, when output is not a terminal, bars would be drawn only when it needs to be redrawn.
    pub fn draw(&self, force: bool) {
        self.inner.draw(force);
    }

    /// Hide all progress bars, run the closure, and show them again like indicatif::MultiProgress::suspend.
    ///
    /// This method is used for implementing integrations with other libraries that may print to the terminal.
    ///
    /// When output is not a terminal, the closure would still be run but nothing would be done to the progress bars.
    pub fn suspend<F: FnOnce(&mut Box<dyn Out>) -> R, R>(&self, f: F) -> R {
        self.inner.suspend(f)
    }

    /// Create a writer for integration with other libraries.
    pub fn create_writer(&self) -> writer::KyuriWriter {
        writer::KyuriWriter::new(self.inner.clone())
    }
}

impl Drop for ManagerInner {
    /// Force a draw when the `ManagerInner` is dropped.
    fn drop(&mut self) {
        self.draw(true);
    }
}

impl Bar {
    fn get_manager_and_state(&self) -> Option<(Arc<ManagerInner>, Arc<Mutex<BarState>>)> {
        let manager = self.manager.upgrade()?;
        let state = manager.states.lock().unwrap().get(&self.id)?.clone();
        Some((manager, state))
    }

    /// Increment the progress bar by `n`. This makes an unforced draw.
    pub fn inc(&self, n: u64) {
        if let Some((manager, state)) = self.get_manager_and_state() {
            let mut state = state.lock().unwrap();
            state.pos += n;
            state.need_redraw = true;
            // Drop state before drawing, deadlock otherwise!
            std::mem::drop(state);
            manager.mark_redraw();
            manager.draw(false);
        }
    }

    /// Set the position of the progress bar. This makes an unforced draw.
    pub fn set_pos(&self, pos: u64) {
        if let Some((manager, state)) = self.get_manager_and_state() {
            let mut state = state.lock().unwrap();
            state.pos = pos;
            state.need_redraw = true;
            // Drop state before drawing, deadlock otherwise!
            std::mem::drop(state);
            manager.mark_redraw();
            manager.draw(false);
        }
    }

    /// Set the total length of the progress bar. This makes an unforced draw.
    pub fn set_len(&self, len: u64) {
        if let Some((manager, state)) = self.get_manager_and_state() {
            let mut state = state.lock().unwrap();
            state.len = len;
            state.need_redraw = true;
            // Drop state before drawing, deadlock otherwise!
            std::mem::drop(state);
            manager.mark_redraw();
            manager.draw(false);
        }
    }

    /// Get the position of the progress bar.
    ///
    /// When manager is dropped, this would return 0
    pub fn get_pos(&self) -> u64 {
        self.get_manager_and_state()
            .map_or(0, |(_, state)| state.lock().unwrap().pos)
    }

    /// Get the total length of the progress bar.
    ///
    /// When manager is dropped, this would return 0
    pub fn get_len(&self) -> u64 {
        self.get_manager_and_state()
            .map_or(0, |(_, state)| state.lock().unwrap().len)
    }

    /// Set the progress bar to the end, and force a draw.
    pub fn finish(&self) {
        if let Some((manager, state)) = self.get_manager_and_state() {
            let state = state.lock().unwrap();
            let pos = state.pos;
            let len = state.len;
            if pos != len {
                self.set_pos(len);
            }
            std::mem::drop(state);
            manager.draw(true);
        }
    }

    /// Set the progress bar to the end, force a draw, and remove the progress bar from the manager.
    pub fn finish_and_drop(self) {
        self.finish();
        // Automatically drop
    }

    /// Set the visibility of the progress bar. This makes an forced draw when visible actually changes.
    pub fn set_visible(&self, visible: bool) {
        if let Some((manager, state)) = self.get_manager_and_state() {
            let mut state = state.lock().unwrap();
            if state.visible != visible {
                state.visible = visible;
                state.need_redraw = true;
                // Drop state before drawing, deadlock otherwise!
                std::mem::drop(state);
                manager.mark_redraw();
                manager.draw(true);
            }
        }
    }

    /// Get the visibility of the progress bar.
    ///
    /// When manager is dropped, this would return false
    pub fn is_visible(&self) -> bool {
        self.get_manager_and_state()
            .map_or(false, |(_, state)| state.lock().unwrap().visible)
    }

    /// Set the message of the progress bar. This makes an unforced draw.
    pub fn set_message(&self, message: &str) {
        if let Some((manager, state)) = self.get_manager_and_state() {
            let mut state = state.lock().unwrap();
            state.message = message.to_string();
            state.need_redraw = true;
            // Drop state before drawing, deadlock otherwise!
            std::mem::drop(state);
            manager.mark_redraw();
            manager.draw(false);
        }
    }

    /// Set the template of the progress bar. This makes an unforced draw.
    pub fn set_template(&self, template: &str) {
        if let Some((manager, state)) = self.get_manager_and_state() {
            let mut state = state.lock().unwrap();
            state.template = Template::new(template);
            state.need_redraw = true;
            // Drop state before drawing, deadlock otherwise!
            std::mem::drop(state);
            manager.mark_redraw();
            manager.draw(false);
        }
    }

    /// Return whether the progress bar (the manager) is still alive.
    ///
    /// When the manager is dropped, the progress bar would not be able to be interacted with.
    pub fn alive(&self) -> bool {
        self.get_manager_and_state().is_some()
    }
}

impl Drop for Bar {
    /// Drop the progress bar. This removes the progress bar from the manager and forces a draw.
    fn drop(&mut self) {
        if let Some((manager, _)) = self.get_manager_and_state() {
            manager.states.lock().unwrap().remove(&self.id);
            manager.mark_redraw();
            manager.draw(true);
        }
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
            true,
        );
        let bar_2 = manager.create_bar(
            100,
            "Uploading",
            "{msg}\n[{elapsed}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
            true,
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
            true,
        );

        bar.set_pos(0);
        manager.draw(true);
    }

    #[test]
    fn inc() {
        let manager = Manager::new(std::time::Duration::from_secs(1));
        let bar = manager.create_bar(
            100,
            "Downloading",
            "{msg}\n[{elapsed}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
            true,
        );

        bar.inc(10);
        bar.inc(10);
        bar.inc(10);
        bar.inc(10);
        bar.inc(10);

        assert_eq!(bar.get_pos(), 50);

        std::mem::drop(bar);
    }

    #[test]
    fn visible() {
        let manager = Manager::new(std::time::Duration::from_secs(1));
        let bar = manager.create_bar(
            100,
            "Downloading",
            "{msg}\n[{elapsed}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
            true,
        );

        assert_eq!(bar.is_visible(), true);

        bar.set_visible(false);
        assert_eq!(bar.is_visible(), false);

        std::mem::drop(bar);
    }

    #[test]
    fn ticker() {
        let manager = Manager::new(std::time::Duration::from_secs(1));
        manager.set_ticker(true);
        let bar = manager.create_bar(
            100,
            "Downloading",
            "{msg}\n[{elapsed}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
            true,
        );

        std::thread::sleep(std::time::Duration::from_secs(2));
        std::mem::drop(bar);
    }

    #[test]
    fn alive() {
        let manager = Manager::new(std::time::Duration::from_secs(1));
        let bar = manager.create_bar(
            100,
            "Downloading",
            "{msg}\n[{elapsed}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
            true,
        );

        assert_eq!(bar.alive(), true);

        std::mem::drop(manager);
        assert_eq!(bar.alive(), false);
    }

    #[cfg(target_os = "linux")]
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
            true,
        );
        let pb2 = progressbar_manager.create_bar(
            10,
            "Downloading http://d2.example.com/",
            TEMPLATE_SIMPLE,
            true,
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
