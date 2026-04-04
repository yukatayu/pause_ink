use std::cmp::Ordering;

mod history;

use serde::{Deserialize, Serialize};

pub use history::*;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TimeBase {
    pub numerator: u32,
    pub denominator: u32,
}

impl TimeBase {
    pub const fn new(numerator: u32, denominator: u32) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    pub const fn milliseconds() -> Self {
        Self::new(1, 1_000)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MediaTime {
    pub ticks: i64,
    pub time_base: TimeBase,
}

impl MediaTime {
    pub const fn new(ticks: i64, time_base: TimeBase) -> Self {
        Self { ticks, time_base }
    }

    pub const fn from_millis(value: i64) -> Self {
        Self::new(value, TimeBase::milliseconds())
    }

    fn ordering_key(self, other: Self) -> (i128, i128) {
        let left = self.ticks as i128
            * self.time_base.numerator as i128
            * other.time_base.denominator as i128;
        let right = other.ticks as i128
            * other.time_base.numerator as i128
            * self.time_base.denominator as i128;
        (left, right)
    }
}

impl PartialEq for TimeBase {
    fn eq(&self, other: &Self) -> bool {
        self.numerator == other.numerator && self.denominator == other.denominator
    }
}

impl Eq for TimeBase {}

impl PartialEq for MediaTime {
    fn eq(&self, other: &Self) -> bool {
        let (left, right) = self.ordering_key(*other);
        left == right
    }
}

impl Eq for MediaTime {}

impl PartialOrd for MediaTime {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MediaTime {
    fn cmp(&self, other: &Self) -> Ordering {
        let (left, right) = self.ordering_key(*other);
        left.cmp(&right)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClearKind {
    Instant,
    Ordered,
    ReverseOrdered,
    WipeOut,
    DissolveOut,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClearEvent {
    pub time: MediaTime,
    pub kind: ClearKind,
}

pub fn page_index_for_time(clears: &[ClearEvent], time: MediaTime) -> usize {
    clears.iter().filter(|clear| clear.time <= time).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_boundary_belongs_to_next_page_even_with_mixed_time_bases() {
        let ntsc_tick = TimeBase::new(1, 30_000);
        let clears = vec![
            ClearEvent {
                time: MediaTime::new(1001, ntsc_tick),
                kind: ClearKind::Instant,
            },
        ];

        let just_before = MediaTime::new(1000, ntsc_tick);
        let exactly_on_clear = MediaTime::new(1, TimeBase::new(1001, 30_000));

        assert_eq!(page_index_for_time(&clears, just_before), 0);
        assert_eq!(page_index_for_time(&clears, exactly_on_clear), 1);
    }

    #[test]
    fn media_time_compares_across_time_bases() {
        let one_second = MediaTime::from_millis(1_000);
        let ntsc_frame = MediaTime::new(1001, TimeBase::new(1, 30_000));

        assert!(one_second > ntsc_frame);
        assert_eq!(one_second, MediaTime::new(1, TimeBase::new(1, 1)));
    }

    #[test]
    fn history_respects_depth_limit_and_invalidates_redo() {
        let mut state = Vec::<String>::new();
        let mut history = CommandHistory::with_limit(2);

        history
            .apply(&mut state, Box::new(PushValue("first")))
            .expect("first command should apply");
        history
            .apply(&mut state, Box::new(PushValue("second")))
            .expect("second command should apply");
        history
            .apply(&mut state, Box::new(PushValue("third")))
            .expect("third command should apply");

        assert_eq!(state, vec!["first", "second", "third"]);

        assert!(history.undo(&mut state).expect("undo should succeed"));
        assert_eq!(state, vec!["first", "second"]);
        assert!(history.undo(&mut state).expect("undo should succeed"));
        assert_eq!(state, vec!["first"]);
        assert!(!history
            .undo(&mut state)
            .expect("oldest command should be evicted at depth limit"));

        history
            .apply(&mut state, Box::new(PushValue("replacement")))
            .expect("new command should apply");
        assert!(!history
            .redo(&mut state)
            .expect("redo should be invalidated after a new command"));
    }

    #[test]
    fn grouped_commands_undo_in_reverse_order() {
        let mut state = Vec::<String>::new();
        let mut history = CommandHistory::with_limit(DEFAULT_HISTORY_DEPTH);

        history
            .apply(
                &mut state,
                Box::new(CommandBatch::new(vec![
                    Box::new(PushValue("first")),
                    Box::new(PushValue("second")),
                ])),
            )
            .expect("batched command should apply");

        assert_eq!(state, vec!["first", "second"]);
        assert!(history.undo(&mut state).expect("batch undo should succeed"));
        assert!(state.is_empty());
    }

    struct PushValue(&'static str);

    impl Command<Vec<String>> for PushValue {
        fn apply(&self, state: &mut Vec<String>) -> Result<(), CommandError> {
            state.push(self.0.to_owned());
            Ok(())
        }

        fn undo(&self, state: &mut Vec<String>) -> Result<(), CommandError> {
            match state.pop() {
                Some(value) if value == self.0 => Ok(()),
                Some(value) => Err(CommandError::new(format!(
                    "unexpected undo order: expected {}, got {}",
                    self.0, value
                ))),
                None => Err(CommandError::new("state was empty during undo")),
            }
        }
    }
}
