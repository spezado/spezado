// SPDX-License-Identifier: AGPL-3.0-or-later
//! Storage of spezado: the events of every set of books in one SQLite database, the originals of documents as files
//! named by their hash.
//!
//! One database holds all books of a workspace, so that books that belong together – a business and its owner's
//! private books – can be written to in one transaction. The store keeps each event in its canonical encoding, the
//! form it is hashed and passed on in; it knows nothing about accounting.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use librotenado::{BooksId, ContentHash, EventSet, ObjectError};
use rusqlite::{Connection, OptionalExtension, params};

/// Why the store failed.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The database.
    #[error("database: {0}")]
    Database(#[from] rusqlite::Error),
    /// The file system.
    #[error("file system: {0}")]
    Io(#[from] io::Error),
    /// A stored event could not be read – the database was changed from outside.
    #[error("stored event of books {books}: {error}")]
    Event {
        /// The books.
        books: BooksId,
        /// What was wrong.
        error: ObjectError,
    },
    /// A stored identity could not be read – the database was changed from outside.
    #[error("stored identity {0:?} is not valid")]
    Identity(String),
    /// A stored original does not have the hash it is stored under.
    #[error("the original {0} is damaged")]
    Damaged(ContentHash),
}

/// The schema; its version is kept in `user_version`.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS settings (
    books TEXT NOT NULL,          -- '' for settings of the workspace
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (books, key)
) STRICT;
CREATE TABLE IF NOT EXISTS events (
    books TEXT NOT NULL,
    hash TEXT NOT NULL,
    canonical BLOB NOT NULL,
    PRIMARY KEY (books, hash)
) STRICT;
CREATE TABLE IF NOT EXISTS sent (
    books TEXT NOT NULL,
    kind TEXT NOT NULL,           -- 'event' or 'original'
    id TEXT NOT NULL,             -- its hash
    PRIMARY KEY (books, kind, id)
) STRICT;
PRAGMA user_version = 2;
";

/// The store of a workspace: a directory with `spezado.sqlite` and `originals/`.
#[derive(Debug)]
pub struct Store {
    connection: Connection,
    originals: PathBuf,
}

impl Store {
    /// Opens the store in `directory`, creating it if needed.
    pub fn open(directory: &Path) -> Result<Self, StoreError> {
        fs::create_dir_all(directory.join("originals"))?;
        let connection = Connection::open(directory.join("spezado.sqlite"))?;
        connection.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
        connection.execute_batch(SCHEMA)?;
        Ok(Store { connection, originals: directory.join("originals") })
    }

