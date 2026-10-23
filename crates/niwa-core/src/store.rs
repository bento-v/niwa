//! The SQLite store: sessions, slips, plants, block lists and settings.
//!
//! Totals, unlocks and the streak are calculated from sessions, never stored.
//! Times are Unix seconds.

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use crate::config::{BlockListConfig, Mode};
use crate::schedule::Schedule;
use crate::species::Species;

pub type Result<T> = rusqlite::Result<T>;

const SCHEMA: &str = "
CREATE TABLE session (
    id              INTEGER PRIMARY KEY,
    started_at      INTEGER NOT NULL,
    ended_at        INTEGER,
    planned_minutes INTEGER NOT NULL,
    mode            TEXT NOT NULL,
    seed            TEXT NOT NULL,
    -- NULL while the session is running.
    outcome         TEXT,
    slip_count      INTEGER NOT NULL DEFAULT 0,
    block_list      TEXT NOT NULL,
    focused_seconds INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE slip (
    id         INTEGER PRIMARY KEY,
    session_id INTEGER NOT NULL REFERENCES session(id),
    at         INTEGER NOT NULL,
    app_name   TEXT NOT NULL
);
CREATE TABLE plant (
    id         INTEGER PRIMARY KEY,
    session_id INTEGER NOT NULL REFERENCES session(id),
    species    TEXT NOT NULL,
    created    INTEGER NOT NULL
);
CREATE TABLE block_list (
    id     INTEGER PRIMARY KEY,
    name   TEXT NOT NULL,
    config TEXT NOT NULL
);
CREATE TABLE setting (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

/// Version 2 (Oct 17, 2026): schedules.
const SCHEMA_2: &str = "
CREATE TABLE schedule (
    id            INTEGER PRIMARY KEY,
    -- One bit per day, Monday first.
    days          INTEGER NOT NULL,
    -- Minutes after midnight.
    start_minute  INTEGER NOT NULL,
    end_minute    INTEGER NOT NULL,
    enabled       INTEGER NOT NULL DEFAULT 1,
    -- The end of the last window started or skipped, so none runs twice.
    handled_until INTEGER NOT NULL DEFAULT 0
);
";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Grown,
    Withered,
}

impl Outcome {
    pub fn id(self) -> &'static str {
        match self {
            Outcome::Grown => "grown",
            Outcome::Withered => "withered",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSession<'a> {
    pub started_at: i64,
    pub planned_minutes: u32,
    pub mode: Mode,
    pub species: Species,
    pub block_list: &'a str,
}

/// A session that hasn't been given an outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unfinished {
    pub id: i64,
    pub mode: Mode,
    pub species: Species,
    pub planned_minutes: u32,
    pub block_list: String,
    pub slip_count: u32,
}

/// A plant in the garden, with the session that grew it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GardenPlant {
    pub id: i64,
    pub species: Species,
    pub outcome: Outcome,
    pub planned_minutes: u32,
    pub focused_seconds: u32,
    pub created: i64,
    pub mode: Mode,
    pub slip_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredBlockList {
    pub id: i64,
    pub name: String,
    pub config: BlockListConfig,
}

pub struct Store {
    db: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Store> {
        Store::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Store> {
        Store::init(Connection::open_in_memory()?)
    }

    fn init(db: Connection) -> Result<Store> {
        db.pragma_update(None, "foreign_keys", true)?;
        let version: u32 = db.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version == 0 {
            db.execute_batch(&format!("BEGIN; {SCHEMA} PRAGMA user_version = 1; COMMIT;"))?;
        }
        if version <= 1 {
            db.execute_batch(&format!("BEGIN; {SCHEMA_2} PRAGMA user_version = 2; COMMIT;"))?;
        }
        Ok(Store { db })
    }

    /// Every schedule, earliest start first.
    pub fn schedules(&self) -> Result<Vec<Schedule>> {
        let mut statement = self
            .db
            .prepare("SELECT id, days, start_minute, end_minute, enabled FROM schedule ORDER BY start_minute, id")?;
        let rows = statement.query_map([], |row| {
            Ok(Schedule {
                id: row.get(0)?,
                days: row.get(1)?,
                start: row.get(2)?,
                end: row.get(3)?,
                enabled: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    /// Adds a schedule (id 0) or changes one. Returns its id. A changed
    /// schedule starts over: none of its windows count as handled.
    pub fn save_schedule(&self, schedule: &Schedule) -> Result<i64> {
        let values = params![schedule.days, schedule.start, schedule.end, schedule.enabled];
        if schedule.id == 0 {
            self.db.execute(
                "INSERT INTO schedule (days, start_minute, end_minute, enabled) VALUES (?1, ?2, ?3, ?4)",
                values,
            )?;
            return Ok(self.db.last_insert_rowid());
        }
        self.db.execute(
            "UPDATE schedule SET days = ?1, start_minute = ?2, end_minute = ?3, enabled = ?4, handled_until = 0
             WHERE id = ?5",
            params![schedule.days, schedule.start, schedule.end, schedule.enabled, schedule.id],
        )?;
        Ok(schedule.id)
    }

    pub fn delete_schedule(&self, id: i64) -> Result<()> {
        self.db.execute("DELETE FROM schedule WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// The end of the last window of this schedule that has been dealt with
    /// (started, or skipped), as Unix seconds.
    pub fn schedule_handled_until(&self, id: i64) -> Result<i64> {
        self.db
            .query_row("SELECT handled_until FROM schedule WHERE id = ?1", params![id], |row| row.get(0))
            .optional()
            .map(Option::unwrap_or_default)
    }

    pub fn set_schedule_handled_until(&self, id: i64, until: i64) -> Result<()> {
        self.db
            .execute("UPDATE schedule SET handled_until = ?1 WHERE id = ?2", params![until, id])?;
        Ok(())
    }

    /// The seed of the most recent session, if any.
    pub fn last_seed(&self) -> Result<Option<Species>> {
        let seed: Option<String> = self
            .db
            .query_row("SELECT seed FROM session ORDER BY started_at DESC, id DESC LIMIT 1", [], |row| row.get(0))
            .optional()?;
        Ok(seed.as_deref().and_then(Species::from_id))
    }

    pub fn start_session(&self, session: &NewSession<'_>) -> Result<i64> {
        self.db.execute(
            "INSERT INTO session (started_at, planned_minutes, mode, seed, block_list)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                session.started_at,
                session.planned_minutes,
                session.mode.id(),
                session.species.id(),
                session.block_list,
            ],
        )?;
        Ok(self.db.last_insert_rowid())
    }

    /// Removes a session that was recorded but then couldn't start. Sessions
    /// that have an outcome are history and are left alone.
    pub fn discard_session(&self, session_id: i64) -> Result<()> {
        self.db.execute(
            "DELETE FROM session WHERE id = ?1 AND outcome IS NULL",
            [session_id],
        )?;
        Ok(())
    }

    pub fn record_slip(&self, session_id: i64, at: i64, app_name: &str) -> Result<()> {
        self.db.execute(
            "INSERT INTO slip (session_id, at, app_name) VALUES (?1, ?2, ?3)",
            params![session_id, at, app_name],
        )?;
        self.db.execute(
            "UPDATE session SET slip_count = slip_count + 1 WHERE id = ?1",
            [session_id],
        )?;
        Ok(())
    }

    /// Ends a running session and puts its plant, grown or withered, in the
    /// garden. Does nothing if the session has already ended.
    pub fn finish_session(
        &mut self,
        session_id: i64,
        ended_at: i64,
        outcome: Outcome,
        focused_seconds: u32,
    ) -> Result<()> {
        let tx = self.db.transaction()?;
        let ended = tx.execute(
            "UPDATE session SET ended_at = ?2, outcome = ?3, focused_seconds = ?4
             WHERE id = ?1 AND outcome IS NULL",
            params![session_id, ended_at, outcome.id(), focused_seconds],
        )?;
        if ended == 1 {
            tx.execute(
                "INSERT INTO plant (session_id, species, created)
                 SELECT id, seed, ?2 FROM session WHERE id = ?1",
                params![session_id, ended_at],
            )?;
        }
        tx.commit()
    }

    /// Sessions with no outcome yet: left running by a crash or a restart, or
    /// a Strict session the service has carried on with.
    pub fn unfinished(&self) -> Result<Vec<Unfinished>> {
        self.db
            .prepare(
                "SELECT id, mode, seed, planned_minutes, block_list, slip_count
                 FROM session WHERE outcome IS NULL ORDER BY id",
            )?
            .query_map([], |row| {
                let mode: String = row.get(1)?;
                let species: String = row.get(2)?;
                Ok(Unfinished {
                    id: row.get(0)?,
                    mode: Mode::from_id(&mode).unwrap_or_default(),
                    species: Species::from_id(&species).unwrap_or(Species::Sprout),
                    planned_minutes: row.get(3)?,
                    block_list: row.get(4)?,
                    slip_count: row.get(5)?,
                })
            })?
            .collect()
    }

    /// Withers every unfinished session. In Gentle mode a session doesn't
    /// survive the app closing.
    pub fn wither_unfinished(&mut self, now: i64) -> Result<usize> {
        let unfinished: Vec<i64> = self
            .db
            .prepare("SELECT id FROM session WHERE outcome IS NULL")?
            .query_map([], |row| row.get(0))?
            .collect::<Result<_>>()?;
        for &id in &unfinished {
            self.finish_session(id, now, Outcome::Withered, 0)?;
        }
        Ok(unfinished.len())
    }

    /// Every plant, newest first.
    pub fn garden(&self) -> Result<Vec<GardenPlant>> {
        self.db
            .prepare(
                "SELECT plant.id, plant.species, session.outcome, session.planned_minutes,
                        session.focused_seconds, plant.created, session.mode, session.slip_count
                 FROM plant JOIN session ON session.id = plant.session_id
                 ORDER BY plant.created DESC, plant.id DESC",
            )?
            .query_map([], |row| {
                let species: String = row.get(1)?;
                let outcome: String = row.get(2)?;
                Ok(GardenPlant {
                    id: row.get(0)?,
                    species: Species::from_id(&species).unwrap_or(Species::Sprout),
                    outcome: match outcome.as_str() {
                        "grown" => Outcome::Grown,
                        _ => Outcome::Withered,
                    },
                    planned_minutes: row.get(3)?,
                    focused_seconds: row.get(4)?,
                    created: row.get(5)?,
                    mode: Mode::from_id(&row.get::<_, String>(6)?).unwrap_or_default(),
                    slip_count: row.get(7)?,
                })
            })?
            .collect()
    }

    /// Total minutes from finished sessions. Withered sessions don't count.
    pub fn focused_minutes(&self) -> Result<u32> {
        self.focused_minutes_since(i64::MIN)
    }

    /// Minutes from sessions that finished at or after `since`.
    pub fn focused_minutes_since(&self, since: i64) -> Result<u32> {
        self.db.query_row(
            "SELECT COALESCE(SUM(planned_minutes), 0) FROM session
             WHERE outcome = 'grown' AND ended_at >= ?1",
            [since],
            |row| row.get(0),
        )
    }

    /// Every minute Niwa has spent blocking, across all finished sessions,
    /// counted as the Stats screen counts a day: a grown session in full, a
    /// withered one for the whole minutes it ran, never past its plan.
    pub fn blocked_minutes(&self) -> Result<u32> {
        self.db.query_row(
            "SELECT COALESCE(SUM(CASE outcome
                 WHEN 'grown' THEN planned_minutes
                 ELSE MIN(focused_seconds / 60, planned_minutes)
             END), 0) FROM session WHERE outcome IS NOT NULL",
            [],
            |row| row.get(0),
        )
    }

    /// Sessions that ended at or after `since`: when, how, how long they were
    /// planned for, and how many seconds they actually ran.
    pub fn ended_since(&self, since: i64) -> Result<Vec<(i64, Outcome, u32, u32)>> {
        self.db
            .prepare(
                "SELECT ended_at, outcome, planned_minutes, focused_seconds FROM session
                 WHERE outcome IS NOT NULL AND ended_at >= ?1 ORDER BY ended_at",
            )?
            .query_map([since], |row| {
                let outcome: String = row.get(1)?;
                Ok((
                    row.get(0)?,
                    match outcome.as_str() {
                        "grown" => Outcome::Grown,
                        _ => Outcome::Withered,
                    },
                    row.get(2)?,
                    row.get(3)?,
                ))
            })?
            .collect()
    }

    /// Slips at or after `since`: when, and which app.
    pub fn slips_since(&self, since: i64) -> Result<Vec<(i64, String)>> {
        self.db
            .prepare("SELECT at, app_name FROM slip WHERE at >= ?1 ORDER BY at")?
            .query_map([since], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect()
    }

    /// When each finished session ended, for working out the streak.
    pub fn finished_times(&self) -> Result<Vec<i64>> {
        self.db
            .prepare("SELECT ended_at FROM session WHERE outcome = 'grown'")?
            .query_map([], |row| row.get(0))?
            .collect()
    }

    pub fn block_lists(&self) -> Result<Vec<StoredBlockList>> {
        self.db
            .prepare("SELECT id, name, config FROM block_list ORDER BY id")?
            .query_map([], |row| {
                let config: String = row.get(2)?;
                Ok(StoredBlockList {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    // A list written by a newer version still loads, minus
                    // whatever this version doesn't understand.
                    config: serde_json::from_str(&config).unwrap_or_default(),
                })
            })?
            .collect()
    }

    pub fn add_block_list(&self, name: &str, config: &BlockListConfig) -> Result<i64> {
        self.db.execute(
            "INSERT INTO block_list (name, config) VALUES (?1, ?2)",
            params![name, to_json(config)],
        )?;
        Ok(self.db.last_insert_rowid())
    }

    pub fn update_block_list(&self, id: i64, config: &BlockListConfig) -> Result<()> {
        self.db.execute(
            "UPDATE block_list SET config = ?2 WHERE id = ?1",
            params![id, to_json(config)],
        )?;
        Ok(())
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        self.db
            .query_row("SELECT value FROM setting WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.db.execute(
            "INSERT INTO setting (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}

fn to_json(config: &BlockListConfig) -> String {
    serde_json::to_string(config).expect("block list config is plain data")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(species: Species, planned_minutes: u32, started_at: i64) -> NewSession<'static> {
        NewSession {
            started_at,
            planned_minutes,
            mode: Mode::Gentle,
            species,
            block_list: "Deep work",
        }
    }

    fn grow(store: &mut Store, species: Species, minutes: u32, start: i64) -> i64 {
        let id = store.start_session(&session(species, minutes, start)).unwrap();
        let end = start + i64::from(minutes) * 60;
        store
            .finish_session(id, end, Outcome::Grown, minutes * 60)
            .unwrap();
        id
    }

    #[test]
    fn schedules_are_saved_changed_and_removed() {
        let store = Store::open_in_memory().unwrap();
        let mornings = Schedule { id: 0, days: 0b001_1111, start: 9 * 60, end: 12 * 60, enabled: true };
        let id = store.save_schedule(&mornings).unwrap();
        assert_eq!(store.schedules().unwrap(), vec![Schedule { id, ..mornings }]);

        store.set_schedule_handled_until(id, 1_000).unwrap();
        assert_eq!(store.schedule_handled_until(id).unwrap(), 1_000);
        // Changing a schedule forgets which of its windows were dealt with.
        store.save_schedule(&Schedule { id, end: 13 * 60, ..mornings }).unwrap();
        assert_eq!(store.schedule_handled_until(id).unwrap(), 0);
        assert_eq!(store.schedules().unwrap()[0].end, 13 * 60);

        store.delete_schedule(id).unwrap();
        assert!(store.schedules().unwrap().is_empty());
        assert_eq!(store.schedule_handled_until(id).unwrap(), 0);
    }

    #[test]
    fn a_version_1_database_gains_schedules() {
        let path = std::env::temp_dir().join(format!("niwa-v1-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let db = Connection::open(&path).unwrap();
            db.execute_batch(&format!("{SCHEMA} PRAGMA user_version = 1;")).unwrap();
        }
        let store = Store::open(&path).unwrap();
        assert!(store.schedules().unwrap().is_empty());
        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_last_seed_is_the_latest_sessions() {
        let mut store = Store::open_in_memory().unwrap();
        assert_eq!(store.last_seed().unwrap(), None);
        grow(&mut store, Species::Pine, 50, 1_000);
        grow(&mut store, Species::Sprout, 25, 9_000);
        assert_eq!(store.last_seed().unwrap(), Some(Species::Sprout));
    }

    #[test]
    fn blocked_minutes_add_up_every_finished_session() {
        let mut store = Store::open_in_memory().unwrap();
        assert_eq!(store.blocked_minutes().unwrap(), 0);
        grow(&mut store, Species::Pine, 50, 1_000);
        // Withered after 12 minutes 59 seconds: 12 whole minutes.
        let withered = store.start_session(&session(Species::Sprout, 25, 10_000)).unwrap();
        store.finish_session(withered, 10_779, Outcome::Withered, 12 * 60 + 59).unwrap();
        // A Strict session's time can run on through its wait; never past the plan.
        let long = store.start_session(&session(Species::Sprout, 25, 20_000)).unwrap();
        store.finish_session(long, 22_000, Outcome::Withered, 40 * 60).unwrap();
        // Still running: not counted.
        store.start_session(&session(Species::Sprout, 25, 30_000)).unwrap();
        assert_eq!(store.blocked_minutes().unwrap(), 50 + 12 + 25);
    }

    #[test]
    fn a_finished_session_puts_a_grown_plant_in_the_garden() {
        let mut store = Store::open_in_memory().unwrap();
        grow(&mut store, Species::Pine, 50, 1_000);

        let garden = store.garden().unwrap();
        assert_eq!(garden.len(), 1);
        assert_eq!(garden[0].species, Species::Pine);
        assert_eq!(garden[0].outcome, Outcome::Grown);
        assert_eq!(garden[0].planned_minutes, 50);
        assert_eq!(garden[0].created, 1_000 + 50 * 60);
    }

    #[test]
    fn withered_plants_stay_in_the_garden_but_earn_no_minutes() {
        let mut store = Store::open_in_memory().unwrap();
        grow(&mut store, Species::Sprout, 25, 1_000);
        let id = store.start_session(&session(Species::Pine, 50, 5_000)).unwrap();
        store
            .finish_session(id, 5_720, Outcome::Withered, 720)
            .unwrap();

        let garden = store.garden().unwrap();
        assert_eq!(garden.len(), 2);
        assert_eq!(garden[0].outcome, Outcome::Withered);
        assert_eq!(garden[0].focused_seconds, 720);
        assert_eq!(store.focused_minutes().unwrap(), 25);
        assert_eq!(store.finished_times().unwrap(), vec![1_000 + 25 * 60]);
    }

    #[test]
    fn garden_is_newest_first() {
        let mut store = Store::open_in_memory().unwrap();
        grow(&mut store, Species::Sprout, 25, 1_000);
        grow(&mut store, Species::Camellia, 45, 10_000);
        grow(&mut store, Species::Pine, 50, 20_000);
        let species: Vec<_> = store.garden().unwrap().iter().map(|p| p.species).collect();
        assert_eq!(species, [Species::Pine, Species::Camellia, Species::Sprout]);
    }

    #[test]
    fn finishing_twice_leaves_one_plant() {
        let mut store = Store::open_in_memory().unwrap();
        let id = grow(&mut store, Species::Pine, 50, 1_000);
        store.finish_session(id, 9_999, Outcome::Withered, 1).unwrap();

        let garden = store.garden().unwrap();
        assert_eq!(garden.len(), 1);
        assert_eq!(garden[0].outcome, Outcome::Grown);
    }

    #[test]
    fn sessions_left_running_wither_at_startup() {
        let mut store = Store::open_in_memory().unwrap();
        grow(&mut store, Species::Sprout, 25, 1_000);
        store.start_session(&session(Species::Pine, 50, 5_000)).unwrap();

        assert_eq!(store.wither_unfinished(6_000).unwrap(), 1);
        assert_eq!(store.wither_unfinished(7_000).unwrap(), 0);
        let garden = store.garden().unwrap();
        assert_eq!(garden[0].outcome, Outcome::Withered);
        assert_eq!(garden[0].species, Species::Pine);
    }

    #[test]
    fn discarding_removes_only_sessions_that_never_ended() {
        let mut store = Store::open_in_memory().unwrap();
        let grown = grow(&mut store, Species::Sprout, 25, 1_000);
        let pending = store.start_session(&session(Species::Pine, 50, 5_000)).unwrap();

        store.discard_session(grown).unwrap();
        store.discard_session(pending).unwrap();

        assert!(store.unfinished().unwrap().is_empty());
        assert_eq!(store.garden().unwrap().len(), 1);
        assert_eq!(store.focused_minutes().unwrap(), 25);
    }

    #[test]
    fn unfinished_lists_only_sessions_without_an_outcome() {
        let mut store = Store::open_in_memory().unwrap();
        grow(&mut store, Species::Sprout, 25, 1_000);
        let strict = NewSession {
            mode: Mode::Strict,
            ..session(Species::Pine, 50, 5_000)
        };
        let id = store.start_session(&strict).unwrap();
        store.record_slip(id, 5_100, "Discord").unwrap();

        assert_eq!(
            store.unfinished().unwrap(),
            vec![Unfinished {
                id,
                mode: Mode::Strict,
                species: Species::Pine,
                planned_minutes: 50,
                block_list: "Deep work".into(),
                slip_count: 1,
            }]
        );
    }

    #[test]
    fn minutes_since_counts_only_later_sessions() {
        let mut store = Store::open_in_memory().unwrap();
        grow(&mut store, Species::Sprout, 25, 1_000);
        grow(&mut store, Species::Pine, 50, 100_000);
        assert_eq!(store.focused_minutes().unwrap(), 75);
        assert_eq!(store.focused_minutes_since(50_000).unwrap(), 50);
        assert_eq!(store.focused_minutes_since(1_000_000).unwrap(), 0);
    }

    #[test]
    fn ended_and_slips_since_a_time() {
        let mut store = Store::open_in_memory().unwrap();
        grow(&mut store, Species::Sprout, 25, 1_000);
        let late = grow(&mut store, Species::Pine, 50, 100_000);
        let given_up = store.start_session(&session(Species::Maple, 90, 150_000)).unwrap();
        store.finish_session(given_up, 150_000 + 720, Outcome::Withered, 720).unwrap();
        let running = store.start_session(&session(Species::Pine, 50, 200_000)).unwrap();
        store.record_slip(late, 100_500, "Discord").unwrap();
        store.record_slip(running, 200_100, "Steam games").unwrap();

        assert_eq!(
            store.ended_since(50_000).unwrap(),
            vec![
                (100_000 + 50 * 60, Outcome::Grown, 50, 50 * 60),
                (150_000 + 720, Outcome::Withered, 90, 720),
            ]
        );
        assert_eq!(store.slips_since(150_000).unwrap(), vec![(200_100, "Steam games".to_owned())]);
    }

    #[test]
    fn slips_are_logged_and_counted() {
        let store = Store::open_in_memory().unwrap();
        let id = store.start_session(&session(Species::Pine, 50, 1_000)).unwrap();
        store.record_slip(id, 1_100, "Geometry Dash").unwrap();
        store.record_slip(id, 1_200, "Discord").unwrap();

        let count: u32 = store
            .db
            .query_row("SELECT slip_count FROM session WHERE id = ?1", [id], |r| r.get(0))
            .unwrap();
        let logged: u32 = store
            .db
            .query_row("SELECT COUNT(*) FROM slip WHERE session_id = ?1", [id], |r| r.get(0))
            .unwrap();
        assert_eq!((count, logged), (2, 2));
    }

    #[test]
    fn block_lists_round_trip() {
        let store = Store::open_in_memory().unwrap();
        assert!(store.block_lists().unwrap().is_empty());

        let mut config = BlockListConfig {
            steam: true,
            ..Default::default()
        };
        let id = store.add_block_list("Deep work", &config).unwrap();
        config.riot = true;
        store.update_block_list(id, &config).unwrap();

        assert_eq!(
            store.block_lists().unwrap(),
            vec![StoredBlockList {
                id,
                name: "Deep work".into(),
                config,
            }]
        );
    }

    #[test]
    fn settings_overwrite() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.setting("always_on_top").unwrap(), None);
        store.set_setting("always_on_top", "true").unwrap();
        store.set_setting("always_on_top", "false").unwrap();
        assert_eq!(store.setting("always_on_top").unwrap().as_deref(), Some("false"));
    }

    #[test]
    fn reopening_keeps_the_data() {
        let path = std::env::temp_dir().join(format!("niwa-store-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let mut store = Store::open(&path).unwrap();
            grow(&mut store, Species::Pine, 50, 1_000);
        }
        let reopened = Store::open(&path).unwrap();
        let plants = reopened.garden().unwrap().len();
        drop(reopened);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(plants, 1);
    }
}
