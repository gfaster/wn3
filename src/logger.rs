use log::{Level, Metadata, Record};

pub struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Trace
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let module = record.module_path().unwrap_or("unknown");
            if module.starts_with("selectors") || module.starts_with("html5ever") {
                if record.level() > Level::Info {
                    return;
                }
            }
            if record.target() == "progress" {
                eprintln!("{}...", record.args());
            } else {
                let color = match record.level() {
                    Level::Error => "\x1b[31;1m",
                    Level::Warn => "\x1b[33;1m",
                    Level::Info => "\x1b[1m",
                    Level::Debug => "",
                    Level::Trace => "",
                };
                let color_end = "\x1b[0m";
                eprintln!("[{color}{}{color_end}] {}", record.level(), record.args());
            }
        }
    }

    fn flush(&self) {}
}

use log::{LevelFilter, SetLoggerError};

static LOGGER: SimpleLogger = SimpleLogger;

pub fn init() -> Result<(), SetLoggerError> {
    log::set_logger(&LOGGER).map(|()| log::set_max_level(LevelFilter::Info))
}
