#[allow(dead_code)]
pub trait Implies {
    fn implies(self, other: bool) -> bool;
    fn implies_then<F: FnOnce() -> bool>(self, other: F) -> bool;
}

impl Implies for bool {
    fn implies(self, other: bool) -> bool {
        !self || other
    }

    fn implies_then<F: FnOnce() -> bool>(self, other: F) -> bool {
        if !self {
            return true;
        }
        other()
    }
}

#[cfg(test)]
#[allow(unused_imports)]
pub use test_log::*;

#[cfg(test)]
#[allow(dead_code)]
mod test_log {
    use log::LevelFilter;
    use log::{Level, Metadata, Record};
    use std::cell::Cell;
    use std::sync::Once;

    struct TestLogger;

    thread_local! {
        static THREAD_LEVEL: Cell<LevelFilter> = const { Cell::new(LevelFilter::Off) };
    }

    impl log::Log for TestLogger {
        fn enabled(&self, metadata: &Metadata) -> bool {
            metadata.level() <= THREAD_LEVEL.get()
        }

        fn log(&self, record: &Record) {
            if self.enabled(record.metadata()) {
                let module = record.module_path().unwrap_or("");
                let is_selectors = module.starts_with("selectors") && false;
                let is_html5ever = module.starts_with("html5ever") && false;
                if (is_selectors || is_html5ever) && record.level() > Level::Info {
                    return;
                }
                eprintln!("[{}] {}", record.level(), record.args());
            }
        }

        fn flush(&self) {}
    }

    static LOGGER: TestLogger = TestLogger;
    static LOGGER_INIT: Once = Once::new();

    #[must_use = "logger is turned off when dropped"]
    pub fn test_log_level(level: LevelFilter) -> TestLoggerGuard {
        LOGGER_INIT.call_once(|| {
            log::set_logger(&LOGGER)
                .map(|()| log::set_max_level(LevelFilter::Trace))
                .unwrap()
        });
        THREAD_LEVEL.set(level);
        TestLoggerGuard(())
    }

    /// initializaed log with `LevelFilter::Info`
    #[must_use = "logger is turned off when dropped"]
    pub fn test_log() -> TestLoggerGuard {
        test_log_level(LevelFilter::Info)
    }

    #[clippy::has_significant_drop]
    pub struct TestLoggerGuard(());

    impl Drop for TestLoggerGuard {
        fn drop(&mut self) {
            THREAD_LEVEL.set(LevelFilter::Off)
        }
    }
}
