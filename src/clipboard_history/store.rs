//! SQLite storage of the clipboard history. One writer connection owned by
//! the storage worker; every retention rule takes the time as a parameter so
//! tests can use a fake clock.
use rusqlite::{params, params_from_iter, types::Value, Connection, OptionalExtension};
use std::path::Path;

const SCHEMA_VERSION: i64 = 2;
pub const DAY_MS: i64 = 86_400_000;
/// Sort group of an item: pinned (2), with a comment (1), the rest (0).
const RANK: &str = "(CASE WHEN i.pinned<>0 THEN 2 WHEN trim(i.comment)<>'' THEN 1 ELSE 0 END)";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Policy {
    pub max_items: usize,
    /// 0: keep without a time limit.
    pub retention_days: u32,
    pub max_total_bytes: u64,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            max_items: 100,
            retention_days: 30,
            max_total_bytes: 256 << 20,
        }
    }
}

pub struct FormatRow {
    pub key: String,
    pub derived: bool,
    pub data: Vec<u8>,
}

pub struct NewItem {
    pub captured_ms: i64,
    pub kind: &'static str,
    pub summary: String,
    pub search_text: String,
    pub meta: String,
    pub reduced: bool,
    pub formats: Vec<FormatRow>,
    /// (`table` | `thumb`, bytes)
    pub previews: Vec<(&'static str, Vec<u8>)>,
}

#[derive(Clone, Debug)]
pub struct ItemMeta {
    pub id: i64,
    pub captured_ms: i64,
    pub kind: String,
    pub summary: String,
    pub meta: String,
    pub bytes: i64,
    pub formats: String,
    pub reduced: bool,
    pub has_thumb: bool,
    /// The user's own note on the item.
    pub comment: String,
    /// Pinned: listed first and never removed by the retention rules.
    pub pinned: bool,
}

impl ItemMeta {
    /// Where the item sorts: pinned (2), with a comment (1), the rest (0).
    pub fn rank(&self) -> i64 {
        if self.pinned {
            2
        } else if !self.comment.trim().is_empty() {
            1
        } else {
            0
        }
    }
    /// The paging cursor after this item.
    pub fn cursor(&self) -> (i64, i64, i64) {
        (self.rank(), self.captured_ms, self.id)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Query {
    /// "" for every kind, else text | link | table | image.
    pub kind: String,
    pub search: String,
    /// Continue after this (rank, captured_ms, id): see [`ItemMeta::cursor`].
    pub after: Option<(i64, i64, i64)>,
    pub limit: usize,
}

#[derive(Debug)]
pub enum InsertError {
    TooLarge,
    Db(rusqlite::Error),
}

impl From<rusqlite::Error> for InsertError {
    fn from(e: rusqlite::Error) -> Self {
        InsertError::Db(e)
    }
}

pub struct Store {
    conn: Connection,
}

fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(std::time::Duration::from_secs(3))?;
    // WAL is only meaningful for a file database; ignore the answer.
    let _ = conn.pragma_update(None, "journal_mode", "WAL");
    conn.pragma_update(None, "synchronous", "FULL")?;
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version > SCHEMA_VERSION {
        return Err(rusqlite::Error::InvalidParameterName(
            "newer clipboard database".into(),
        ));
    }
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS items(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            captured_at INTEGER NOT NULL,
            kind TEXT NOT NULL,
            summary TEXT NOT NULL,
            search_text TEXT NOT NULL,
            meta TEXT NOT NULL DEFAULT '',
            total_bytes INTEGER NOT NULL,
            reduced INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS items_order ON items(captured_at DESC, id DESC);
        CREATE TABLE IF NOT EXISTS item_formats(
            item_id INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
            format_key TEXT NOT NULL,
            derived INTEGER NOT NULL DEFAULT 0,
            post_order INTEGER NOT NULL,
            data BLOB NOT NULL,
            PRIMARY KEY(item_id, format_key)
        );
        CREATE TABLE IF NOT EXISTS item_previews(
            item_id INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
            kind TEXT NOT NULL,
            data BLOB NOT NULL,
            PRIMARY KEY(item_id, kind)
        );",
    )?;
    // Version 2: the user's comment and pin on an item.
    let has = |col: &str| -> rusqlite::Result<bool> {
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('items') WHERE name=?1",
            [col],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    };
    if !has("comment")? {
        conn.execute_batch("ALTER TABLE items ADD COLUMN comment TEXT NOT NULL DEFAULT ''")?;
    }
    if !has("pinned")? {
        conn.execute_batch("ALTER TABLE items ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0")?;
    }
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

impl Store {
    /// Opens the database. A damaged file is kept next to the new one under
    /// another name and the history starts empty; the note says so.
    pub fn open(path: &Path) -> Result<(Self, Option<String>), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let try_open = || -> rusqlite::Result<Connection> {
            let conn = Connection::open(path)?;
            init(&conn)?;
            let check: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
            if check != "ok" {
                return Err(rusqlite::Error::InvalidQuery);
            }
            Ok(conn)
        };
        match try_open() {
            Ok(conn) => Ok((Self { conn }, None)),
            Err(first) => {
                let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
                let kept = path.with_extension(format!("corrupt-{stamp}.db"));
                for ext in ["", "-wal", "-shm"] {
                    let mut from = path.as_os_str().to_owned();
                    from.push(ext);
                    let mut to = kept.as_os_str().to_owned();
                    to.push(ext);
                    let _ = std::fs::rename(from, to);
                }
                let conn = Connection::open(path).map_err(|e| format!("{first}; {e}"))?;
                init(&conn).map_err(|e| e.to_string())?;
                Ok((
                    Self { conn },
                    Some(format!(
                        "기록 파일을 열 수 없어 {} 로 보관하고 빈 기록으로 시작했습니다",
                        kept.display()
                    )),
                ))
            }
        }
    }

    #[cfg(test)]
    pub fn memory() -> Self {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        Self { conn }
    }

    /// Stores one item and applies the retention rules in the same
    /// transaction. An item larger than the whole budget is skipped without
    /// deleting anything.
    pub fn insert(
        &mut self,
        item: &NewItem,
        policy: &Policy,
        now_ms: i64,
    ) -> Result<i64, InsertError> {
        let total: u64 = item.formats.iter().map(|f| f.data.len() as u64).sum();
        if total > policy.max_total_bytes {
            return Err(InsertError::TooLarge);
        }
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO items(captured_at,kind,summary,search_text,meta,total_bytes,reduced)
             VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                item.captured_ms,
                item.kind,
                item.summary,
                item.search_text,
                item.meta,
                total as i64,
                item.reduced
            ],
        )?;
        let id = tx.last_insert_rowid();
        for (order, f) in item.formats.iter().enumerate() {
            tx.execute(
                "INSERT INTO item_formats(item_id,format_key,derived,post_order,data) VALUES(?1,?2,?3,?4,?5)",
                params![id, f.key, f.derived, order as i64, f.data],
            )?;
        }
        for (kind, data) in &item.previews {
            tx.execute(
                "INSERT INTO item_previews(item_id,kind,data) VALUES(?1,?2,?3)",
                params![id, kind, data],
            )?;
        }
        retention(&tx, policy, now_ms)?;
        tx.commit()?;
        Ok(id)
    }