    /// The identities of all books in the store.
    pub fn books(&self) -> Result<Vec<BooksId>, StoreError> {
        let mut statement = self.connection.prepare("SELECT DISTINCT books FROM events ORDER BY books")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|id| {
            let id = id?;
            id.parse().map_err(|_| StoreError::Identity(id))
        })
        .collect()
    }

    /// All events of a set of books.
    pub fn load(&self, books: &BooksId) -> Result<EventSet, StoreError> {
        let mut statement = self.connection.prepare("SELECT canonical FROM events WHERE books = ?1")?;
        let mut rows = statement.query(params![books.as_str()])?;
        let mut set = EventSet::new();
        while let Some(row) = rows.next()? {
            let canonical: Vec<u8> = row.get(0)?;
            set.receive(&canonical).map_err(|error| StoreError::Event { books: books.clone(), error })?;
        }
        Ok(set)
    }

    /// Stores events, each as `(books, hash, canonical encoding)`, all or none. Events stored already are left as
    /// they are; returns how many were new.
    pub fn append(&mut self, events: &[(BooksId, String, Vec<u8>)]) -> Result<usize, StoreError> {
        let transaction = self.connection.transaction()?;
        let mut new = 0;
        {
            let mut insert =
                transaction.prepare("INSERT OR IGNORE INTO events (books, hash, canonical) VALUES (?1, ?2, ?3)")?;
            for (books, hash, canonical) in events {
                new += insert.execute(params![books.as_str(), hash, canonical])?;
            }
        }
        transaction.commit()?;
        Ok(new)
    }

    /// A setting of a set of books, or of the workspace when `books` is none.
    pub fn setting(&self, books: Option<&BooksId>, key: &str) -> Result<Option<String>, StoreError> {
        let books = books.map_or("", BooksId::as_str);
        let value = self
            .connection
            .query_row("SELECT value FROM settings WHERE books = ?1 AND key = ?2", params![books, key], |row| {
                row.get(0)
            })
            .optional()?;
        Ok(value)
    }

    /// Sets a setting of a set of books, or of the workspace when `books` is none.
    pub fn set_setting(&self, books: Option<&BooksId>, key: &str, value: &str) -> Result<(), StoreError> {
        let books = books.map_or("", BooksId::as_str);
        self.connection.execute(
            "INSERT INTO settings (books, key, value) VALUES (?1, ?2, ?3)
             ON CONFLICT (books, key) DO UPDATE SET value = excluded.value",
            params![books, key, value],
        )?;
        Ok(())
    }

    /// Stores the bytes of an original under their hash; returns the hash. Storing them again changes nothing.
    pub fn put_original(&self, bytes: &[u8]) -> Result<ContentHash, StoreError> {
        let hash = ContentHash::of(bytes);
        let path = self.original_path(&hash);
        if !path.exists() {
            // Written next to it and renamed, so that a file under a hash is always complete.
            let partial = path.with_extension("partial");
            fs::write(&partial, bytes)?;
            fs::rename(&partial, &path)?;
        }
        Ok(hash)
    }

    /// The bytes of an original, if stored; checked against their hash.
    pub fn original(&self, hash: &ContentHash) -> Result<Option<Vec<u8>>, StoreError> {
        match fs::read(self.original_path(hash)) {
            Ok(bytes) if ContentHash::of(&bytes) == *hash => Ok(Some(bytes)),
            Ok(_) => Err(StoreError::Damaged(*hash)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// The hashes of what was sent to the relay of a set of books: `kind` is `event` or `original`.
    pub fn sent(&self, books: &BooksId, kind: &str) -> Result<std::collections::BTreeSet<String>, StoreError> {
        let mut statement = self.connection.prepare("SELECT id FROM sent WHERE books = ?1 AND kind = ?2")?;
        let rows = statement.query_map(params![books.as_str(), kind], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Notes that something was sent to the relay.
    pub fn mark_sent(&self, books: &BooksId, kind: &str, id: &str) -> Result<(), StoreError> {
        self.connection.execute(
            "INSERT OR IGNORE INTO sent (books, kind, id) VALUES (?1, ?2, ?3)",
            params![books.as_str(), kind, id],
        )?;
        Ok(())
    }

    fn original_path(&self, hash: &ContentHash) -> PathBuf {
        let name = hash.to_string().replace(':', "-");
        self.originals.join(name)
    }
}

#[cfg(test)]
mod tests {
    use librotenado::{Clock, Event, Payload};

    use super::*;

    fn books(n: u8) -> BooksId {
        format!("0192f1e4-0000-7000-8000-0000000000{n:02x}").parse().unwrap()
    }

    /// A temporary directory, removed when dropped.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("spezado-store-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            Scratch(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn events_are_kept_per_books_and_survive_reopening() {
        let scratch = Scratch::new("events");
        let mut set = EventSet::new();
        let mut clock = Clock::new("laptop".parse().unwrap());
        let (hash, _) = set.insert(Event {
            time: clock.now(1_000),
            seq: 1,
            prev: None,
            payload: Payload::BooksFounded { books: books(1) },
        });
        let row = (books(1), hash.to_string(), set.canonical(&hash).unwrap().to_vec());
        {
            let mut store = Store::open(&scratch.0).unwrap();
            assert_eq!(store.append(std::slice::from_ref(&row)).unwrap(), 1);
            assert_eq!(store.append(&[row]).unwrap(), 0);
            store.set_setting(Some(&books(1)), "chart", "de-skr03").unwrap();
            store.set_setting(Some(&books(1)), "chart", "de-skr04").unwrap();
        }
        let store = Store::open(&scratch.0).unwrap();
        assert_eq!(store.books().unwrap(), vec![books(1)]);
        assert_eq!(store.load(&books(1)).unwrap(), set);
        assert!(store.load(&books(2)).unwrap().is_empty());
        assert_eq!(store.setting(Some(&books(1)), "chart").unwrap().as_deref(), Some("de-skr04"));
        assert_eq!(store.setting(None, "chart").unwrap(), None);
    }

    #[test]
    fn originals_are_stored_by_their_hash() {
        let scratch = Scratch::new("originals");
        let store = Store::open(&scratch.0).unwrap();
        let hash = store.put_original(b"<Invoice/>").unwrap();
        assert_eq!(store.put_original(b"<Invoice/>").unwrap(), hash);
        assert_eq!(store.original(&hash).unwrap().as_deref(), Some(&b"<Invoice/>"[..]));
        assert_eq!(store.original(&ContentHash::of(b"other")).unwrap(), None);
        fs::write(store.original_path(&hash), b"changed").unwrap();
        assert!(matches!(store.original(&hash), Err(StoreError::Damaged(_))));
    }
}
