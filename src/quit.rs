//! Quitting the program: an orderly wrap-up, and a forced exit behind it.
//!
//! "종료" in the tray menu must end the program whatever the program is doing.
//! The request is therefore taken the moment the menu reports it (not at the
//! next frame), and a watchdog thread is started with it. The frame loop
//! wraps up in order (see `App::wrap_up`): tray icon, hot keys, background
//! workers, saving, windows. If the frame loop never gets to it, or stalls
//! half-way, the watchdog ends the process with `TerminateProcess` instead of
//! leaving a window or a tray icon behind a program that no longer answers.
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::{Duration, Instant};

/// How long the frame loop may take to even start wrapping up. It is not
/// busy with anything that matters here, so this only runs out when the loop
/// is blocked (a native dialog, a stuck driver call).
pub const START_DEADLINE: Duration = Duration::from_secs(3);
/// How long the whole wrap-up may take, saving included.
pub const FINISH_DEADLINE: Duration = Duration::from_secs(10);

const RUNNING: u8 = 0;
/// Quit was asked for; nothing has been wrapped up yet.
const REQUESTED: u8 = 1;
/// The frame loop is wrapping up.
const WRAPPING_UP: u8 = 2;

/// What the watchdog does about the time passed since the request.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Verdict {
    Wait,
    Terminate,
}

pub struct Quit {
    stage: AtomicU8,
    /// Frames seen after the wrap-up frame.
    later_passes: AtomicU8,
    start_deadline: Duration,
    finish_deadline: Duration,
    /// Whether `terminate` really ends the process (not in tests).
    real: bool,
    /// Set by a `terminate` that did not end the process.
    terminated: AtomicBool,
}

/// The program's one quit state.
pub static QUIT: Quit = Quit::real();

impl Quit {
    pub const fn real() -> Self {
        Self {
            stage: AtomicU8::new(RUNNING),
            later_passes: AtomicU8::new(0),
            start_deadline: START_DEADLINE,
            finish_deadline: FINISH_DEADLINE,
            real: !cfg!(test),
            terminated: AtomicBool::new(false),
        }
    }

    /// A quit state with short deadlines whose `terminate` only records that
    /// it was called.
    #[cfg(test)]
    pub fn dry() -> Self {
        Self {
            stage: AtomicU8::new(RUNNING),
            later_passes: AtomicU8::new(0),
            start_deadline: Duration::from_millis(300),
            finish_deadline: Duration::from_millis(900),
            real: false,
            terminated: AtomicBool::new(false),
        }
    }

