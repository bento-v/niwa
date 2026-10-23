//! Strict sessions: what the background service keeps on disk, how the
//! cooldown exit works, and the seal that makes an edited state file fail.

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::config::{BlockListConfig, Mode};
use crate::schedule::MAX_SCHEDULED_MINUTES;
use crate::species::{MAX_SESSION_MINUTES, MIN_SESSION_MINUTES};
use crate::store::Outcome;

/// What a Strict session was started with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrictPlan {
    /// The session's id in the card's database, so the two can be matched up.
    pub session_id: i64,
    pub species: String,
    pub planned_minutes: u32,
    pub list_name: String,
    pub config: BlockListConfig,
    /// Started by a schedule, which may run up to `MAX_SCHEDULED_MINUTES`.
    #[serde(default)]
    pub scheduled: bool,
}

impl StrictPlan {
    pub fn total_secs(&self) -> u64 {
        u64::from(self.planned_minutes) * 60
    }

    /// How long a request to end this session early waits.
    pub fn exit_wait_secs(&self) -> u64 {
        Mode::Strict.exit_wait_secs(self.planned_minutes)
    }

    pub fn validate(&self) -> Result<(), String> {
        check_length(self.planned_minutes, self.scheduled)
    }
}

/// Sessions run from `MIN_SESSION_MINUTES` up to `MAX_SESSION_MINUTES`, or
/// `MAX_SCHEDULED_MINUTES` when a schedule started them.
pub fn check_length(minutes: u32, scheduled: bool) -> Result<(), String> {
    let most = match scheduled {
        true => MAX_SCHEDULED_MINUTES,
        false => MAX_SESSION_MINUTES,
    };
    if !(MIN_SESSION_MINUTES..=most).contains(&minutes) {
        return Err(format!("Sessions run from {MIN_SESSION_MINUTES} minutes to {} hours.", most / 60));
    }
    Ok(())
}

/// A blocked app that was launched during the session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlipRecord {
    /// Unix seconds.
    pub at: i64,
    pub label: String,
}

/// Everything about a Strict session that must survive the card closing, the
/// service restarting and the PC rebooting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrictState {
    pub plan: StrictPlan,
    /// The Windows account the session belongs to. Only that account's
    /// programs are ever closed.
    pub user_sid: String,
    /// Seconds of the session used so far, counted on the awake clock. Time
    /// with the PC asleep or switched off isn't in it.
    pub elapsed_secs: u64,
    /// When, in session seconds, an early end was asked for.
    pub exit_requested_at: Option<u64>,
    pub slips: Vec<SlipRecord>,
    /// Set once the session is over. The state is kept until the next session
    /// so the card can collect the result whenever it next runs.
    pub outcome: Option<EndedAs>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EndedAs {
    Grown,
    Withered,
}

impl From<EndedAs> for Outcome {
    fn from(ended: EndedAs) -> Self {
        match ended {
            EndedAs::Grown => Outcome::Grown,
            EndedAs::Withered => Outcome::Withered,
        }
    }
}

impl StrictState {
    pub fn new(plan: StrictPlan, user_sid: String) -> Self {
        StrictState {
            plan,
            user_sid,
            elapsed_secs: 0,
            exit_requested_at: None,
            slips: Vec::new(),
            outcome: None,
        }
    }

    pub fn is_running(&self) -> bool {
        self.outcome.is_none()
    }

    pub fn remaining_secs(&self) -> u64 {
        self.plan.total_secs().saturating_sub(self.elapsed_secs)
    }

    /// Seconds until a requested early end takes effect, if one is pending.
    pub fn exit_secs_left(&self) -> Option<u64> {
        let requested = self.exit_requested_at?;
        Some((requested + self.plan.exit_wait_secs()).saturating_sub(self.elapsed_secs))
    }

    /// Moves the session on to `elapsed_secs` and ends it if it is over.
    ///
    /// Finishing wins over a pending early end: someone who asked to leave
    /// but was still there at the end has finished the session.
    pub fn advance(&mut self, elapsed_secs: u64) {
        if !self.is_running() {
            return;
        }
        let total = self.plan.total_secs();
        self.elapsed_secs = elapsed_secs.min(total);
        if self.elapsed_secs >= total {
            self.outcome = Some(EndedAs::Grown);
        } else if self.exit_secs_left() == Some(0) {
            self.outcome = Some(EndedAs::Withered);
        }
    }

    /// Asks to end early. The wait starts now; asking again doesn't restart it.
    pub fn request_exit(&mut self) {
        if self.is_running() && self.exit_requested_at.is_none() {
            self.exit_requested_at = Some(self.elapsed_secs);
        }
    }

