//! One-line display of the check in the interactive terminal.
//!
//! This is the design of the adapter: for the duration of the wait, it hides the input and
//! then restores the display, without making a decision about the result of the check.

use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

use carosign_l10n::{FluentArgs, Localizer};

const FRAMES: [&str; 4] = ["⠋", "⠙", "⠹", "⠸"];

fn wait_limit(
    deadline: Option<&carosign_protocol::carosign::common::v1::BootDeadline>,
) -> Duration {
    let limit = carosign_linux_ipc::client::verification_events::VERIFICATION_WAIT_LIMIT;
    let Some(deadline) = deadline else {
        return limit;
    };
    let Ok(boot) = std::fs::read_to_string("/proc/sys/kernel/random/boot_id") else {
        return limit;
    };
    let hex: String = boot.trim().chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 || deadline.boot_id.len() != 16 {
        return limit;
    }
    let parsed = (0..16)
        .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16))
        .collect::<Result<Vec<_>, _>>();
    if parsed.as_deref() != Ok(deadline.boot_id.as_slice()) {
        return limit;
    }
    let mut now: libc::timespec = unsafe { std::mem::zeroed() };
    if unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut now) } != 0
        || now.tv_sec < 0
        || now.tv_nsec < 0
    {
        return limit;
    }
    let now_ns = (now.tv_sec as u64)
        .saturating_mul(1_000_000_000)
        .saturating_add(now.tv_nsec as u64);
    limit.min(Duration::from_nanos(
        deadline.boottime_ns.saturating_sub(now_ns),
    ))
}

/// The terminal of the running application only if it manages it itself.
pub struct TtyUi {
    tty: std::fs::File,
    localizer: Arc<Localizer>,
    original_echo: libc::tcflag_t,
    dirty: bool,
    shown_at: Option<Instant>,
    attempts: Arc<AtomicUsize>,
    limit: Duration,
    continuous: bool,
}

impl TtyUi {
    /// Graphic and non-interactive calls should use PAM conversation.
    pub fn open(service: &str, localizer: Arc<Localizer>) -> Option<Self> {
        if !matches!(service, "sudo" | "sudo-i" | "login")
            || std::env::var("TERM")
                .ok()
                .is_none_or(|t| t.is_empty() || t == "dumb")
        {
            return None;
        }
        // The stdin and stderr must belong to the same terminal: checking one /dev/tty
        // mistakenly detects it in some graphics processes.
        unsafe {
            if libc::isatty(0) != 1 || libc::isatty(2) != 1 {
                return None;
            }
            let mut input: libc::stat = std::mem::zeroed();
            let mut output: libc::stat = std::mem::zeroed();
            let mut termios: libc::termios = std::mem::zeroed();
            if libc::fstat(0, &mut input) != 0
                || libc::fstat(2, &mut output) != 0
                || input.st_rdev != output.st_rdev
                || libc::tcgetpgrp(0) != libc::getpgrp()
                || libc::tcgetattr(0, &mut termios) != 0
                || termios.c_lflag & libc::ICANON == 0
            {
                return None;
            }
        }
        let tty = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .ok()?;
        Self::from_tty(tty, localizer)
    }

