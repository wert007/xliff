pub mod version_1_2;
pub mod version_2;

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Language(String);
