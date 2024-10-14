use rsnano_core::WorkNonce;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WorkDto {
    pub work: WorkNonce,
}

impl WorkDto {
    pub fn new(work: WorkNonce) -> Self {
        Self { work }
    }
}

#[cfg(test)]
mod tests {
    use super::WorkDto;

    #[test]
    fn serialize_work_get_dto() {
        let work = WorkDto::new(1.into());

        let expected_json = r#"{"work":"0000000000000001"}"#;
        let serialized = serde_json::to_string(&work).unwrap();

        assert_eq!(serialized, expected_json);
    }

    #[test]
    fn deserialize_work_get_dto() {
        let json_data = r#"{"work":"0000000000000001"}"#;
        let work: WorkDto = serde_json::from_str(json_data).unwrap();

        let expected_work = WorkDto::new(1.into());

        assert_eq!(work, expected_work);
    }
}
