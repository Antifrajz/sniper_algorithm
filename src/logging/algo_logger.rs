use flexi_logger::{DeferredNow, FileSpec, Logger, WriteMode};
use log::Record;
use std::sync::Once;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

pub struct AlgoLogger {
    algo_id: String,
    log_file: std::fs::File, // Unique file handle for each algorithm
}

// Static initializer for one-time global logger setup
static INIT: Once = Once::new();

impl AlgoLogger {
    /// Initializes the global logger configuration once, setting up the logs directory.
    pub fn init_once() -> Result<(), Box<dyn std::error::Error>> {
        // Clear the logs directory if it exists
        let logs_dir = Path::new("logs");
        if logs_dir.exists() {
            fs::remove_dir_all(logs_dir)?; // Delete all files in the logs directory
        }
        fs::create_dir_all(logs_dir)?; // Recreate the directory

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

    /// Creates a new AlgoLogger for a specific algorithm ID, each with its own file.
    pub fn new(algo_id: &str) -> Self {
        let log_file_path = format!("logs/{}.log", algo_id);

        // Create or open the specific log file for this algorithm
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_path)
            .expect("Unable to open log file");

        Self {
            algo_id: algo_id.to_string(),
            log_file,
        }
    }

    /// Logs a message to this algorithm's log file with the specified level and context.
    fn write_log(&mut self, level: &str, context: &str, message: std::fmt::Arguments) {
        // Format the timestamp and message for this log entry
        let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S");
        writeln!(
            self.log_file,
            "[{}] [{}] [{}] {}",
            timestamp, level, context, message
        )
        .expect("Failed to write log message");
    }

    /// Logs an info-level message with formatted arguments and context.
    pub fn log_info(&mut self, context: &str, args: std::fmt::Arguments) {
        self.write_log("INFO", context, args);
    }

    /// Logs an error-level message with formatted arguments and context.
    pub fn log_error(&mut self, context: &str, args: std::fmt::Arguments) {
        self.write_log("ERROR", context, args);
    }

    /// Logs a debug-level message with formatted arguments and context, only in debug builds.
    #[cfg(debug_assertions)]
    pub fn log_debug(&mut self, context: &str, args: std::fmt::Arguments) {
        self.write_log("DEBUG", context, args);
    }

    /// No-op in release builds, so debug logs are ignored.
    #[cfg(not(debug_assertions))]
    pub fn log_debug(&mut self, _context: &str, _args: std::fmt::Arguments) {
        // Do nothing in release builds
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
