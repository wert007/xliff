use std::{borrow::Cow, thread::JoinHandle};

use regex::RegexBuilder;
use xliff_translation::{StringId, Translator, empty_string_id};

#[derive(Debug)]
pub struct LazyTranslation {
    handle: Option<JoinHandle<Option<String>>>,
    results: Vec<StringId>,
}

impl LazyTranslation {
    pub fn maybe_translate(source: &str, translation_needed: bool) -> Self {
        if !translation_needed {
            return Self::multiple(&[]);
        }
        let source = source.to_owned();
        Self {
            handle: Some(std::thread::spawn(move || {
                crate::ai::english_to_german(&source)
                    .inspect_err(|e| _ = dbg!(e))
                    .ok()
            })),
            results: Vec::new(),
        }
    }

    pub fn instant(translation: StringId) -> Self {
        Self {
            handle: None,
            results: vec![translation],
        }
    }

    pub fn multiple(translations: &[StringId]) -> Self {
        Self {
            handle: None,
            results: translations
                .iter()
                .copied()
                .filter(|s| *s != empty_string_id())
                .collect(),
        }
    }

    pub fn resolve(&mut self, translator: &mut Translator) -> &[StringId] {
        let Some(handle) = self.handle.take() else {
            return &self.results;
        };
        let result = handle.join().unwrap();
        if let Some(result) = result {
            self.results.push(translator.intern(result));
        }
        &self.results
    }

    pub fn combine(&mut self, mut other: LazyTranslation) {
        self.results.append(&mut other.results);
        self.results.sort();
        self.results.dedup();
        match other.handle {
            None => {}
            Some(handle) => {
                self.handle = Some(handle);
            }
        }
    }
}

pub fn highlight<'a>(base: &'a str, highlight: &str) -> Cow<'a, str> {
    if highlight == "" {
        return base.into();
    }
    let r = RegexBuilder::new(&highlight)
        .case_insensitive(true)
        .build()
        .unwrap();
    if !r.is_match(base) {
        return base.into();
    }
    let mut base2 = base.to_owned();
    let ranges: Vec<_> = r.find_iter(base).map(|f| f.range()).collect();
    for m in ranges.into_iter().rev() {
        base2.insert_str(m.end, "\x1b[39m");
        base2.insert_str(m.start, "\u{1b}[36m");
    }
    base2.into()
}
