//! The backend tick (ADR-022): a thread calls `Session::tick` every 100 ms,
//! or at once when woken, and sends the events it returns to the window.

use std::panic::{self, AssertUnwindSafe};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, Runtime};

use crate::commands::SharedSession;
use crate::events::emit_all;

/// How often the tick runs without being woken.
pub const TICK: Duration = Duration::from_millis(100);

/// Wakes the tick at once, for example after a command changed the session.
#[derive(Debug, Clone)]
pub struct Waker(Sender<()>);

impl Waker {
    /// A waker and the receiving end the tick thread waits on.
    #[must_use]
    pub fn new() -> (Self, Receiver<()>) {
        let (sender, receiver) = mpsc::channel();
        (Self(sender), receiver)
    }

    /// Runs the tick now. Without a tick thread nothing happens.
    pub fn wake(&self) {
        let _ = self.0.send(());
    }
}

/// Starts the tick thread of `app`.
///
/// The app keeps its [`Waker`] in its managed state, so the thread runs
/// until the process ends; the app state is written at the end by the exit
/// handler in `main.rs`, not by the tick.
pub fn spawn<R: Runtime>(app: AppHandle<R>, wakes: Receiver<()>) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("underline-tick".into())
        .spawn(move || {
            run(&wakes, TICK, || {
                let events = app.state::<SharedSession>().lock().tick(Instant::now());
                emit_all(&app, events);
            });
        })
        .map(drop)
}

/// Calls `tick` every `every`, or at once on a wake, until every sender of
/// `wakes` is gone. Wakes that come while a tick waits are served by that
/// one tick. A panicking tick is logged and the next one runs as usual, so
/// one failure does not stop the clocks.
fn run(wakes: &Receiver<()>, every: Duration, mut tick: impl FnMut()) {
    while let Ok(()) | Err(RecvTimeoutError::Timeout) = wakes.recv_timeout(every) {
        while wakes.try_recv().is_ok() {}
        if panic::catch_unwind(AssertUnwindSafe(&mut tick)).is_err() {
            eprintln!("The backend tick failed; it runs again with the next one");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn adr022_a_wake_reaches_the_receiver() {
        let (waker, wakes) = Waker::new();

        waker.wake();

        assert_eq!(wakes.try_recv(), Ok(()));
    }

    #[test]
    fn adr022_waking_without_a_tick_thread_is_harmless() {
        let (waker, wakes) = Waker::new();
        drop(wakes);

        waker.wake();
    }

    #[test]
    fn adr022_wakes_that_came_together_run_one_tick() {
        let (waker, wakes) = Waker::new();
        waker.wake();
        waker.wake();
        waker.wake();
        drop(waker);
        let ticks = Cell::new(0);

        run(&wakes, Duration::from_secs(60), || {
            ticks.set(ticks.get() + 1)
        });

        assert_eq!(ticks.get(), 1);
    }

    #[test]
    fn adr022_the_tick_runs_on_after_a_panic() {
        let (waker, wakes) = Waker::new();
        let stopper = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            drop(waker);
        });
        let ticks = Cell::new(0);

        run(&wakes, Duration::from_millis(10), || {
            ticks.set(ticks.get() + 1);
            assert!(ticks.get() > 1, "the first tick fails");
        });

        stopper.join().unwrap();
        assert!(ticks.get() >= 2, "ticks: {}", ticks.get());
    }
}
