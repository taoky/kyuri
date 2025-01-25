// Uses code from termsize and console crates.

const DEFAULT_WIDTH: u16 = 80;

#[cfg(all(windows, feature = "console_width"))]
#[path = "windows.rs"]
mod imp;

#[cfg(all(unix, feature = "console_width"))]
#[path = "unix.rs"]
mod imp;

#[cfg(not(any(
    all(windows, feature = "console_width"),
    all(unix, feature = "console_width")
)))]
#[path = "non.rs"]
mod imp;

pub(crate) use imp::get_width;