    fn from_tty(tty: std::fs::File, localizer: Arc<Localizer>) -> Option<Self> {
        let mut settings: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(tty.as_raw_fd(), &mut settings) } != 0 {
            return None;
        }
        let mask = libc::ECHO | libc::ECHONL;
        let original_echo = settings.c_lflag & mask;
        settings.c_lflag &= !mask;
        if unsafe { libc::tcsetattr(tty.as_raw_fd(), libc::TCSANOW, &settings) } != 0 {
            return None;
        }
        Some(Self {
            tty,
            localizer,
            original_echo,
            dirty: false,
            shown_at: None,
            attempts: Arc::new(AtomicUsize::new(1)),
            limit: carosign_linux_ipc::client::verification_events::VERIFICATION_WAIT_LIMIT,
            continuous: false,
        })
    }

    /// The first frame is drawn before returning to the reception of service events.
    pub fn show(
        &mut self,
        attempts: Arc<AtomicUsize>,
        deadline: Option<&carosign_protocol::carosign::common::v1::BootDeadline>,
        continuous: bool,
    ) {
        self.attempts = attempts;
        self.continuous = continuous;
        self.limit = wait_limit(deadline);
        self.shown_at = Some(Instant::now());
        self.draw(0);
    }

    /// Waits for the check or input line to be completed; Enter asks for cancellation.
    /// Stopping a render does not mean a successful cancellation on the service side.
    pub fn wait(mut self, done: Receiver<()>) -> bool {
        let mut frame = 0;
        loop {
            match done.try_recv() {
                Ok(()) | Err(TryRecvError::Disconnected) => {
                    // A quick response should not erase the message before the person has
                    // time to see it.
                    if let Some(shown) = self.shown_at {
                        std::thread::sleep(
                            Duration::from_millis(350).saturating_sub(shown.elapsed()),
                        );
                    }
                    return false;
                }
                Err(TryRecvError::Empty) => {}
            }
            self.draw(frame);
            frame += 1;
            let mut fd = libc::pollfd {
                fd: self.tty.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: fd is valid, poll does not store the address.
            let ready =
                unsafe { libc::poll(&mut fd, 1, Duration::from_millis(100).as_millis() as i32) };
            if ready > 0 && fd.revents & libc::POLLIN != 0 {
                // In canonical mode, an empty string takes up one byte.
                // Early input is not transmitted to either the PAM or the subsequent shell.
                let mut count: libc::c_int = 0;
                if unsafe { libc::ioctl(self.tty.as_raw_fd(), libc::FIONREAD, &mut count) } != 0 {
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }
                if count > 1 {
                    return true;
                }
                let mut input = [0u8; 1];
                match self.tty.read(&mut input) {
                    Ok(n) if input[..n].contains(&b'\n') || input[..n].contains(&b'\r') => {
                        return true;
                    }
                    Ok(0) | Err(_) => return false,
                    _ => return true,
                }
            } else if ready < 0
                || fd.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0
            {
                return false;
            }
        }
    }

    fn draw(&mut self, frame: usize) {
        let mut size: libc::winsize = unsafe { std::mem::zeroed() };
        let columns = if unsafe { libc::ioctl(self.tty.as_raw_fd(), libc::TIOCGWINSZ, &mut size) }
            == 0
            && size.ws_col > 0
        {
            size.ws_col as usize
        } else {
            80
        };
        let remaining = self.limit.saturating_sub(
            self.shown_at
                .map_or(Duration::ZERO, |start| start.elapsed()),
        );
        let seconds = remaining.as_secs() + u64::from(remaining.subsec_nanos() > 0);
        let mut args = FluentArgs::new();
        args.set("attempt", self.attempts.load(Ordering::Relaxed));
        let key = if self.continuous {
            args.set("total", crate::pam_ffi::SUDO_MAX_ATTEMPTS);
            "tty-verifying-continuous"
        } else {
            args.set("seconds", seconds);
            "tty-verifying"
        };
        let message = self.localizer.format(key, Some(&args));
        let text = format!("{}  {message}", FRAMES[frame % FRAMES.len()]);
        let visible: String = text.chars().take(columns.saturating_sub(1)).collect();
        let _ = write!(self.tty, "\r\x1b[2K{visible}");
        let _ = self.tty.flush();
        self.dirty = true;
    }
}

impl Drop for TtyUi {
    fn drop(&mut self) {
        // An early password should not become a command after a successful verification.
        let _ = unsafe { libc::tcflush(self.tty.as_raw_fd(), libc::TCIFLUSH) };
        if self.dirty {
            let _ = self.tty.write_all(b"\r\x1b[2K");
            let _ = self.tty.flush();
        }
        let mut settings: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(self.tty.as_raw_fd(), &mut settings) } == 0 {
            let mask = libc::ECHO | libc::ECHONL;
            settings.c_lflag = (settings.c_lflag & !mask) | self.original_echo;
            let _ = unsafe { libc::tcsetattr(self.tty.as_raw_fd(), libc::TCSANOW, &settings) };
        }
    }
}

#[path = "tty_ui.tests.rs"]
#[cfg(test)]
mod tests;