    /// Quit was asked for. True for the first request only.
    pub fn request(&self) -> bool {
        self.stage
            .compare_exchange(RUNNING, REQUESTED, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub fn requested(&self) -> bool {
        self.stage.load(Ordering::SeqCst) != RUNNING
    }

    /// The frame loop starts wrapping up. False when it already did: the
    /// frame is then a later one, see `later_pass`.
    pub fn begin_wrap_up(&self) -> bool {
        self.stage
            .compare_exchange(REQUESTED, WRAPPING_UP, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// Counts a frame that came after the wrap-up frame and returns how many
    /// there have been, this one included.
    pub fn later_pass(&self) -> u8 {
        self.later_passes
            .fetch_add(1, Ordering::SeqCst)
            .saturating_add(1)
    }

    #[cfg(test)]
    pub fn wrapping_up(&self) -> bool {
        self.stage.load(Ordering::SeqCst) == WRAPPING_UP
    }

    /// What to do `since` after the request.
    pub fn verdict(&self, since: Duration) -> Verdict {
        let stage = self.stage.load(Ordering::SeqCst);
        let limit = if stage == REQUESTED {
            self.start_deadline
        } else {
            self.finish_deadline
        };
        if stage != RUNNING && since >= limit {
            Verdict::Terminate
        } else {
            Verdict::Wait
        }
    }

    /// Ends the process now, without running anything else.
    pub fn terminate(&self) {
        self.terminated.store(true, Ordering::SeqCst);
        if !self.real {
            return;
        }
        #[cfg(windows)]
        // SAFETY: ends this process; nothing runs afterwards.
        unsafe {
            use windows_sys::Win32::System::Threading::{GetCurrentProcess, TerminateProcess};
            TerminateProcess(GetCurrentProcess(), 0);
        }
        std::process::exit(0);
    }

    #[cfg(test)]
    pub fn was_terminated(&self) -> bool {
        self.terminated.load(Ordering::SeqCst)
    }

    /// Watches over the quit: ends the process when the frame loop does not
    /// do it in time.
    fn watch(&'static self, poll: Duration) {
        let started = Instant::now();
        let _ = std::thread::Builder::new()
            .name("quit-watchdog".into())
            .spawn(move || loop {
                std::thread::sleep(poll);
                if self.verdict(started.elapsed()) == Verdict::Terminate {
                    self.terminate();
                    return;
                }
            });
    }
}

/// Asks the program to quit and arms the watchdog. Safe to call from any
/// thread, any number of times, whatever the frame loop is doing.
pub fn request() {
    if QUIT.request() {
        QUIT.watch(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_first_request_counts_and_wrap_up_starts_once() {
        let q = Quit::dry();
        assert!(!q.requested());
        assert!(!q.begin_wrap_up(), "nothing to wrap up before a request");
        assert!(q.request());
        assert!(!q.request());
        assert!(q.requested() && !q.wrapping_up());
        assert!(q.begin_wrap_up());
        assert!(q.wrapping_up());
        assert!(!q.begin_wrap_up(), "a later frame is not a new wrap-up");
        assert_eq!((q.later_pass(), q.later_pass()), (1, 2));
    }

    #[test]
    fn the_watchdog_waits_for_a_loop_that_is_wrapping_up_but_not_for_a_silent_one() {
        let q = Quit::dry();
        let ms = Duration::from_millis;
        assert_eq!(q.verdict(ms(100_000)), Verdict::Wait, "not asked to quit");
        q.request();
        assert_eq!(q.verdict(ms(200)), Verdict::Wait);
        assert_eq!(q.verdict(ms(300)), Verdict::Terminate, "silent loop");
        q.begin_wrap_up();
        assert_eq!(q.verdict(ms(800)), Verdict::Wait);
        assert_eq!(q.verdict(ms(900)), Verdict::Terminate, "stalled wrap-up");
        // The real limits are the documented ones.
        assert_eq!(QUIT.start_deadline, Duration::from_secs(3));
        assert_eq!(QUIT.finish_deadline, Duration::from_secs(10));
    }

    #[test]
    fn a_blocked_frame_loop_is_ended_by_the_watchdog() {
        // Leaked so the watchdog thread can hold it, like the real static.
        let q: &'static Quit = Box::leak(Box::new(Quit::dry()));
        assert!(q.request());
        // Nothing ever calls `begin_wrap_up`: the loop is blocked.
        let started = Instant::now();
        q.watch(Duration::from_millis(10));
        while !q.was_terminated() && started.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(q.was_terminated(), "the watchdog must end a blocked loop");
        assert!(
            started.elapsed() >= q.start_deadline,
            "but not before the deadline"
        );
    }

    #[test]
    fn a_loop_that_stalls_while_wrapping_up_is_ended_later() {
        let q: &'static Quit = Box::leak(Box::new(Quit::dry()));
        assert!(q.request());
        q.watch(Duration::from_millis(10));
        assert!(q.begin_wrap_up());
        let started = Instant::now();
        std::thread::sleep(q.start_deadline + Duration::from_millis(200));
        assert!(
            !q.was_terminated(),
            "wrapping up has its own, longer deadline"
        );
        while !q.was_terminated() && started.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(q.was_terminated());
        assert!(started.elapsed() >= q.finish_deadline);
    }
}
