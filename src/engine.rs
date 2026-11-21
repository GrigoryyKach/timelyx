//! The session builder: a small state machine that turns focus changes, idle checks, lock, sleep and pause
//! into finished sessions. It knows nothing about the operating system or the database, so every rule below
//! is covered by unit tests with a synthetic event stream.
//!
//! Times are Unix seconds. The open session lives in memory only; `flush` hands out what is finished (and a
//! slice of the open one, so a crash loses at most a minute).

use chrono::{Datelike, NaiveDate, TimeZone};

/// What a session is about. Two sessions with the same target are the same activity.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Target {
    /// Full path of the executable (plus `#vim` for a terminal that runs an editor).
    pub exe: String,
    /// Normalised site name for browsers (`sites::PRIVATE` and `sites::OTHER` included).
    pub site: Option<String>,
}

/// The foreground window as the engine needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Window {
    pub target: Target,
    pub fullscreen: bool,
    /// Idle time does not stop the clock (fullscreen, or a Video or Games category).
    pub keep_when_idle: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    pub start: i64,
    pub end: i64,
    pub target: Target,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    Lock,
    Sleep,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Running,
    /// No input for longer than the threshold; the session was closed at the last input.
    Idle { since: i64 },
    Stopped(Stop),
    Paused { until: i64 },
}

/// Sessions shorter than this are dropped (a window that flashes by).
const MIN_SESSION: i64 = 1;
/// A gap between two ticks longer than this means the computer slept or the process was suspended.
const SLEEP_GAP: i64 = 60;

pub struct Engine {
    idle_secs: i64,
    state: State,
    open: Option<(i64, Window)>,
    current: Option<Window>,
    done: Vec<Session>,
    last_tick: Option<i64>,
}

