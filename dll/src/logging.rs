use std::{fs::File, io::Write, path::PathBuf, sync::Mutex};

use log::Log;

pub struct Logger {
    file: Mutex<File>,
}

impl Log for Logger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata())
            && let Ok(mut file) = self.file.lock()
        {
            _ = writeln!(file, "[{}] {}", record.level(), record.args());
        }
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            _ = file.flush();
        }
    }
}

impl Logger {
    pub fn new(path: PathBuf) -> std::io::Result<Self> {
        Ok(Self {
            file: Mutex::new(File::create(path)?),
        })
    }
}
