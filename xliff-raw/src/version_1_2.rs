use crate::Language;

#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum XliffBoolean {
    #[serde(rename = "no")]
    No,
    #[serde(rename = "yes")]
    Yes,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smartproject() {
        let text = r#"
        <?xml version="1.0" encoding="utf-8"?>
<xliff xmlns="urn:oasis:names:tc:xliff:document:1.2" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="1.2" xsi:schemaLocation="urn:oasis:names:tc:xliff:document:1.2 xliff-core-1.2-transitional.xsd">
  <file datatype="xml" source-language="en-US" target-language="de-DE" original="smartPROJECT">
    <body>
      <group id="body">
        <trans-unit id="Codeunit 716193044 - Method 1925602947 - NamedType 672599587" size-unit="char" translate="yes" xml:space="preserve">
          <source>Error!</source>
          <target>Fehler!</target>
          <note from="Developer" annotates="general" priority="2"></note>
          <note from="Xliff Generator" annotates="general" priority="3">Codeunit PMS Assembly Schedule Mgmt. - Method NewAsmWorkStepNo - NamedType Txt001</note>
        </trans-unit>
        </group>
        </body>
        </file>
        </xliff>"#;
        let _xliff: Xliff = quick_xml::de::from_str(text).unwrap();
    }

    #[test]
    fn big_file1() {
        let _xliff: Xliff =
            quick_xml::de::from_str(include_str!("../../examples/smartPROJECT.g.xlf")).unwrap();
    }

    #[test]
    fn big_file2() {
        let _xliff: Xliff =
            quick_xml::de::from_str(include_str!("../../examples/smartPROJECT.de-de.xlf")).unwrap();
    }
}
