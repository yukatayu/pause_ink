use serde::{Deserialize, Serialize};

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
    pub time_ms: u64,
    pub kind: ClearKind,
}

pub fn page_index_for_time(clears: &[ClearEvent], time_ms: u64) -> usize {
    clears.iter().filter(|c| c.time_ms <= time_ms).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_index_counts_prior_clears() {
        let clears = vec![
            ClearEvent { time_ms: 1000, kind: ClearKind::Instant },
            ClearEvent { time_ms: 2000, kind: ClearKind::Instant },
        ];
        assert_eq!(page_index_for_time(&clears, 0), 0);
        assert_eq!(page_index_for_time(&clears, 1500), 1);
        assert_eq!(page_index_for_time(&clears, 2500), 2);
    }
}
