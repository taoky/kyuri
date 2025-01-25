// https://github.com/console-rs/console/blob/f37cb6e7bc575b38bcdc0111457b60ca2d71cdd5/src/unix_term.rs#L48

use super::DEFAULT_WIDTH;
use std::mem;

pub(crate) fn get_width<T: std::os::fd::AsRawFd + ?Sized>(f: &T) -> u16 {
    unsafe {
        let mut winsize: libc::winsize = mem::zeroed();
        libc::ioctl(f.as_raw_fd(), libc::TIOCGWINSZ.into(), &mut winsize);
        if winsize.ws_col == 0 {
            DEFAULT_WIDTH
        } else {
            winsize.ws_col as u16
        }
    }
}
