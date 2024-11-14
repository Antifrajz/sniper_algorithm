use flexi_logger::{FileSpec, Logger, WriteMode};
use std::sync::Once;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

pub struct AlgoLogger {
    log_file: std::fs::File,
}

// Static initializer for one-time global logger setup
static INIT: Once = Once::new();

impl AlgoLogger {
    pub fn init_once() -> Result<(), Box<dyn std::error::Error>> {
        let logs_dir = Path::new("logs");
        if logs_dir.exists() {
            fs::remove_dir_all(logs_dir)?;
        }
        fs::create_dir_all(logs_dir)?;

        let reports_dir = Path::new("reports");
        if reports_dir.exists() {
            fs::remove_dir_all(reports_dir)?;
        }
        fs::create_dir_all(reports_dir)?;

        INIT.call_once(|| {
            Logger::try_with_str("info")
                .unwrap()
                .log_to_file(FileSpec::default().directory("logs").basename("global"))
                .write_mode(WriteMode::BufferAndFlush)
                .use_utc()
                .start()
                .expect("Failed to initialize global logger");
        });
        Ok(())
    }

    pub fn new(algo_type: &str, algo_id: &str) -> Self {
        let log_file_path = format!("logs/{}_{}_.log", algo_type, algo_id);

        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_path)
            .expect("Unable to open log file");

        Self { log_file }
    }

    fn write_log(&mut self, level: &str, context: &str, message: std::fmt::Arguments) {
        let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S");
        writeln!(
            self.log_file,
            "[{}] [{}] [{}] {}",
            timestamp, level, context, message
        )
        .expect("Failed to write log message");
    }

    pub fn log_info(&mut self, context: &str, args: std::fmt::Arguments) {
        self.write_log("INFO", context, args);
    }

    pub fn log_error(&mut self, context: &str, args: std::fmt::Arguments) {
        self.write_log("ERROR", context, args);
    }

    #[cfg(debug_assertions)]
    pub fn log_debug(&mut self, context: &str, args: std::fmt::Arguments) {
        self.write_log("DEBUG", context, args);
    }

    /// No-op in release builds, so debug logs are ignored.
    #[cfg(not(debug_assertions))]
    pub fn log_debug(&mut self, _context: &str, _args: std::fmt::Arguments) {
        // nop
    }
}

// Macros to simplify logging with formatted arguments and context
#[macro_export]
macro_rules! log_info {
    ($logger:expr, $context:expr, $($arg:tt)*) => {
        $logger.log_info($context, format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_error {
    ($logger:expr, $context:expr, $($arg:tt)*) => {
        $logger.log_error($context, format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_debug {
    ($logger:expr, $context:expr, $($arg:tt)*) => {
        $logger.log_debug($context, format_args!($($arg)*))
    };
}