    /// Time, count, then capacity. Returns the number of deleted items.
    pub fn apply_retention(&mut self, policy: &Policy, now_ms: i64) -> rusqlite::Result<usize> {
        let tx = self.conn.transaction()?;
        let n = retention(&tx, policy, now_ms)?;
        tx.commit()?;
        Ok(n)
    }

    pub fn list(&self, q: &Query) -> rusqlite::Result<Vec<ItemMeta>> {
        let mut sql = String::from(
            "SELECT i.id,i.captured_at,i.kind,i.summary,i.meta,i.total_bytes,i.reduced,
                (SELECT group_concat(format_key,',') FROM item_formats f WHERE f.item_id=i.id),
                EXISTS(SELECT 1 FROM item_previews p WHERE p.item_id=i.id AND p.kind='thumb'),
                i.comment,i.pinned
             FROM items i WHERE 1=1",
        );
        let mut args: Vec<Value> = vec![];
        if !q.kind.is_empty() {
            sql.push_str(" AND i.kind=?");
            args.push(Value::Text(q.kind.clone()));
        }
        if !q.search.trim().is_empty() {
            sql.push_str(" AND (i.search_text LIKE ? ESCAPE '\\' OR i.comment LIKE ? ESCAPE '\\')");
            let esc = q
                .search
                .trim()
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_");
            args.push(Value::Text(format!("%{esc}%")));
            args.push(Value::Text(format!("%{esc}%")));
        }
        if let Some((rank, ms, id)) = q.after {
            sql.push_str(&format!(
                " AND ({RANK}<? OR ({RANK}=? AND (i.captured_at<? OR (i.captured_at=? AND i.id<?))))"
            ));
            args.push(Value::Integer(rank));
            args.push(Value::Integer(rank));
            args.push(Value::Integer(ms));
            args.push(Value::Integer(ms));
            args.push(Value::Integer(id));
        }
        // Pinned first, then those with a comment, then newest first.
        sql.push_str(&format!(
            " ORDER BY {RANK} DESC, i.captured_at DESC, i.id DESC LIMIT ?"
        ));
        args.push(Value::Integer(q.limit.max(1) as i64));
        let mut st = self.conn.prepare(&sql)?;
        let rows = st.query_map(params_from_iter(args), |r| {
            Ok(ItemMeta {
                id: r.get(0)?,
                captured_ms: r.get(1)?,
                kind: r.get(2)?,
                summary: r.get(3)?,
                meta: r.get(4)?,
                bytes: r.get(5)?,
                reduced: r.get(6)?,
                formats: r.get::<_, Option<String>>(7)?.unwrap_or_default(),
                has_thumb: r.get(8)?,
                comment: r.get(9)?,
                pinned: r.get::<_, i64>(10)? != 0,
            })
        })?;
        rows.collect()
    }

