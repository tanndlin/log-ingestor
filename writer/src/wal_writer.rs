use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::mpsc::Sender,
};

use anyhow::{Context, Result};
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
    pub(crate) fn new(wal_dir: impl Into<PathBuf>, tx: Sender<Event>) -> Result<Self> {
        let wal_dir = wal_dir.into();
        let pos = if fs::exists(&wal_dir).context("checking for WAL dir")? {
            get_offset(&wal_dir)?
        } else {
            fs::create_dir_all(&wal_dir).context("creating WAL dir")?;
            WalPos::new(0, 0)
        };

        let cur_file =
            Self::open_segment(&wal_dir, pos.segment).context("opening current WAL segment")?;
        Ok(Self {
            wal_dir,
            cur_file,
            pos,
            tx,
        })
    }

    pub fn write(&mut self, trace: &Trace) -> Result<()> {
        let bytes = trace.to_vec().context("serializing trace")?;
        // TODO: Data loss
        self.cur_file
            .write_all(&bytes)
            .context("writing trace to WAL")?;
        self.pos.offset += bytes.len() as u64;

        let metadata = self
            .cur_file
            .metadata()
            .context("reading WAL segment metadata")?;
        let file_length = metadata.len();
        if file_length >= MAX_FILE_SIZE {
            self.cur_file = Self::open_segment(&self.wal_dir, self.pos.segment + file_length)
                .context("opening new WAL segment")?;

            self.pos.segment += file_length;
            self.pos.offset = 0;
        }

        self.tx
            .send(Event::Committed(self.pos.clone()))
            .context("sending Committed event to DB writer")?;
        Ok(())
    }

    fn open_segment(wal_dir: &Path, segment: u64) -> io::Result<File> {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(wal_dir.join(format!("{segment:020}.seg")))
    }
}

fn get_offset(wal_dir: &Path) -> Result<WalPos> {
    let mut latest = None;
    for entry in fs::read_dir(wal_dir).context("listing WAL dir")? {
        let name = entry.context("reading WAL dir entry")?.file_name();
        let name = name.to_string_lossy();
        if let Some(stem) = name.strip_suffix(".seg") {
            let segment = stem
                .parse::<u64>()
                .with_context(|| format!("bad WAL segment name: {name}"))?;
            latest = latest.max(Some(segment));
        }
    }

    // WAL dir exists but has no segments yet
    let Some(segment) = latest else {
        return Ok(WalPos::new(0, 0));
    };

    let offset = segment_len(wal_dir, segment)?;
    Ok(WalPos::new(segment, offset))
}

fn segment_len(wal_dir: &Path, segment: u64) -> Result<u64> {
    Ok(fs::metadata(wal_dir.join(format!("{segment:020}.seg")))
        .context("getting WAL segment metadata")?
        .len())
}