impl Engine {
    /// `idle_secs == 0` turns idle detection off.
    pub fn new(idle_secs: i64) -> Self {
        Self { idle_secs, state: State::Running, open: None, current: None, done: Vec::new(), last_tick: None }
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn set_idle_secs(&mut self, idle_secs: i64) {
        self.idle_secs = idle_secs;
    }

    /// The open session as (start, window), if one is being recorded.
    pub fn open_session(&self) -> Option<(i64, &Window)> {
        self.open.as_ref().map(|(s, w)| (*s, w))
    }

    fn close_open(&mut self, at: i64) {
        if let Some((start, w)) = self.open.take() {
            if at - start >= MIN_SESSION {
                self.done.push(Session { start, end: at, target: w.target });
            }
        }
    }

    fn reopen(&mut self, at: i64) {
        self.open = self.current.clone().map(|w| (at, w));
    }

    /// The foreground window changed (or its title did; for a browser the target then carries the new site).
    pub fn focus(&mut self, now: i64, window: Option<Window>) {
        self.current = window.clone();
        if self.state != State::Running {
            return;
        }
        match (&mut self.open, window) {
            (Some((_, open)), Some(new)) if open.target == new.target => {
                // Same activity: only the flags may have changed (fullscreen entered, category edited).
                *open = new;
            }
            (_, new) => {
                self.close_open(now);
                self.open = new.map(|w| (now, w));
            }
        }
    }

    /// Periodic check (every 5 to 10 seconds). `last_input` is the time of the last keyboard or mouse input.
    pub fn tick(&mut self, now: i64, last_input: i64) {
        // The computer slept: close the session where the last tick saw it alive.
        if let Some(lt) = self.last_tick {
            if now - lt > SLEEP_GAP {
                self.close_open(lt);
                if self.state == State::Running {
                    self.state = State::Idle { since: lt };
                }
            }
        }
        self.last_tick = Some(now);
        match self.state {
            State::Paused { until } if now >= until => {
                self.state = State::Running;
                self.reopen(now);
            }
            State::Running => {
                if let Some((start, w)) = &self.open {
                    if self.idle_secs > 0 && !w.keep_when_idle && now - last_input >= self.idle_secs {
                        // Back-dated: the idle minutes are not counted.
                        let at = last_input.max(*start);
                        self.close_open(at);
                        self.state = State::Idle { since: at };
                    }
                } else if self.current.is_some() {
                    self.reopen(now);
                }
            }
            State::Idle { since } => {
                if last_input > since {
                    self.state = State::Running;
                    self.reopen(last_input.min(now));
                }
            }
            _ => {}
        }
    }

    /// Lock, sleep or shutdown: the session ends now and nothing counts until `resume`.
    pub fn stop(&mut self, now: i64, why: Stop) {
        if matches!(self.state, State::Paused { .. }) {
            return;
        }
        self.close_open(now);
        self.state = State::Stopped(why);
    }

    /// Unlock or wake: record the foreground window again.
    pub fn resume(&mut self, now: i64) {
        if matches!(self.state, State::Stopped(_) | State::Idle { .. }) {
            self.state = State::Running;
            self.reopen(now);
        }
    }

    /// After a wake-up: record again only from the first input that follows.
    pub fn await_input(&mut self, now: i64) {
        if matches!(self.state, State::Stopped(_)) {
            self.state = State::Idle { since: now };
        }
    }

    /// The user paused tracking until `until` (`i64::MAX` for "until resumed").
    pub fn pause(&mut self, now: i64, until: i64) {
        self.close_open(now);
        self.state = State::Paused { until };
    }

    pub fn unpause(&mut self, now: i64) {
        if matches!(self.state, State::Paused { .. }) {
            self.state = State::Running;
            self.reopen(now);
        }
    }

    /// Hands out finished sessions plus a slice of the open one ending at `now`; the open session continues
    /// from `now`, so nothing is counted twice.
    pub fn flush(&mut self, now: i64) -> Vec<Session> {
        if let Some((start, w)) = &mut self.open {
            if now - *start >= MIN_SESSION {
                self.done.push(Session { start: *start, end: now, target: w.target.clone() });
                *start = now;
            }
        }
        std::mem::take(&mut self.done)
    }

    /// Final flush: closes the open session and returns everything.
    pub fn finish(&mut self, now: i64) -> Vec<Session> {
        self.close_open(now);
        std::mem::take(&mut self.done)
    }
}

// ───────────────────────── days ─────────────────────────

/// Start (Unix seconds) of the local day after the one that contains `ts`.
pub fn next_midnight<Tz: TimeZone>(tz: &Tz, ts: i64) -> i64 {
    let Some(dt) = tz.timestamp_opt(ts, 0).single() else { return ts + 86_400 };
    let next: NaiveDate = dt.date_naive().succ_opt().unwrap_or(dt.date_naive());
    // A midnight that does not exist (a DST jump at 00:00) is replaced by the first valid moment of that day.
    for h in 0..4 {
        if let Some(t) = tz.from_local_datetime(&next.and_hms_opt(h, 0, 0).unwrap()).earliest() {
            let t = t.timestamp();
            if t > ts {
                return t;
            }
        }
    }
    ts + 86_400
}

/// Start (Unix seconds) of the local day that contains `ts`.
pub fn day_start<Tz: TimeZone>(tz: &Tz, ts: i64) -> i64 {
    let Some(dt) = tz.timestamp_opt(ts, 0).single() else { return ts - ts.rem_euclid(86_400) };
    let d = dt.date_naive();
    for h in 0..4 {
        if let Some(t) = tz.from_local_datetime(&d.and_hms_opt(h, 0, 0).unwrap()).earliest() {
            return t.timestamp();
        }
    }
    ts - ts.rem_euclid(86_400)
}

/// Splits `[start, end)` at local midnights, so a session never spans two days.
pub fn split_days<Tz: TimeZone>(tz: &Tz, start: i64, end: i64) -> Vec<(i64, i64)> {
    let mut out = Vec::new();
    let mut cur = start;
    while cur < end {
        let next = next_midnight(tz, cur);
        if end <= next {
            out.push((cur, end));
            break;
        }
        out.push((cur, next));
        cur = next;
    }
    out
}

/// Monday = 0 .. Sunday = 6 for the local day of `ts`.
pub fn weekday_index<Tz: TimeZone>(tz: &Tz, ts: i64) -> usize {
    tz.timestamp_opt(ts, 0).single().map(|d| d.weekday().num_days_from_monday() as usize).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, Offset};

    fn t(exe: &str) -> Target {
        Target { exe: exe.into(), site: None }
    }

    fn w(exe: &str) -> Window {
        Window { target: t(exe), fullscreen: false, keep_when_idle: false }
    }