    /// Restore bytes in posting order.
    pub fn formats(&self, id: i64) -> rusqlite::Result<Vec<(String, Vec<u8>)>> {
        let mut st = self.conn.prepare(
            "SELECT format_key,data FROM item_formats WHERE item_id=?1 ORDER BY post_order",
        )?;
        let rows = st.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect()
    }

    pub fn format(&self, id: i64, key: &str) -> rusqlite::Result<Option<Vec<u8>>> {
        self.conn
            .query_row(
                "SELECT data FROM item_formats WHERE item_id=?1 AND format_key=?2",
                params![id, key],
                |r| r.get(0),
            )
            .optional()
    }

    pub fn preview(&self, id: i64, kind: &str) -> rusqlite::Result<Option<Vec<u8>>> {
        self.conn
            .query_row(
                "SELECT data FROM item_previews WHERE item_id=?1 AND kind=?2",
                params![id, kind],
                |r| r.get(0),
            )
            .optional()
    }

    pub fn set_comment(&mut self, id: i64, comment: &str) -> rusqlite::Result<usize> {
        self.conn.execute(
            "UPDATE items SET comment=?2 WHERE id=?1",
            params![id, comment.trim()],
        )
    }
    pub fn set_pinned(&mut self, id: i64, pinned: bool) -> rusqlite::Result<usize> {
        self.conn.execute(
            "UPDATE items SET pinned=?2 WHERE id=?1",
            params![id, pinned as i64],
        )
    }
    pub fn delete(&mut self, id: i64) -> rusqlite::Result<usize> {
        self.conn.execute("DELETE FROM items WHERE id=?1", [id])
    }
    pub fn delete_before(&mut self, ms: i64) -> rusqlite::Result<usize> {
        self.conn
            .execute("DELETE FROM items WHERE captured_at<?1", [ms])
    }
    pub fn delete_all(&mut self) -> rusqlite::Result<usize> {
        let n = self.conn.execute("DELETE FROM items", [])?;
        let _ = self.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        Ok(n)
    }
    pub fn count(&self) -> rusqlite::Result<i64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM items", [], |r| r.get(0))
    }
    pub fn total_bytes(&self) -> rusqlite::Result<i64> {
        self.conn
            .query_row("SELECT COALESCE(SUM(total_bytes),0) FROM items", [], |r| {
                r.get(0)
            })
    }
}

