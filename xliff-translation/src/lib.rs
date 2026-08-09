use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
    fmt::Display,
    fs::File,
    io::{BufReader, BufWriter},
    path::PathBuf,
};

use lasso::{Key, Rodeo, Spur};
use xliff_raw::version_1_2::relaxed::{
    BodyElement, Group, GroupElement, Source, Target, TransUnit, Xliff,
};

pub type StringId = lasso::Spur;

mod error;
pub use error::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LanguageStr(tinystr::TinyAsciiStr<8>);

impl Display for LanguageStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl LanguageStr {
    pub fn starts_with(&self, s: &str) -> bool {
        self.0.starts_with(s)
    }
    pub fn try_from_str(s: &str) -> std::result::Result<Self, tinystr::ParseError> {
        Ok(Self(tinystr::TinyAsciiStr::try_from_str(s)?))
    }

    fn is_base(&self) -> bool {
        self.0 == "g"
    }
}

pub type Result<T> = std::result::Result<T, error::Error>;

// #[derive(Debug)]
// pub struct TranslationUnit {
//     target_lang: LanguageStr,
//     id: u32,
//     source: String,
//     target: Option<String>,
// }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MissingTranslation {
    pub language: LanguageStr,
    pub id: Spur,
}

impl MissingTranslation {
    fn new(language: LanguageStr, id: Spur) -> Self {
        Self { language, id }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FastIndex {
    file_index: usize,
    trans_unit_index: usize,
}

pub struct TranslationEntry {
    pub id: StringId,
    pub from: StringId,
    pub to: StringId,
}
impl TranslationEntry {
    fn from_tuple(id: Spur, (from, to): (Spur, Option<Spur>)) -> TranslationEntry {
        Self {
            id,
            from,
            to: to.unwrap(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TranslationFile {
    path: PathBuf,
    raw: Xliff,
    translated_ids: HashSet<lasso::Spur>,
    ids_with_same_translation: HashSet<lasso::Spur>,
    ids_to_source_and_translation: HashMap<lasso::Spur, (Spur, Option<Spur>)>,
    source_to_translation: HashMap<lasso::Spur, Vec<lasso::Spur>>,
    ids: HashMap<lasso::Spur, FastIndex>,
}

fn for_each_trans_unit(raw: &Xliff, mut cb: impl FnMut(&TransUnit, usize, usize)) {
    let mut trans_unit_index;
    let mut file_index = 0;
    for file in &raw.files {
        trans_unit_index = 0;
        for element in &file.body.elements {
            match element {
                BodyElement::Group(group) => {
                    for_each_trans_unit_in_group(group, file_index, &mut trans_unit_index, &mut cb)
                }
                BodyElement::TransUnit(trans_unit) => {
                    cb(trans_unit, file_index, trans_unit_index);
                    trans_unit_index += 1;
                }
            }
        }
        file_index += 1;
    }
}

fn for_each_trans_unit_in_group(
    group: &Group,
    file_index: usize,
    trans_unit_index: &mut usize,
    cb: &mut impl FnMut(&TransUnit, usize, usize),
) {
    for element in &group.elements {
        match element {
            GroupElement::Note(_) => {}
            GroupElement::Group(group) => {
                for_each_trans_unit_in_group(group, file_index, trans_unit_index, cb)
            }
            GroupElement::TransUnit(trans_unit) => {
                cb(trans_unit, file_index, *trans_unit_index);
                *trans_unit_index += 1;
            }
        }
    }
}

impl TranslationFile {
    fn new(file: &PathBuf, is_base: bool, string_interner: &mut Rodeo) -> Result<Self> {
        let raw: xliff_raw::version_1_2::relaxed::Xliff =
            quick_xml::de::from_reader(BufReader::new(File::open(file)?))?;
        let mut ids = HashMap::new();
        let mut translated_ids = HashSet::new();
        let mut ids_with_same_translation = HashSet::new();
        let mut ids_to_source_and_translation = HashMap::new();
        let mut source_to_translation: HashMap<Spur, Vec<Spur>> = HashMap::new();

        // let mut file_index = 0;
        // let mut trans_unit_index;
        for_each_trans_unit(&raw, |trans_unit, file_index, trans_unit_index| {
            let id = string_interner.get_or_intern(&trans_unit.id);
            let source = string_interner.get_or_intern(trans_unit.source.text.clone());
            let target = trans_unit
                .target
                .as_ref()
                .map(|t| string_interner.get_or_intern(t.text.clone()));
            // assert_eq!(string_interner.resolve(&id), &trans_unit.id);
            ids_to_source_and_translation.insert(id, (source, target));
            if source != empty_string_id()
                && (is_base || target.is_some_and(|t| t != empty_string_id()))
            {
                translated_ids.insert(id);
            }
            if !is_base && source != empty_string_id() && target == Some(source) {
                ids_with_same_translation.insert(id);
            }
            if !is_base
                && source != empty_string_id()
                && target.is_some_and(|t| t != empty_string_id())
            {
                let entries = source_to_translation.entry(source).or_default();
                if !entries.contains(&target.unwrap()) {
                    entries.push(target.unwrap());
                }
            }

            ids.insert(
                id,
                FastIndex {
                    file_index,
                    trans_unit_index,
                },
            );
        });
        Ok(Self {
            path: file.clone(),
            raw,
            source_to_translation,
            ids_to_source_and_translation,
            ids,
            translated_ids,
            ids_with_same_translation,
        })
    }

    fn add_translation(
        &mut self,
        id: Spur,
        source: Spur,
        translation: Spur,
        translation_str: String,
        interner: &mut Rodeo,
    ) {
        if let Some(index) = self.ids.get(&id) {
            self.change_translation(id, *index, translation, translation_str);
        } else {
            let BodyElement::Group(group) = &mut self.raw.files[0].body.elements[0] else {
                todo!("This is kind of a soft error, but also a hard error. hmm.")
            };
            let id = interner.resolve(&id).into();
            let source = interner.resolve(&source).into();
            group.elements.push(GroupElement::TransUnit(TransUnit {
                id,
                size_unit: None,
                translate: None,
                xml_space: xliff_raw::version_1_2::relaxed::WhitespacePreservation::Default,
                source: Source { text: source },
                target: Some(Target {
                    text: translation_str,
                }),
                note: Vec::new(),
            }));
        }
    }

    fn get_source_and_translation(&self, id: Spur) -> (Spur, Option<Spur>) {
        self.ids_to_source_and_translation
            .get(&id)
            .copied()
            .expect("This apparently cannot fail")
    }

    fn search_for_source_regex(&self, needle: &regex::Regex, string_interner: &Rodeo) -> Vec<Spur> {
        let mut result = Vec::new();
        for_each_trans_unit(&self.raw, |t, _, _| {
            if needle.is_match(&t.source.text) {
                result.push(string_interner.get(&t.id).unwrap());
            }
        });
        result
    }

    fn search_for_target_regex(&self, needle: &regex::Regex, string_interner: &Rodeo) -> Vec<Spur> {
        let mut result = Vec::new();
        for_each_trans_unit(&self.raw, |t, _, _| {
            if t.target.as_ref().is_some_and(|t| needle.is_match(&t.text)) {
                result.push(string_interner.get(&t.id).unwrap());
            }
        });
        result
    }

    fn save_file(self) -> Result<()> {
        use serde::Serialize;
        pub(crate) struct ToFmtWrite<T>(pub T);

        impl<T> std::fmt::Write for ToFmtWrite<T>
        where
            T: std::io::Write,
        {
            fn write_str(&mut self, s: &str) -> std::fmt::Result {
                self.0.write_all(s.as_bytes()).map_err(|_| std::fmt::Error)
            }
        }

        let writer = BufWriter::new(File::create(self.path)?);
        let mut writer = ToFmtWrite(writer);
        let mut se = quick_xml::se::Serializer::with_root(&mut writer, Some("xliff"))?;
        se.expand_empty_elements(true)
            .indent(' ', 4)
            .set_quote_level(quick_xml::se::QuoteLevel::Full);
        self.raw.serialize(se)?;
        Ok(())
    }

    fn get_translations(&self, source: Spur) -> &Vec<Spur> {
        static EMPTY_VEC: Vec<Spur> = Vec::new();
        self.source_to_translation
            .get(&source)
            .unwrap_or(&EMPTY_VEC)
    }

    fn change_translation(
        &mut self,
        id: Spur,
        index: FastIndex,
        translation: Spur,
        translation_str: String,
    ) {
        self.ids_to_source_and_translation
            .get_mut(&id)
            .expect("This should exist")
            .1 = Some(translation);
        let BodyElement::Group(group) = &mut self.raw.files[index.file_index].body.elements[0]
        else {
            todo!("This is kind of a soft error, but also a hard error. hmm.")
        };
        let GroupElement::TransUnit(trans_unit) = &mut group.elements[index.trans_unit_index]
        else {
            todo!("This is kind of a soft error, but also a hard error. hmm.")
        };
        trans_unit.target = Some(Target {
            text: translation_str,
        });
    }

    fn find_related(&self, entry: Spur, string_interner: &Rodeo) -> Vec<TranslationEntry> {
        let mut result = Vec::new();
        let base_type = string_interner
            .resolve(&entry)
            .split('-')
            .next()
            .unwrap()
            .trim();
        let ids = get_all_ids_starting_with(base_type, string_interner);
        for id in ids {
            let Some(tuple) = self.ids_to_source_and_translation.get(&id).copied() else {
                continue;
            };
            if tuple.1.is_some() {
                result.push(TranslationEntry::from_tuple(id, tuple));
            }
        }
        result
    }
}

fn get_all_ids_starting_with(pat: &str, string_interner: &Rodeo) -> Vec<Spur> {
    string_interner
        .iter()
        .filter(|(_, s)| s.starts_with(pat))
        .map(|(i, _)| i)
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Translator {
    string_interner: Rodeo,
    base: TranslationFile,
    languages: HashMap<LanguageStr, TranslationFile>,
}

const EMPTY_STRING_ID: usize = 0;
pub fn empty_string_id() -> Spur {
    Spur::try_from_usize(EMPTY_STRING_ID).unwrap()
}

impl Translator {
    pub fn new(files: &[PathBuf]) -> Result<Self> {
        let mut base = None;
        let mut languages = HashMap::new();
        let mut rodeo = Rodeo::new();
        let empty: Spur = rodeo.get_or_intern_static("");
        assert_eq!(empty.into_inner().get() as usize, EMPTY_STRING_ID + 1);
        for file in files {
            let file_name = file
                .file_name()
                .ok_or(error::Error::InvalidFilePath(file.to_path_buf()))?
                .to_str()
                .ok_or(error::Error::OnlyUtf8EncodedFilePathsAllowed(
                    file.to_path_buf(),
                ))?;
            let file_stem = file_name
                .strip_suffix(".xlf")
                .ok_or(error::Error::UnsupportedFileNameFormat(file.to_path_buf()))?;
            let (_file_name, language_code) = file_stem
                .rsplit_once('.')
                .ok_or(error::Error::UnsupportedFileNameFormat(file.to_path_buf()))?;
            let language_code = LanguageStr::try_from_str(language_code)?;
            let it = TranslationFile::new(file, language_code.is_base(), &mut rodeo)?;
            if language_code.is_base() {
                base = Some(it);
            } else {
                languages.insert(language_code, it);
            }
        }
        Ok(Self {
            base: base.ok_or(error::Error::NoBaseTranslationFound)?,
            languages,
            string_interner: rodeo,
        })
    }

    pub fn find_missing_translations(&self) -> Vec<MissingTranslation> {
        let mut missing_ids = Vec::new();
        for (lang_code, lang_file) in &self.languages {
            missing_ids.extend(
                self.base
                    .translated_ids
                    .difference(&lang_file.translated_ids)
                    .map(|id| MissingTranslation::new(*lang_code, *id)),
            );
        }
        missing_ids
    }

    pub fn find_untranslated_entries(&self) -> Vec<MissingTranslation> {
        let mut missing_ids = Vec::new();
        for (lang_code, lang_file) in &self.languages {
            missing_ids.extend(
                lang_file
                    .ids_with_same_translation
                    .iter()
                    .map(|id| MissingTranslation::new(*lang_code, *id)),
            );
        }
        missing_ids
    }

    pub fn add_translation(
        &mut self,
        language: LanguageStr,
        id: Spur,
        translation: Spur,
    ) -> Result<()> {
        let translation_str = self.resolve(translation).to_owned();
        let (s, _) = self.base.ids_to_source_and_translation.get(&id).unwrap();
        self.languages
            .get_mut(&language)
            .ok_or(error::Error::LanguageNotFound(language))?
            .add_translation(
                id,
                *s,
                translation,
                translation_str,
                &mut self.string_interner,
            );
        Ok(())
    }

    pub fn get_source_and_translation(
        &self,
        language: LanguageStr,
        id: Spur,
    ) -> Result<(Spur, Option<Spur>)> {
        if language.is_base() {
            return Ok(self.base.get_source_and_translation(id));
        }
        Ok(self
            .languages
            .get(&language)
            .ok_or(error::Error::LanguageNotFound(language))?
            .get_source_and_translation(id))
    }

    pub fn find_regex_match(&self, needle: impl Borrow<regex::Regex>) -> Vec<MissingTranslation> {
        let needle = needle.borrow();
        let mut result = Vec::new();
        for (lang_code, lang_file) in &self.languages {
            result.extend(
                lang_file
                    .search_for_source_regex(needle, &self.string_interner)
                    .into_iter()
                    .map(|id| MissingTranslation::new(*lang_code, id)),
            );
            result.extend(
                lang_file
                    .search_for_target_regex(needle, &self.string_interner)
                    .into_iter()
                    .map(|id| MissingTranslation::new(*lang_code, id)),
            );
        }
        result
    }

    pub fn save_files(self) -> Result<()> {
        for (_, lang_file) in self.languages {
            lang_file.save_file()?;
        }
        Ok(())
    }

    pub fn intern(&mut self, s: String) -> Spur {
        self.string_interner.get_or_intern(s)
    }

    pub fn resolve(&self, id: Spur) -> &str {
        self.string_interner.resolve(&id)
    }

    pub fn get_translation(&self, language: LanguageStr, source: Spur) -> &Vec<Spur> {
        static EMPTY_VEC: Vec<Spur> = Vec::new();
        self.languages
            .get(&language)
            .map(|l| l.get_translations(source))
            .unwrap_or(&EMPTY_VEC)
    }

    pub fn languages(&self) -> impl Iterator<Item = LanguageStr> {
        self.languages.keys().copied()
    }

    pub fn get_sources(&self, language: LanguageStr) -> Vec<Spur> {
        self.languages[&language]
            .source_to_translation
            .keys()
            .copied()
            .collect()
    }

    pub fn find_related(&self, language: LanguageStr, entry: Spur) -> Vec<TranslationEntry> {
        self.languages[&language].find_related(entry, &self.string_interner)
    }
}
