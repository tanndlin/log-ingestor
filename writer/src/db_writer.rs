use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufReader, ErrorKind, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::mpsc::Receiver,
    thread::{self, JoinHandle},
};

use anyhow::{Context, Result};
use sqlx::PgPool;
use util::{Level, Trace};

use crate::{counting_reader::CountingReader, event::Event, wal_pos::WalPos};

const MAX_BATCH: usize = 10_000;

pub struct DbWriter {
    wal_dir: PathBuf,
    pool: PgPool,
    rx: Receiver<Event>,
}

impl DbWriter {
    pub(crate) fn new(wal_dir: impl Into<PathBuf>, pool: PgPool, rx: Receiver<Event>) -> Self {
        Self {
            wal_dir: wal_dir.into(),
            pool,
            rx,
        }
    }

    async fn run(self) -> Result<()> {
        let mut pos = self.get_last_checkpoint()?;
        let mut file = self.open_segment(pos.segment)?;
        file.seek(SeekFrom::Start(pos.offset))?;
        while let Ok(event) = self.rx.recv() {
            match event {
                Event::Committed(committed) => {
                    if pos == committed {
                        continue;
                    }

                    let limit = if pos.segment == committed.segment {
                        committed.offset
                    } else {
                        self.segment_len(pos.segment)?
                    };

                    let batch = read_records(&mut file, &mut pos, limit)?;
                    if !batch.is_empty() {
                        self.db_insert(&self.pool, batch).await?;
                        self.save_checkpoint(&pos)?;
                    }

                    if pos.segment != committed.segment
                        && pos.offset == self.segment_len(pos.segment)?
                    {
                        pos = WalPos {
                            segment: self.next_segment(pos.segment)?,
                            offset: 0,
                        };
                        file = self.open_segment(pos.segment)?;
                    }
                }
            }
        }

        Ok(())
    }

    fn get_last_checkpoint(&self) -> io::Result<WalPos> {
        let buf = match fs::read_to_string(self.wal_dir.join("checkpoint")) {
            Ok(buf) => buf,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(WalPos::new(0, 0)),
            Err(e) => return Err(e),
        };

        let invalid = || io::Error::new(ErrorKind::InvalidData, format!("bad checkpoint: {buf:?}"));
        let (segment, offset) = buf.trim().split_once('\n').ok_or_else(invalid)?;
        Ok(WalPos::new(
            segment.parse().map_err(|_| invalid())?,
            offset.parse().map_err(|_| invalid())?,
        ))
    }

    fn open_segment(&self, segment: u64) -> io::Result<File> {
        OpenOptions::new()
            .create(false)
            .write(false)
            .read(true)
            .open(self.wal_dir.join(format!("{segment:020}.seg")))
    }

    fn segment_len(&self, segment: u64) -> Result<u64> {
        Ok(
            fs::metadata(self.wal_dir.join(format!("{segment:020}.seg")))
                .context("getting existing segment metadata")?
                .len(),
        )
    }

    async fn db_insert(&self, pool: &PgPool, traces: Vec<Trace>) -> Result<()> {
        let mut levels = Vec::with_capacity(traces.len());
        let mut timestamps = Vec::with_capacity(traces.len());
        let mut messages = Vec::with_capacity(traces.len());
        let mut applications = Vec::with_capacity(traces.len());
        let mut span_ids = Vec::with_capacity(traces.len());
        for t in traces {
            levels.push(t.level);
            timestamps.push(t.timestamp);
            messages.push(t.message);
            applications.push(t.application);
            span_ids.push(t.span_id);
        }

        sqlx::query!(
            "INSERT INTO traces (level, timestamp, message, application, span_id) SELECT * FROM UNNEST($1::level[], $2::timestamptz[], $3::text[], $4::text[], $5::bigint[])",
            &levels as &[Level],
            &timestamps,
            &messages,
            &applications,
            &span_ids as &[Option<i64>],
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    fn save_checkpoint(&self, pos: &WalPos) -> Result<()> {
        let temp_path = self.wal_dir.join("checkpoint.tmp");
        {
            let mut file = File::create(&temp_path).context("creating checkpoint.tmp")?;
            write!(file, "{}\n{}", pos.segment, pos.offset)?;
            file.sync_all().context("fsync")?;
        }

        fs::rename(&temp_path, self.wal_dir.join("checkpoint"))
            .context("renaming checkpoint.temp")?;
        // Windows can't open a directory as a File; NTFS journals the rename itself.
        #[cfg(unix)]
        File::open(&self.wal_dir)?.sync_all()?;
        Ok(())
    }

    fn next_segment(&self, segment: u64) -> Result<u64> {
        Ok(segment + self.segment_len(segment)?)
    }
}

pub fn create_db_writer(
    wal_dir: impl Into<PathBuf>,
    db_url: &str,
    rx: Receiver<Event>,
) -> Result<JoinHandle<()>> {
    let pool = PgPool::connect_lazy(db_url).context("connecting to postgres")?;
    let db_writer = DbWriter::new(wal_dir, pool, rx);

    let thread = thread::spawn(move || {
        if let Err(e) = run_on_thread(db_writer) {
            eprintln!("DB writer stopped: {e:#}");
        }
    });

    Ok(thread)
}

fn run_on_thread(db_writer: DbWriter) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("building DB writer runtime")?;
    rt.block_on(async {
        sqlx::migrate!()
            .run(&db_writer.pool)
            .await
            .context("running migrations")?;
        db_writer.run().await
    })
}

fn read_records(file: &mut File, pos: &mut WalPos, limit: u64) -> Result<Vec<Trace>> {
    // The BufReader below reads ahead and is dropped on return, leaving the
    // file cursor past the last decoded record, so resync it to `pos` each call.
    file.seek(SeekFrom::Start(pos.offset))
        .context("seeking in file to read records")?;
    let mut traces = vec![];
    let mut reader = CountingReader {
        inner: BufReader::new(file),
        count: 0,
    };
    let mut scratch = [0u8; util::MAX_TRACE_SIZE];
    while traces.len() < MAX_BATCH && pos.offset < limit {
        let before = reader.count;
        let (next, _) =
            postcard::from_io((&mut reader, &mut scratch)).context("reading records from file")?;
        pos.offset += reader.count - before;
        traces.push(next);
    }

    Ok(traces)
}
