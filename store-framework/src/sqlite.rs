//! SQLite backend. The framework tables live here; application tables come from `Persist`.

use crate::{Backend, InstanceId, InstanceInfo, JournalEntry, JournalRow, Status};
use rusqlite::{params, Connection, OptionalExtension, Transaction};

pub struct Sqlite {
    conn: Connection,
}

const FRAMEWORK_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS flow_instance (
    instance   INTEGER PRIMARY KEY AUTOINCREMENT,
    flow       TEXT NOT NULL,
    status     TEXT NOT NULL,          -- 'waiting' | 'finished'
    seq        INTEGER NOT NULL        -- responses delivered so far
);
CREATE TABLE IF NOT EXISTS flow_journal (
    instance   INTEGER NOT NULL,
    seq        INTEGER NOT NULL,
    at         TEXT NOT NULL,
    event      TEXT NOT NULL,
    response   TEXT NOT NULL,
    PRIMARY KEY (instance, seq)
);
";

impl Sqlite {
    pub fn open(path: &str) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(FRAMEWORK_SCHEMA)?;
        Ok(Sqlite { conn })
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        Self::open(":memory:")
    }

    /// Raw access for application queries (support tooling, reports, tests).
    pub fn connection(&self) -> &Connection {
        &self.conn
    }
}

fn status_str(s: Status) -> &'static str {
    match s {
        Status::Waiting => "waiting",
        Status::Finished => "finished",
    }
}

impl Backend for Sqlite {
    type Tx<'a> = Transaction<'a>;
    type Error = rusqlite::Error;

    fn begin(&mut self) -> rusqlite::Result<Transaction<'_>> {
        self.conn.transaction()
    }

    fn commit(tx: Transaction<'_>) -> rusqlite::Result<()> {
        tx.commit()
    }

    fn execute_ddl(&mut self, ddl: &str) -> rusqlite::Result<()> {
        self.conn.execute_batch(ddl)
    }

    fn insert_instance(tx: &mut Transaction<'_>, flow: &str) -> rusqlite::Result<InstanceId> {
        tx.execute("INSERT INTO flow_instance (flow, status, seq) VALUES (?1, 'waiting', 0)", params![flow])?;
        Ok(tx.last_insert_rowid())
    }

    fn instance(tx: &mut Transaction<'_>, id: InstanceId) -> rusqlite::Result<Option<InstanceInfo>> {
        let row: Option<(i64, String)> = tx
            .query_row("SELECT seq, status FROM flow_instance WHERE instance = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        Ok(row.map(|(seq, st)| InstanceInfo {
            seq,
            status: if st == "finished" { Status::Finished } else { Status::Waiting },
        }))
    }

    fn advance(tx: &mut Transaction<'_>, id: InstanceId, seq: i64, status: Status) -> rusqlite::Result<()> {
        tx.execute("UPDATE flow_instance SET seq = ?2, status = ?3 WHERE instance = ?1", params![id, seq, status_str(status)])?;
        Ok(())
    }

    fn append_journal(tx: &mut Transaction<'_>, id: InstanceId, seq: i64, e: &JournalEntry) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT INTO flow_journal (instance, seq, at, event, response) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, seq, e.at, e.event, e.response],
        )?;
        Ok(())
    }

    fn waiting_instances(tx: &mut Transaction<'_>, flow: &str) -> rusqlite::Result<Vec<InstanceId>> {
        let mut st = tx.prepare("SELECT instance FROM flow_instance WHERE flow = ?1 AND status = 'waiting' ORDER BY instance")?;
        let rows = st.query_map(params![flow], |r| r.get(0))?;
        rows.collect()
    }

    fn journal(tx: &mut Transaction<'_>, id: InstanceId) -> rusqlite::Result<Vec<JournalRow>> {
        let mut st = tx.prepare("SELECT seq, at, event, response FROM flow_journal WHERE instance = ?1 ORDER BY seq")?;
        let rows = st.query_map(params![id], |r| {
            Ok(JournalRow { seq: r.get(0)?, entry: JournalEntry { at: r.get(1)?, event: r.get(2)?, response: r.get(3)? } })
        })?;
        rows.collect()
    }
}