    pub fn cancel_exit(&mut self) {
        if self.is_running() {
            self.exit_requested_at = None;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SealError {
    /// The file isn't a sealed state at all.
    Malformed,
    /// The contents don't match the seal: the file was edited.
    Tampered,
}

#[derive(Serialize, Deserialize)]
struct Sealed {
    state: String,
    seal: String,
}

fn mac(key: &[u8], state: &str) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts a key of any length");
    mac.update(state.as_bytes());
    mac
}

/// Serialises a state together with a keyed checksum of it. Only something
/// holding `key` can produce a seal that `open` accepts.
pub fn seal(key: &[u8], state: &StrictState) -> String {
    let state = serde_json::to_string(state).expect("strict state is plain data");
    let seal = mac(key, &state)
        .finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    serde_json::to_string(&Sealed { state, seal }).expect("sealed state is plain data")
}

/// Reads a sealed state back, refusing it if it was changed after sealing.
pub fn open(key: &[u8], text: &str) -> Result<StrictState, SealError> {
    let sealed: Sealed = serde_json::from_str(text).map_err(|_| SealError::Malformed)?;
    if !sealed.seal.len().is_multiple_of(2) {
        return Err(SealError::Malformed);
    }
    let claimed: Vec<u8> = (0..sealed.seal.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(sealed.seal.get(i..i + 2).unwrap_or("zz"), 16))
        .collect::<Result<_, _>>()
        .map_err(|_| SealError::Malformed)?;
    mac(key, &sealed.state)
        .verify_slice(&claimed)
        .map_err(|_| SealError::Tampered)?;
    serde_json::from_str(&sealed.state).map_err(|_| SealError::Malformed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(minutes: u32) -> StrictState {
        StrictState::new(
            StrictPlan {
                session_id: 7,
                species: "pine".into(),
                planned_minutes: minutes,
                list_name: "Deep work".into(),
                config: BlockListConfig::default(),
                scheduled: false,
            },
            "S-1-5-21-1-2-3-1001".into(),
        )
    }

    #[test]
    fn only_scheduled_sessions_run_past_five_hours() {
        let mut plan = state(50).plan;
        plan.planned_minutes = 8 * 60;
        assert!(plan.validate().is_err());
        plan.scheduled = true;
        assert!(plan.validate().is_ok());
        plan.planned_minutes = 12 * 60 + 1;
        assert!(plan.validate().is_err());
        // Plans saved before schedules existed read as not scheduled.
        let old = r#"{"sessionId":1,"species":"pine","plannedMinutes":50,"listName":"Deep work","config":{}}"#;
        assert!(!serde_json::from_str::<StrictPlan>(old).unwrap().scheduled);
    }

    #[test]
    fn a_session_left_alone_finishes_grown() {
        let mut s = state(50);
        s.advance(49 * 60);
        assert!(s.is_running());
        assert_eq!(s.remaining_secs(), 60);
        s.advance(50 * 60);
        assert_eq!(s.outcome, Some(EndedAs::Grown));
    }

    #[test]
    fn elapsed_never_passes_the_total() {
        let mut s = state(50);
        s.advance(10_000_000);
        assert_eq!(s.elapsed_secs, 50 * 60);
        assert_eq!(s.remaining_secs(), 0);
    }

    #[test]
    fn asking_to_end_takes_a_third_of_the_session() {
        let mut s = state(90);
        s.advance(10 * 60);
        s.request_exit();
        assert_eq!(s.exit_secs_left(), Some(30 * 60));

        s.advance(39 * 60 + 59);
        assert!(s.is_running());
        assert_eq!(s.exit_secs_left(), Some(1));

        s.advance(40 * 60);
        assert_eq!(s.outcome, Some(EndedAs::Withered));
        assert_eq!(s.remaining_secs(), 50 * 60);
    }

    #[test]
    fn asking_twice_does_not_restart_the_wait() {
        let mut s = state(90);
        s.advance(600);
        s.request_exit();
        s.advance(900);
        s.request_exit();
        assert_eq!(s.exit_requested_at, Some(600));
    }

    #[test]
    fn a_cancelled_request_lets_the_session_run_on() {
        let mut s = state(90);
        s.advance(600);
        s.request_exit();
        s.advance(1200);
        s.cancel_exit();
        assert_eq!(s.exit_secs_left(), None);
        s.advance(600 + s.plan.exit_wait_secs() + 60);
        assert!(s.is_running());
    }

    #[test]
    fn finishing_beats_a_pending_request() {
        // Asked to end with ten minutes left: the session finishes first.
        let mut s = state(50);
        s.advance(40 * 60);
        s.request_exit();
        s.advance(50 * 60);
        assert_eq!(s.outcome, Some(EndedAs::Grown));
    }

    #[test]
    fn nothing_changes_once_it_has_ended() {
        let mut s = state(50);
        s.advance(50 * 60);
        s.request_exit();
        s.advance(10);
        assert_eq!(s.outcome, Some(EndedAs::Grown));
        assert_eq!(s.exit_requested_at, None);
        assert_eq!(s.elapsed_secs, 50 * 60);
    }

    #[test]
    fn plans_outside_the_allowed_lengths_are_refused() {
        assert!(state(10).plan.validate().is_ok());
        assert!(state(300).plan.validate().is_ok());
        assert!(state(9).plan.validate().is_err());
        assert!(state(301).plan.validate().is_err());
    }

    #[test]
    fn a_sealed_state_opens_with_the_same_key() {
        let mut s = state(50);
        s.advance(1234);
        s.slips.push(SlipRecord {
            at: 99,
            label: "Geometry Dash".into(),
        });
        let sealed = seal(b"key", &s);
        assert_eq!(open(b"key", &sealed), Ok(s));
    }

    #[test]
    fn an_edited_state_is_rejected() {
        let mut s = state(90);
        s.advance(60);
        let sealed = seal(b"key", &s);
        // Someone edits the file to say the session is nearly over.
        let edited = sealed.replace(r#"\"elapsedSecs\":60"#, r#"\"elapsedSecs\":5399"#);
        assert_ne!(edited, sealed);
        assert_eq!(open(b"key", &edited), Err(SealError::Tampered));
    }

    #[test]
    fn a_state_sealed_with_another_key_is_rejected() {
        let sealed = seal(b"key", &state(50));
        assert_eq!(open(b"other", &sealed), Err(SealError::Tampered));
    }

    #[test]
    fn rubbish_is_malformed() {
        assert_eq!(open(b"key", "not json"), Err(SealError::Malformed));
        assert_eq!(open(b"key", r#"{"state":"{}","seal":"xyz"}"#), Err(SealError::Malformed));
    }
}
