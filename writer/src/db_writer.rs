use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufReader, ErrorKind, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::mpsc::Receiver,
    thread::{self, JoinHandle},
};

use sqlx::{PgPool, QueryBuilder};
use util::Trace;

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

    async fn run(self) {
        let mut pos = get_last_checkpoint(&self.wal_dir).unwrap();
        let mut file = open_segment(&self.wal_dir, pos.segment).unwrap();
        file.seek(SeekFrom::Start(pos.offset)).unwrap();
        while let Ok(event) = self.rx.recv() {
            match event {
                Event::Committed(committed) => {
                    if pos == committed {
                        continue;
                    }

                    let limit = if pos.segment == committed.segment {
                        committed.offset
                    } else {
                        segment_len(&self.wal_dir, pos.segment)
                    };

                    let batch = read_records(&mut file, &mut pos, limit);
                    if !batch.is_empty() {
                        db_insert(&self.pool, batch).await;
                        save_checkpoint(&self.wal_dir, &pos);
                    }

                    if pos.segment != committed.segment
                        && pos.offset == segment_len(&self.wal_dir, pos.segment)
                    {
                        pos = WalPos {
                            segment: next_segment(&self.wal_dir, pos.segment),
                            offset: 0,
                        };
                        file = open_segment(&self.wal_dir, pos.segment).unwrap();
                    }
                }
            }
        }
    }
}

pub fn create_db_writer(
    wal_dir: impl Into<PathBuf>,
    db_url: &str,
    rx: Receiver<Event>,
) -> Result<JoinHandle<()>, String> {
    let pool = PgPool::connect_lazy(db_url).map_err(|e| e.to_string())?;
    let db_writer = DbWriter::new(wal_dir, pool, rx);

    let thread = thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            sqlx::migrate!().run(&db_writer.pool).await.unwrap();
            db_writer.run().await;
        });
    });

    Ok(thread)
}

fn get_last_checkpoint(wal_dir: &Path) -> io::Result<WalPos> {
    let buf = match fs::read_to_string(wal_dir.join("checkpoint")) {
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

fn open_segment(wal_dir: &Path, segment: u64) -> io::Result<File> {
    OpenOptions::new()
        .create(false)
        .write(false)
        .read(true)
        .open(wal_dir.join(format!("{segment:020}.seg")))
}

fn segment_len(wal_dir: &Path, segment: u64) -> u64 {
    fs::metadata(wal_dir.join(format!("{segment:020}.seg")))
        .unwrap()
        .len()
}

fn read_records(file: &mut File, pos: &mut WalPos, limit: u64) -> Vec<Trace> {
    // The BufReader below reads ahead and is dropped on return, leaving the
    // file cursor past the last decoded record, so resync it to `pos` each call.
    file.seek(SeekFrom::Start(pos.offset)).unwrap();
    let mut traces = vec![];
    let mut reader = CountingReader {
        inner: BufReader::new(file),
        count: 0,
    };
    let mut scratch = [0u8; util::MAX_TRACE_SIZE];
    while traces.len() < MAX_BATCH && pos.offset < limit {
        let before = reader.count;
        let (next, _) = postcard::from_io((&mut reader, &mut scratch)).unwrap();
        pos.offset += reader.count - before;
        traces.push(next);
    }

    traces
}

async fn db_insert(pool: &PgPool, traces: Vec<Trace>) {
    let mut query_builder = QueryBuilder::new("INSERT INTO traces (timestamp, message)");

    query_builder.push_values(traces, |mut b, trace| {
        b.push_bind(trace.timestamp).push_bind(trace.message);
    });

    let query = query_builder.build();
    query.execute(pool).await.unwrap();
}

fn save_checkpoint(wal_dir: &Path, pos: &WalPos) {
    let temp_path = wal_dir.join("checkpoint.tmp");
    {
        let mut file = File::create(&temp_path).unwrap();
        write!(file, "{}\n{}", pos.segment, pos.offset).unwrap();
        file.sync_all().unwrap();
    }

    fs::rename(&temp_path, wal_dir.join("checkpoint")).unwrap();
    // Windows can't open a directory as a File; NTFS journals the rename itself.
    #[cfg(unix)]
    File::open(wal_dir).unwrap().sync_all().unwrap();
}

fn next_segment(wal_dir: &Path, segment: u64) -> u64 {
    segment + segment_len(wal_dir, segment)
}