fn retention(tx: &Connection, p: &Policy, now_ms: i64) -> rusqlite::Result<usize> {
    let mut deleted = 0;
    if p.retention_days > 0 {
        let cutoff = now_ms - p.retention_days as i64 * DAY_MS;
        deleted += tx.execute(
            "DELETE FROM items WHERE pinned=0 AND captured_at<?1",
            [cutoff],
        )?;
    }
    // Pinned items stay and do not count toward the number kept.
    deleted += tx.execute(
        "DELETE FROM items WHERE id IN
         (SELECT id FROM items WHERE pinned=0
          ORDER BY captured_at DESC, id DESC LIMIT -1 OFFSET ?1)",
        [p.max_items as i64],
    )?;
    let mut total: i64 =
        tx.query_row("SELECT COALESCE(SUM(total_bytes),0) FROM items", [], |r| {
            r.get(0)
        })?;
    if total as u64 > p.max_total_bytes {
        let mut st = tx.prepare(
            "SELECT id,total_bytes FROM items WHERE pinned=0 ORDER BY captured_at ASC, id ASC",
        )?;
        let rows: Vec<(i64, i64)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        for (id, bytes) in rows {
            if total as u64 <= p.max_total_bytes {
                break;
            }
            deleted += tx.execute("DELETE FROM items WHERE id=?1", [id])?;
            total -= bytes;
        }
    }
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(ms: i64, text: &str, bytes: usize) -> NewItem {
        NewItem {
            captured_ms: ms,
            kind: "text",
            summary: text.into(),
            search_text: text.into(),
            meta: String::new(),
            reduced: false,
            formats: vec![FormatRow {
                key: "text".into(),
                derived: false,
                data: vec![1; bytes],
            }],
            previews: vec![],
        }
    }

    fn ids(s: &Store) -> Vec<i64> {
        s.list(&Query {
            limit: 1000,
            ..Default::default()
        })
        .unwrap()
        .into_iter()
        .map(|m| m.id)
        .collect()
    }

    #[test]
    fn keeps_latest_n_on_insert() {
        let mut s = Store::memory();
        let p = Policy {
            max_items: 100,
            retention_days: 0,
            ..Default::default()
        };
        for i in 0..101 {
            s.insert(&item(1000 + i, "x", 1), &p, 2000).unwrap();
        }
        let got = ids(&s);
        assert_eq!(got.len(), 100);
        assert_eq!(got[0], 101);
        assert!(!got.contains(&1));
    }

    #[test]
    fn expiry_boundary_is_exact_and_ties_use_id() {
        let mut s = Store::memory();
        let p = Policy {
            max_items: 100,
            retention_days: 7,
            ..Default::default()
        };
        let now = 100 * DAY_MS;
        let cutoff = now - 7 * DAY_MS;
        s.insert(
            &item(cutoff - 1, "old", 1),
            &Policy {
                retention_days: 0,
                ..p
            },
            now,
        )
        .unwrap();
        s.insert(
            &item(cutoff, "edge", 1),
            &Policy {
                retention_days: 0,
                ..p
            },
            now,
        )
        .unwrap();
        assert_eq!(s.apply_retention(&p, now).unwrap(), 1);
        assert_eq!(ids(&s), vec![2]);
        // same instant: the smaller id goes first when the count is exceeded
        let tight = Policy {
            max_items: 1,
            retention_days: 0,
            ..p
        };
        s.insert(&item(cutoff, "tie", 1), &tight, now).unwrap();
        assert_eq!(ids(&s), vec![3]);
    }

    #[test]
    fn capacity_deletes_oldest_and_skips_oversized() {
        let mut s = Store::memory();
        let p = Policy {
            max_items: 100,
            retention_days: 0,
            max_total_bytes: 100,
        };
        s.insert(&item(1, "a", 60), &p, 10).unwrap();
        s.insert(&item(2, "b", 60), &p, 10).unwrap();
        assert_eq!(ids(&s), vec![2]);
        assert!(matches!(
            s.insert(&item(3, "c", 101), &p, 10),
            Err(InsertError::TooLarge)
        ));
        assert_eq!(ids(&s), vec![2]);
    }

    #[test]
    fn search_filter_paging_and_escaping() {
        let mut s = Store::memory();
        let p = Policy {
            retention_days: 0,
            ..Default::default()
        };
        s.insert(&item(1, "100% 한글", 1), &p, 10).unwrap();
        s.insert(&item(2, "abc", 1), &p, 10).unwrap();
        let mut link = item(3, "https://a.com", 1);
        link.kind = "link";
        s.insert(&link, &p, 10).unwrap();
        let q = |search: &str, kind: &str| Query {
            search: search.into(),
            kind: kind.into(),
            limit: 10,
            after: None,
        };
        assert_eq!(s.list(&q("%", "")).unwrap().len(), 1);
        assert_eq!(s.list(&q("한글", "")).unwrap().len(), 1);
        assert_eq!(s.list(&q("", "link")).unwrap().len(), 1);
        let first = s
            .list(&Query {
                limit: 2,
                ..Default::default()
            })
            .unwrap();
        let last = first.last().unwrap();
        let rest = s
            .list(&Query {
                limit: 2,
                after: Some(last.cursor()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(rest.len(), 1);
    }

    #[test]
    fn pinned_then_commented_then_newest_and_pins_survive_retention() {
        let mut s = Store::memory();
        let p = Policy {
            max_items: 100,
            retention_days: 0,
            ..Default::default()
        };
        for k in 1..=5 {
            s.insert(&item(k, &format!("t{k}"), 1), &p, 10).unwrap();
        }
        s.set_comment(2, "  회의 메모 ").unwrap();
        s.set_pinned(1, true).unwrap();
        assert_eq!(ids(&s), vec![1, 2, 5, 4, 3]);
        // The comment is stored trimmed and found by search.
        let got = s
            .list(&Query {
                search: "회의".into(),
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].comment, "회의 메모");
        // Paging across the groups keeps the same order.
        let mut seen = vec![];
        let mut after = None;
        loop {
            let page = s
                .list(&Query {
                    limit: 2,
                    after,
                    ..Default::default()
                })
                .unwrap();
            let Some(last) = page.last() else { break };
            after = Some(last.cursor());
            seen.extend(page.iter().map(|m| m.id));
        }
        assert_eq!(seen, vec![1, 2, 5, 4, 3]);
        // Retention: a pin stays however old; the rest follow the rules.
        let tight = Policy {
            max_items: 1,
            retention_days: 0,
            ..p
        };
        s.apply_retention(&tight, 10).unwrap();
        assert_eq!(ids(&s), vec![1, 5]);
        let week = Policy {
            retention_days: 7,
            ..p
        };
        s.apply_retention(&week, 100 * DAY_MS).unwrap();
        assert_eq!(ids(&s), vec![1]);
        // Unpinning puts it back among the rest.
        s.set_pinned(1, false).unwrap();
        assert!(
            !s.list(&Query {
                limit: 9,
                ..Default::default()
            })
            .unwrap()[0]
                .pinned
        );
    }

    #[test]
    fn a_version_1_database_gains_the_new_columns() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE items(id INTEGER PRIMARY KEY AUTOINCREMENT, captured_at INTEGER NOT NULL,
             kind TEXT NOT NULL, summary TEXT NOT NULL, search_text TEXT NOT NULL,
             meta TEXT NOT NULL DEFAULT '', total_bytes INTEGER NOT NULL,
             reduced INTEGER NOT NULL DEFAULT 0);
             INSERT INTO items(captured_at,kind,summary,search_text,total_bytes) VALUES(1,'text','a','a',1);
             PRAGMA user_version=1;",
        )
        .unwrap();
        init(&conn).unwrap();
        let s = Store { conn };
        let got = s
            .list(&Query {
                limit: 9,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(got.len(), 1);
        assert!(got[0].comment.is_empty() && !got[0].pinned);
    }

    #[test]
    fn delete_cascades_to_children() {
        let mut s = Store::memory();
        let p = Policy::default();
        let mut it = item(1, "a", 3);
        it.previews.push(("thumb", vec![1]));
        let id = s.insert(&it, &p, 1).unwrap();
        assert!(s.formats(id).unwrap().len() == 1 && s.preview(id, "thumb").unwrap().is_some());
        s.delete(id).unwrap();
        assert!(s.formats(id).unwrap().is_empty() && s.preview(id, "thumb").unwrap().is_none());
        s.insert(&it, &p, 1).unwrap();
        assert_eq!(s.delete_all().unwrap(), 1);
        assert_eq!(s.count().unwrap(), 0);
    }
}
