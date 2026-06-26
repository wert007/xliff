use crate::Language;

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Xliff {
    #[serde(rename = "@version")]
    version: String,
    #[serde(rename = "@srcLang")]
    src_lang: Language,
    #[serde(rename = "@trgLang")]
    trg_lang: Language,
    #[serde(rename = "file")]
    files: Vec<File>,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct File {
    #[serde(rename = "@id")]
    id: String,
    #[serde(rename = "@can_resegment")]
    can_resegment: Option<bool>,
    #[serde(rename = "@original")]
    original: Option<String>,
    #[serde(rename = "@translate")]
    translate: Option<bool>,
    #[serde(rename = "@srcDir")]
    src_dir: Option<String>,
    #[serde(rename = "@trgDir")]
    trg_dir: Option<String>,

    // Body
    skeleton: Option<Skeleton>,
    notes: Option<Notes>,
    #[serde(default)]
    unit: Vec<Unit>,
    #[serde(default)]
    group: Vec<Group>,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Unit {
    notes: Option<Notes>,
    original_data: Option<OriginalData>,
    segment: Vec<Segment>,
    // TODO: ignorable
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Group {
    notes: Option<Notes>,
    children: Vec<GroupOrUnit>,
    #[serde(rename = "@id")]
    id: String,
    #[serde(rename = "@name")]
    name: Option<String>,
    #[serde(rename = "@canResegment")]
    can_resegment: Option<bool>,
    #[serde(rename = "@translate")]
    translate: Option<bool>,
    #[serde(rename = "@srcDir")]
    src_dir: Option<Direction>,
    #[serde(rename = "@trgDir")]
    trg_dir: Option<Direction>,
    // TODO: type,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum GroupOrUnit {
    Group(Group),
    Unit(Unit),
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Segment {
    source: Source,
    target: Option<Target>,
    #[serde(rename = "@id")]
    id: Option<String>,
    #[serde(rename = "@canResegment")]
    can_resegment: Option<bool>,
    // TODO, State and SubState.
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Source {
    // TODO: Formatting?
    #[serde(rename = "$text")]
    #[serde(default)]
    text: String,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Target {
    #[serde(rename = "$text")]
    #[serde(default)]
    text: String,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct OriginalData {
    data: Vec<Data>,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Data {
    #[serde(rename = "@id")]
    id: String,
    #[serde(rename = "@dir")]
    dir: Direction,
    #[serde(rename = "$text")]
    #[serde(default)]
    text: Vec<StringOrUnicode>,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum Direction {
    #[serde(rename = "ltr")]
    LeftToRight,
    #[serde(rename = "rtl")]
    RightToLeft,
    #[serde(rename = "auto")]
    Auto,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum StringOrUnicode {
    String(String),
    Unicode(Cp),
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Cp {
    #[serde(rename = "@hex")]
    hex: String,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Skeleton {
    #[serde(rename = "@href")]
    href: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Notes {
    #[serde(rename = "note")]
    notes: Vec<Note>,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Note {
    #[serde(rename = "$text")]
    #[serde(default)]
    text: String,
    #[serde(rename = "@id")]
    id: Option<String>,
    #[serde(rename = "@appliesTo")]
    applies_to: Option<AppliesTo>,
    #[serde(rename = "@category")]
    category: Option<String>,
    #[serde(rename = "@priority")]
    priority: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum AppliesTo {
    #[serde(rename = "target")]
    Target,
    #[serde(rename = "source")]
    Source,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_example() {
        let text = r#"
        <xliff xmlns="urn:oasis:names:tc:xliff:document:2.2" version="2.2"
    srcLang="en" trgLang="fr">
  <file id="f1">
    <notes>
      <note id="n1">note for file.</note>
    </notes>
    <unit id="u1">
      <my:elem xmlns:my="myNamespaceURI" id="x1">data</my:elem>
      <notes>
        <note id="n1">note for unit</note>
      </notes>
      <segment id="s1">
        <source>Hello World!</source>
        <target>Bonjour le Monde!</target>
      </segment>
    </unit>
  </file>
</xliff>
        "#;
        let _xliff: Xliff = quick_xml::de::from_str(text).unwrap();
    }

    #[test]
    fn wikipedia_example() {
        let text = r#"
        <xliff xmlns="urn:oasis:names:tc:xliff:document:2.0" version="2.0"
 srcLang="en-US" trgLang="ja-JP">
 <file id="f1" original="Graphic Example.psd">
  <skeleton href="Graphic Example.psd.skl"/>
  <unit id="1">
   <segment>
    <source>Quetzal</source>
    <target>Quetzal</target>
   </segment>
  </unit>
  <unit id="2">
   <segment>
    <source>An application to manipulate and process XLIFF documents</source>
    <target>XLIFF 文書を編集、または処理 するアプリケーションです。</target>
   </segment>
  </unit>
  <unit id="3">
   <segment>
    <source>XLIFF Data Manager</source>
    <target>XLIFF データ・マネージャ</target>
   </segment>
  </unit>
 </file>
</xliff>
        "#;
        let _xliff: Xliff = quick_xml::de::from_str(text).unwrap();
    }
}
