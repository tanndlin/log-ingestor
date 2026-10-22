use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::mpsc::Sender,
};

use util::Trace;

use crate::{event::Event, wal_pos::WalPos};

const MAX_FILE_SIZE: u64 = 64 * 1024 * 1024; // 64 MB

pub struct WalWriter {
    wal_dir: PathBuf,
    cur_file: File,
    pos: WalPos,
    tx: Sender<Event>,
}

impl WalWriter {
    pub(crate) fn new(wal_dir: impl Into<PathBuf>, tx: Sender<Event>) -> Self {
        let wal_dir = wal_dir.into();
        let pos = if fs::exists(&wal_dir).unwrap() {
            get_offset(&wal_dir)
        } else {
            fs::create_dir_all(&wal_dir).unwrap();
            WalPos::new(0, 0)
        };

        let cur_file = Self::open_segment(&wal_dir, pos.segment).unwrap();
        Self {
            wal_dir,
            cur_file,
            pos,
            tx,
        }
    }

    pub fn write(&mut self, trace: &Trace) {
        let bytes = trace.to_vec();
        self.cur_file.write_all(&bytes).unwrap();
        self.pos.offset += bytes.len() as u64;

        let metadata = self.cur_file.metadata().unwrap();
        let file_length = metadata.len();
        if file_length >= MAX_FILE_SIZE {
            self.cur_file =
                Self::open_segment(&self.wal_dir, self.pos.segment + file_length).unwrap();

            self.pos.segment += file_length;
            self.pos.offset = 0;
        }

        self.tx.send(Event::Committed(self.pos.clone())).unwrap();
    }

    fn open_segment(wal_dir: &Path, segment: u64) -> io::Result<File> {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(wal_dir.join(format!("{segment:020}.seg")))
    }
}

fn get_offset(wal_dir: &Path) -> WalPos {
    let segment = fs::read_dir(wal_dir)
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
        .unwrap_or(0);

    let offset = segment_len(wal_dir, segment);
    WalPos::new(segment, offset)
}

fn segment_len(wal_dir: &Path, segment: u64) -> u64 {
    fs::metadata(wal_dir.join(format!("{segment:020}.seg")))
        .unwrap()
        .len()
}
