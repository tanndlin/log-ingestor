use std::{
    cell::RefCell,
    collections::VecDeque,
    fs::{self, File, OpenOptions},
    future::Future,
    io::{self, BufReader, ErrorKind, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::mpsc::{Receiver, TryRecvError},
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::{Context, Result};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::time::Instant;
use util::{Level, Trace};

use crate::{counting_reader::CountingReader, event::Event, wal_pos::WalPos};

const MAX_BATCH: usize = 10_000;
/// How often a retry backoff checks whether the WAL writer has shut down.
const SHUTDOWN_POLL: Duration = Duration::from_millis(100);

pub struct DbWriter {
    wal_dir: PathBuf,
    pool: PgPool,
    rx: Receiver<Event>,
    /// Events taken off `rx` while checking for shutdown, handled before reading `rx` again.
    pending: RefCell<VecDeque<Event>>,
}

impl DbWriter {
    pub(crate) fn new(wal_dir: impl Into<PathBuf>, pool: PgPool, rx: Receiver<Event>) -> Self {
        Self {
            wal_dir: wal_dir.into(),
            pool,
            rx,
            pending: RefCell::default(),
        }
    }

    async fn run(&self) -> Result<()> {
        // Wait out a DB that isn't up yet before migrating.
        with_retry(
            "connecting to postgres",
            || self.is_shut_down(),
            || self.pool.acquire(),
        )
        .await?;
        sqlx::migrate!()
            .run(&self.pool)
            .await
            .context("running migrations")?;
        let mut pos = self.get_last_checkpoint()?;
        let mut file = self.open_segment(pos.segment)?;
        file.seek(SeekFrom::Start(pos.offset))?;
        while let Some(event) = self.next_event() {
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
                        self.db_insert(batch).await?;
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

    fn next_event(&self) -> Option<Event> {
        let pending = self.pending.borrow_mut().pop_front();
        pending.or_else(|| self.rx.recv().ok())
    }

    /// True once the WAL writer has dropped its sender. Queued events are moved
    /// to `pending` rather than dropped, since `rx` can't be peeked.
    fn is_shut_down(&self) -> bool {
        loop {
            match self.rx.try_recv() {
                Ok(event) => self.pending.borrow_mut().push_back(event),
                Err(TryRecvError::Empty) => return false,
                Err(TryRecvError::Disconnected) => return true,
            }
        }
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

    async fn db_insert(&self, traces: Vec<Trace>) -> Result<()> {
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

        // The batch stays in memory, so a transient failure retries the same rows
        // instead of restarting from the checkpoint.
        // Giving up on shutdown is safe: the checkpoint isn't advanced, so the
        // batch is replayed on the next start.
        with_retry("inserting batch", || self.is_shut_down(), || {
            sqlx::query!(
                "INSERT INTO traces (level, timestamp, message, application, span_id) SELECT * FROM UNNEST($1::level[], $2::timestamptz[], $3::text[], $4::text[], $5::bigint[])",
                &levels as &[Level],
                &timestamps,
                &messages,
                &applications,
                &span_ids as &[Option<i64>],
            )
            .execute(&self.pool)
        })
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
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(3))
        .connect_lazy(db_url)
        .context("parsing postgres url")?;
    let db_writer = DbWriter::new(wal_dir, pool, rx);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("building DB writer runtime")?;

    let thread = thread::spawn(move || {
        if let Err(e) = rt.block_on(db_writer.run()) {
            eprintln!("DB writer stopped: {e:#}");
        }
    });

    Ok(thread)
}

/// Retries `f` with capped exponential backoff while it fails with a transient
/// error. Permanent errors (bad data, constraint violations) return immediately,
/// and so does the last transient error once `is_shut_down` reports true.
async fn with_retry<T, F, Fut>(what: &str, is_shut_down: impl Fn() -> bool, mut f: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, sqlx::Error>>,
{
    let mut delay = Duration::from_secs(1);
    loop {
        match f().await {
            Err(e) if is_transient(&e) => {
                eprintln!("{what} failed: {e}. Retrying in {delay:?}...");
                let deadline = Instant::now() + delay;
                loop {
                    if is_shut_down() {
                        return Err(e).context(format!("{what}: shut down while retrying"));
                    }
                    let now = Instant::now();
                    if now >= deadline {
                        break;
                    }
                    tokio::time::sleep((deadline - now).min(SHUTDOWN_POLL)).await;
                }
                delay = (delay * 2).min(Duration::from_secs(10));
            }
            result => return result.with_context(|| what.to_owned()),
        }
    }
}

fn is_transient(e: &sqlx::Error) -> bool {
    match e {
        sqlx::Error::Io(_) | sqlx::Error::Tls(_) | sqlx::Error::PoolTimedOut => true,
        // 08: connection exception, 53: insufficient resources, 57P: operator intervention (e.g. shutdown)
        sqlx::Error::Database(db) => db
            .code()
            .is_some_and(|c| c.starts_with("08") || c.starts_with("53") || c.starts_with("57P")),
        _ => false,
    }
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
