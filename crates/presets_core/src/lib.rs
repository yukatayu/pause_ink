use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportProfileSummary {
    pub id: String,
    pub display_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_holds_values() {
        let s = ExportProfileSummary {
            id: "youtube".into(),
            display_name: "YouTube".into(),
        };
        assert_eq!(s.id, "youtube");
        assert_eq!(s.display_name, "YouTube");
    }
}
