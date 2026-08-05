use crate::Language;

#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum XliffBoolean {
    #[serde(rename = "no")]
    No,
    #[serde(rename = "yes")]
    Yes,
    #[serde(rename = "")]
    Empty,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Xliff {
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "file")]
    pub files: Vec<File>,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct File {
    #[serde(rename = "@original")]
    pub original: String,
    #[serde(rename = "@source-language")]
    pub source_language: Language,
    #[serde(rename = "@datatype")]
    pub datatype: String,
    pub header: Option<Header>,
    pub body: Body,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Header;

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Body {
    #[serde(rename = "$value")]
    pub elements: Vec<BodyElement>,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum BodyElement {
    #[serde(rename = "group")]
    Group(Group),
    #[serde(rename = "trans-unit")]
    TransUnit(TransUnit),
    // BinUnit(BinUnit),
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Group {
    #[serde(rename = "@id")]
    pub id: Option<String>,
    #[serde(rename = "@datatype")]
    pub datatype: Option<String>,
    #[serde(rename = "$value")]
    pub elements: Vec<GroupElement>,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum GroupElement {
    // ContextGroup(ContextGroup),
    // CountGroup(CountGroup),
    // PropGroup(PropGroup),
    #[serde(rename = "note")]
    Note(Note),
    #[serde(rename = "group")]
    Group(Group),
    #[serde(rename = "trans-unit")]
    TransUnit(TransUnit),
    // BinUnit(BinUnit),
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct TransUnit {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@size-unit")]
    pub size_unit: Option<String>,
    #[serde(rename = "@translate")]
    pub translate: Option<XliffBoolean>,
    #[serde(rename = "@xml:space")]
    pub xml_space: WhitespacePreservation,
    pub source: Source,
    pub target: Option<Target>,
    #[serde(default)]
    pub note: Vec<Note>,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Source {
    #[serde(rename = "$text")]
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Target {
    #[serde(rename = "$text")]
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Note {
    #[serde(rename = "@from")]
    pub from: Option<String>,
    #[serde(rename = "@priority")]
    pub priority: Option<u8>,
    #[serde(rename = "@annotates")]
    pub annotates: Annotates,
    #[serde(rename = "$text")]
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum Annotates {
    #[serde(rename = "general")]
    General,
    #[serde(rename = "source")]
    Source,
    #[serde(rename = "target")]
    Target,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum WhitespacePreservation {
    #[serde(rename = "default")]
    Default,
    #[serde(rename = "preserve")]
    Preserve,
}
