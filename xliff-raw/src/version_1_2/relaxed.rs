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
    #[serde(rename = "@xmlns")]
    pub xmlns: String,
    #[serde(rename = "@xmlns:xsi")]
    pub xmlns_xsi: String,
    #[serde(rename = "@xsi:schemaLocation")]
    pub xsi_schema_location: String,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct File {
    #[serde(rename = "@datatype")]
    pub datatype: String,
    #[serde(rename = "@source-language")]
    pub source_language: Language,
    #[serde(skip_serializing_if = "Option::is_none", rename = "@target-language")]
    pub target_language: Option<Language>,
    #[serde(rename = "@original")]
    pub original: String,
    // pub header: Option<Header>,
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
    #[serde(rename = "@datatype", skip)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    #[serde(rename = "@al-object-target", skip_serializing_if = "Option::is_none")]
    pub al_object_target: Option<String>,
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
    #[serde(rename = "@annotates")]
    pub annotates: Annotates,
    #[serde(rename = "@priority")]
    pub priority: Option<u8>,
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
