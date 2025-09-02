// https://github.com/console-rs/console/blob/f37cb6e7bc575b38bcdc0111457b60ca2d71cdd5/src/windows_term/mod.rs#L109

use super::DEFAULT_WIDTH;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::System::Console::{SMALL_RECT, COORD, CONSOLE_SCREEN_BUFFER_INFO, GetConsoleScreenBufferInfo};

pub(crate) fn get_width<T: std::os::windows::io::AsRawHandle + ?Sized>(f: &T) -> u16 {
    let handle = f.as_raw_handle();
    let hand = handle as windows_sys::Win32::Foundation::HANDLE;

    if hand == INVALID_HANDLE_VALUE {
        return DEFAULT_WIDTH;
    }

    let zc = COORD { X: 0, Y: 0 };
    let mut csbi = CONSOLE_SCREEN_BUFFER_INFO {
        dwSize: zc,
        dwCursorPosition: zc,
        wAttributes: 0,
        srWindow: SMALL_RECT {
            Left: 0,
            Top: 0,
            Right: 0,
            Bottom: 0,
        },
        dwMaximumWindowSize: zc,
    };
    if unsafe { GetConsoleScreenBufferInfo(hand, &mut csbi) } == 0 {
        return DEFAULT_WIDTH;
    }

    (csbi.srWindow.Right - csbi.srWindow.Left + 1) as u16
}