    fn site(exe: &str, s: &str) -> Window {
        Window { target: Target { exe: exe.into(), site: Some(s.into()) }, fullscreen: false, keep_when_idle: false }
    }

    fn sessions(v: &[Session]) -> Vec<(i64, i64, String)> {
        v.iter().map(|s| (s.start, s.end, s.target.exe.clone())).collect()
    }

    #[test]
    fn switching_apps_makes_back_to_back_sessions() {
        let mut e = Engine::new(300);
        e.focus(100, Some(w("a")));
        e.focus(160, Some(w("b")));
        e.focus(200, Some(w("a")));
        let out = e.finish(260);
        assert_eq!(sessions(&out), vec![(100, 160, "a".into()), (160, 200, "b".into()), (200, 260, "a".into())]);
    }

    #[test]
    fn same_target_does_not_split() {
        let mut e = Engine::new(300);
        e.focus(0, Some(site("chrome", "YouTube")));
        e.focus(50, Some(site("chrome", "YouTube")));
        e.focus(100, Some(site("chrome", "GitHub")));
        let out = e.finish(150);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].target.site.as_deref(), Some("YouTube"));
        assert_eq!((out[0].start, out[0].end), (0, 100));
    }

    #[test]
    fn flashing_windows_are_dropped() {
        let mut e = Engine::new(300);
        e.focus(0, Some(w("a")));
        e.focus(100, Some(w("b")));
        e.focus(100, Some(w("a"))); // zero-length b
        let out = e.finish(120);
        assert_eq!(sessions(&out), vec![(0, 100, "a".into()), (100, 120, "a".into())]);
    }

    #[test]
    fn idle_is_back_dated_to_the_last_input() {
        let mut e = Engine::new(300);
        e.focus(1000, Some(w("a")));
        // The user typed until 1100, then left.
        for now in (1005..=1400).step_by(5) {
            e.tick(now, 1100.min(now));
        }
        // At 1400 idle is 300 s: the session ends at 1100.
        assert_eq!(e.state(), State::Idle { since: 1100 });
        let out = e.finish(1400);
        assert_eq!(sessions(&out), vec![(1000, 1100, "a".into())]);
    }

    #[test]
    fn coming_back_starts_a_new_session_at_the_first_input() {
        let mut e = Engine::new(300);
        e.focus(0, Some(w("a")));
        for now in (5..=300).step_by(5) {
            e.tick(now, 0);
        }
        assert!(matches!(e.state(), State::Idle { .. }));
        // Away until 1000; input at 998, seen by the tick at 1000.
        for now in (305..995).step_by(5) {
            e.tick(now, 0);
        }
        e.tick(1000, 998);
        assert_eq!(e.state(), State::Running);
        let out = e.finish(1060);
        assert_eq!(sessions(&out), vec![(998, 1060, "a".into())]);
    }

    #[test]
    fn idle_threshold_zero_disables_idle() {
        let mut e = Engine::new(0);
        e.focus(0, Some(w("a")));
        e.tick(5000, 0);
        assert_eq!(e.state(), State::Running);
        assert_eq!(e.finish(5000).len(), 1);
    }

    #[test]
    fn fullscreen_or_video_exception_keeps_counting() {
        let mut e = Engine::new(300);
        let mut game = w("game");
        game.fullscreen = true;
        game.keep_when_idle = true;
        e.focus(0, Some(game));
        for now in (5..=3600).step_by(5) {
            e.tick(now, 0);
        }
        assert_eq!(e.state(), State::Running);
        let out = e.finish(3600);
        assert_eq!(sessions(&out), vec![(0, 3600, "game".into())]);
    }

    #[test]
    fn exception_ends_when_the_user_leaves_the_game() {
        let mut e = Engine::new(300);
        let mut game = w("game");
        game.keep_when_idle = true;
        e.focus(0, Some(game));
        e.tick(1000, 0);
        e.focus(1000, Some(w("editor")));
        e.tick(1005, 0);
        // The editor is not exempt: idle 1005 s >= 300 s, closes at max(last_input, start) = start.
        assert!(matches!(e.state(), State::Idle { .. }));
        let out = e.finish(1005);
        assert_eq!(sessions(&out), vec![(0, 1000, "game".into())]);
    }

    #[test]
    fn lock_stops_and_unlock_resumes() {
        let mut e = Engine::new(300);
        e.focus(0, Some(w("a")));
        e.stop(100, Stop::Lock);
        // Nothing counts while locked, even if the foreground "changes".
        e.focus(200, Some(w("b")));
        e.tick(300, 100);
        assert_eq!(e.state(), State::Stopped(Stop::Lock));
        e.resume(1000);
        let out = e.finish(1100);
        assert_eq!(sessions(&out), vec![(0, 100, "a".into()), (1000, 1100, "b".into())]);
    }

    #[test]
    fn wake_up_counts_only_from_the_first_input() {
        let mut e = Engine::new(300);
        e.focus(0, Some(w("a")));
        e.stop(100, Stop::Sleep);
        e.await_input(5000);
        // A timed wake-up: no input yet.
        e.tick(5005, 100);
        assert!(matches!(e.state(), State::Idle { .. }));
        // The user sits down and types at 5100.
        e.tick(5105, 5100);
        assert_eq!(e.state(), State::Running);
        assert_eq!(sessions(&e.finish(5200)), vec![(0, 100, "a".into()), (5100, 5200, "a".into())]);
    }

    #[test]
    fn sleep_gap_closes_at_the_last_tick() {
        let mut e = Engine::new(300);
        e.focus(0, Some(w("a")));
        e.tick(5, 5);
        e.tick(10, 10);
        // The lid closed at ~10 and opened at 30000; the wake-up input is at 29990.
        e.tick(30000, 29990);
        assert_eq!(e.state(), State::Running);
        let out = e.finish(30060);
        assert_eq!(sessions(&out), vec![(0, 10, "a".into()), (29990, 30060, "a".into())]);
    }

    #[test]
    fn sleep_without_input_stays_idle() {
        let mut e = Engine::new(300);
        e.focus(0, Some(w("a")));
        e.tick(5, 5);
        // A timed wake-up at 30000 with no input since 5: nothing is counted.
        e.tick(30000, 5);
        assert!(matches!(e.state(), State::Idle { .. }));
        assert_eq!(sessions(&e.finish(30005)), vec![(0, 5, "a".into())]);
    }

    #[test]
    fn pause_stops_counting_and_expires() {
        let mut e = Engine::new(300);
        e.focus(0, Some(w("a")));
        e.pause(100, 100 + 900);
        e.focus(500, Some(w("b")));
        e.tick(600, 600);
        assert_eq!(e.state(), State::Paused { until: 1000 });
        e.tick(1000, 1000);
        assert_eq!(e.state(), State::Running);
        let out = e.finish(1100);
        assert_eq!(sessions(&out), vec![(0, 100, "a".into()), (1000, 1100, "b".into())]);
    }

    #[test]
    fn pause_until_resumed() {
        let mut e = Engine::new(300);
        e.focus(0, Some(w("a")));
        e.pause(10, i64::MAX);
        e.tick(100_000, 100_000);
        assert!(matches!(e.state(), State::Paused { .. }));
        e.unpause(100_000);
        assert_eq!(e.state(), State::Running);
        assert_eq!(sessions(&e.finish(100_050)), vec![(0, 10, "a".into()), (100_000, 100_050, "a".into())]);
    }

    #[test]
    fn flush_slices_the_open_session_without_double_counting() {
        let mut e = Engine::new(300);
        e.focus(0, Some(w("a")));
        let first = e.flush(60);
        assert_eq!(sessions(&first), vec![(0, 60, "a".into())]);
        let second = e.flush(120);
        assert_eq!(sessions(&second), vec![(60, 120, "a".into())]);
        e.focus(150, Some(w("b")));
        let rest = e.finish(200);
        assert_eq!(sessions(&rest), vec![(120, 150, "a".into()), (150, 200, "b".into())]);
        let total: i64 = first.iter().chain(&second).chain(&rest).map(|s| s.end - s.start).sum();
        assert_eq!(total, 200);
    }

    #[test]
    fn no_window_closes_the_session() {
        let mut e = Engine::new(300);
        e.focus(0, Some(w("a")));
        e.focus(50, None);
        e.tick(100, 100);
        e.focus(120, Some(w("a")));
        assert_eq!(sessions(&e.finish(150)), vec![(0, 50, "a".into()), (120, 150, "a".into())]);
    }

    #[test]
    fn midnight_split_in_a_fixed_offset_zone() {
        let tz = FixedOffset::east_opt(2 * 3600).unwrap();
        // 2026-11-20 22:00 local = 20:00 UTC.
        let start = chrono::Utc.with_ymd_and_hms(2026, 11, 20, 20, 0, 0).unwrap().timestamp();
        let end = start + 5 * 3600; // until 03:00 local on the 21st
        let parts = split_days(&tz, start, end);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].1 - parts[0].0, 2 * 3600);
        assert_eq!(parts[1].1 - parts[1].0, 3 * 3600);
        assert_eq!(parts[0].1, parts[1].0);
        assert_eq!(parts[0].1, chrono::Utc.with_ymd_and_hms(2026, 11, 20, 22, 0, 0).unwrap().timestamp());
        // Three days in one go.
        assert_eq!(split_days(&tz, start, start + 60 * 3600).len(), 4);
        assert!(split_days(&tz, 10, 10).is_empty());
    }

    /// A zone that moves its clocks one hour forward at `jump` (UTC seconds).
    #[derive(Clone, Copy)]
    struct Dst {
        jump: i64,
    }
    #[derive(Clone, Copy, Debug)]
    struct DstOffset(i32);
    impl Offset for DstOffset {
        fn fix(&self) -> FixedOffset {
            FixedOffset::east_opt(self.0).unwrap()
        }
    }
    impl std::fmt::Display for DstOffset {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }
    impl TimeZone for Dst {
        type Offset = DstOffset;
        fn from_offset(_: &DstOffset) -> Self {
            Dst { jump: i64::MAX }
        }
        fn offset_from_local_date(&self, _: &NaiveDate) -> chrono::LocalResult<DstOffset> {
            chrono::LocalResult::Single(DstOffset(3600))
        }
        fn offset_from_local_datetime(&self, local: &chrono::NaiveDateTime) -> chrono::LocalResult<DstOffset> {
            let as_std = local.and_utc().timestamp() - 3600;
            let as_dst = local.and_utc().timestamp() - 7200;
            let std_ok = as_std < self.jump;
            let dst_ok = as_dst >= self.jump;
            match (std_ok, dst_ok) {
                (true, true) => chrono::LocalResult::Ambiguous(DstOffset(3600), DstOffset(7200)),
                (true, false) => chrono::LocalResult::Single(DstOffset(3600)),
                (false, true) => chrono::LocalResult::Single(DstOffset(7200)),
                (false, false) => chrono::LocalResult::None,
            }
        }
        fn offset_from_utc_date(&self, _: &NaiveDate) -> DstOffset {
            DstOffset(3600)
        }
        fn offset_from_utc_datetime(&self, utc: &chrono::NaiveDateTime) -> DstOffset {
            DstOffset(if utc.and_utc().timestamp() >= self.jump { 7200 } else { 3600 })
        }
    }

    #[test]
    fn days_across_a_clock_change_are_23_hours_long() {
        // Spring forward at local 02:00 on 2026-03-29 (UTC+1 -> UTC+2): 01:00 UTC.
        let jump = chrono::Utc.with_ymd_and_hms(2026, 3, 29, 1, 0, 0).unwrap().timestamp();
        let tz = Dst { jump };
        let day_a = day_start(&tz, jump - 3600); // 2026-03-29 00:00 local = 23:00 UTC on the 28th
        let next = next_midnight(&tz, day_a);
        // The 29th has only 23 hours.
        assert_eq!(next - day_a, 23 * 3600);
        // A session across the whole 29th, 30th splits into 23 h + the rest.
        let parts = split_days(&tz, day_a, day_a + 30 * 3600);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].1 - parts[0].0, 23 * 3600);
        assert_eq!(parts[1].1 - parts[1].0, 7 * 3600);
        // Total time is preserved.
        assert_eq!(parts.iter().map(|p| p.1 - p.0).sum::<i64>(), 30 * 3600);
    }

    #[test]
    fn weekday_index_is_monday_based() {
        let tz = FixedOffset::east_opt(0).unwrap();
        let mon = chrono::Utc.with_ymd_and_hms(2026, 11, 16, 12, 0, 0).unwrap().timestamp(); // a Monday
        assert_eq!(weekday_index(&tz, mon), 0);
        assert_eq!(weekday_index(&tz, mon + 6 * 86_400), 6);
    }
}
