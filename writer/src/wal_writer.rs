use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use util::Trace;

const MAX_FILE_SIZE: u64 = 64 * 1024 * 1024; // 64 MB

pub struct WalWriter {
    wal_dir: PathBuf,
    cur_file: File,
    offset: u64,
}

impl WalWriter {
    pub fn new(wal_dir: impl Into<PathBuf>) -> Self {
        let wal_dir = wal_dir.into();
        let offset = if fs::exists(&wal_dir).unwrap() {
            get_offset(&wal_dir)
        } else {
            fs::create_dir_all(&wal_dir).unwrap();
            0
        };

        let cur_file = Self::open_segment(&wal_dir, offset).unwrap();
        Self {
            wal_dir,
            cur_file,
            offset,
        }
    }

    pub fn write(&mut self, trace: &Trace) {
        self.cur_file.write_all(&trace.to_vec()).unwrap();

        let metadata = self.cur_file.metadata().unwrap();
        let file_length = metadata.len();
        if file_length >= MAX_FILE_SIZE {
            self.cur_file = Self::open_segment(&self.wal_dir, self.offset + file_length).unwrap();
            self.offset += file_length;
        }
    }

    fn open_segment(wal_dir: &Path, offset: u64) -> io::Result<File> {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(wal_dir.join(format!("{offset:020}.seg")))
    }
}

fn get_offset(wal_dir: &Path) -> u64 {
    fs::read_dir(wal_dir)
        .unwrap()
        .filter_map(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .strip_suffix(".seg")
                .map(|f| f.parse::<u64>().unwrap())
        })
        .max()
        .unwrap_or(0)
}
