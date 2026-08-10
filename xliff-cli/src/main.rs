use std::{
    borrow::Cow,
    str::FromStr,
    thread::{self, JoinHandle},
};

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};
use inquire::ui::{RenderConfig, StyleSheet};
use regex::RegexBuilder;
use xliff_translation::{LanguageStr, StringId, Translator, empty_string_id};

mod ai;

#[derive(Debug, Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

impl Args {
    pub fn create_translator(&self) -> anyhow::Result<xliff_translation::Translator> {
        let mut files = Vec::new();
        for file in glob::glob("**/*.xlf").context("Failed resolving glob")? {
            let file = file.context("Path could not get resolved")?;
            files.push(file);
        }
        let translator: Translator =
            Translator::new(&files).context("Failed creating translator")?;
        Ok(translator)
    }
}

#[derive(Debug, Subcommand, Clone)]
enum Command {
    Untranslated(Untranslated),
    Unify(Unify),
    Auto(Auto),
}

#[derive(
    Debug,
    Default,
    strum::FromRepr,
    Clone,
    Copy,
    PartialEq,
    Eq,
    strum::Display,
    strum::AsRefStr,
    strum::EnumString,
)]
#[strum(serialize_all = "kebab-case")]
enum AutoTranslationHandling {
    #[default]
    Skip,
    Translate,
    Ask,
}

#[derive(Debug, Parser, Clone, Copy, PartialEq, Eq, Default)]
struct Auto {
    /// Automatically accept matches from existing translations, where there is
    /// only one possibility. Valid values are [skip, translate, ask].
    #[clap(short = 'u', long, default_value = "translate")]
    unique: AutoTranslationHandling,
    /// Automatically accept matches from existing translations, where there are
    /// multiple possibilities. Whichone will be choosen is not specified. Valid
    /// values are [skip, translate, ask].
    #[clap(short, long)]
    multiple: AutoTranslationHandling,
    /// Automatically accept ai translations. Valid values are [skip, translate,
    /// ask].
    #[clap(short, long)]
    ai: AutoTranslationHandling,
    /// Gives you the chance to manually translate skipped entries, if you
    /// enabled.
    #[clap(short, long)]
    edit: bool,
}

#[derive(Debug, Parser, Clone, Copy, PartialEq, Eq, Default)]
struct Unify;

#[derive(Debug, Parser, Clone, Copy, PartialEq, Eq, Default)]
struct Untranslated {
    /// Allows the user to fix the untranslated entries.
    #[clap(short, long)]
    fix: bool,
    /// Use existing translations for auto translations (so no AI here).
    #[clap(short, long)]
    auto: bool,
    /// Error if there are any untranslated entries. Useful for CI.
    #[clap(short, long)]
    error: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let translator = args.create_translator()?;
    match args.command {
        Command::Untranslated(untranslated) => run_untranslated(translator, untranslated),
        Command::Unify(unify) => run_unify(translator, unify),
        Command::Auto(auto) => run_auto(translator, auto),
    }
}

#[derive(Debug)]
struct LazyTranslation {
    handle: Option<JoinHandle<Option<String>>>,
    result: StringId,
}

impl LazyTranslation {
    pub fn maybe_translate(source: &str, translation_needed: bool) -> Self {
        if !translation_needed {
            return Self::instant(empty_string_id());
        }
        let source = source.to_owned();
        Self {
            handle: Some(thread::spawn(move || {
                ai::english_to_german(&source)
                    .inspect_err(|e| _ = dbg!(e))
                    .ok()
            })),
            result: empty_string_id(),
        }
    }

    pub fn instant(translation: StringId) -> Self {
        Self {
            handle: None,
            result: translation,
        }
    }

    fn resolve(&mut self, translator: &mut Translator) -> StringId {
        let Some(handle) = self.handle.take() else {
            return self.result;
        };
        let result = handle.join().unwrap();
        if let Some(result) = result {
            self.result = translator.intern(result);
        }
        self.result
    }
}

fn highlight<'a>(base: &'a str, highlight: &str) -> Cow<'a, str> {
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

struct UndecidedTranslation {
    full_id: Vec<(LanguageStr, StringId)>,
    from: StringId,
    is_german: bool,
    to: LazyTranslation,
}

#[derive(Debug, Clone, strum::Display, PartialEq, Eq)]
enum Decision {
    Accept,
    Yes,
    Deny,
    No,
    Edit,
    Split,
    Context,
    FindSource,
    Find(String),
    Help,
}

impl FromStr for Decision {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "a" => Self::Accept,
            "y" => Self::Yes,
            "d" => Self::Deny,
            "n" => Self::No,
            "e" => Self::Edit,
            "s" => Self::Split,
            "c" => Self::Context,
            "f" => Self::FindSource,
            "h" | "help" | "?" => Self::Help,
            f => {
                if let Some(term) = f.strip_prefix("f ") {
                    Self::Find(term.into())
                } else {
                    return Err(());
                }
            }
        })
    }
}

impl UndecidedTranslation {
    fn decide<'a>(
        &mut self,
        translator: &'a mut Translator,
        decision: &mut Decision,
    ) -> anyhow::Result<usize> {
        let mut translated = 0;
        let language_hint = (self.full_id.len() == 1).then(|| self.full_id[0].0);
        let is_silent = *decision == Decision::Accept || *decision == Decision::Deny;
        let to_id = self.to.resolve(translator);
        let from = translator.resolve(self.from);
        let to = translator.resolve(to_id);
        let count = self.full_id.len();
        if !is_silent {
            let multiline = from.len() > 80 || to.len() > 80;
            let count_txt = if count > 1 {
                count.to_string()
            } else {
                self.full_id[0].0.to_string()
            };
            if multiline {
                println!("({count_txt}) '{from}'\n    => '{to}'");
            } else {
                println!("({count_txt}) '{from}' => '{to}'");
            }
        }
        let mut cur_decision = loop {
            if *decision == Decision::Accept || *decision == Decision::Deny {
                break decision.clone();
            }
            let result = inquire::CustomType::<Decision>::new(
                "Add this translation? [y, n, a, d, e, s, c, f, ?]",
            )
            .prompt()
            .unwrap();
            if result != Decision::Help || (count <= 1 && result == Decision::Split) {
                break result;
            }
            println!("Use [y] to accept the current translation.");
            println!("Use [n] to deny and skip the current translation.");
            println!("Use [a] to accept all translations.");
            println!("Use [d] to deny all translations.");
            println!("Use [e] to edit the current translation.");
            if count > 1 {
                println!("Use [s] to seperate the {} translations.", count);
            }
            println!("Use [c] to show context with similar textes on the page/table/etc.");
            println!(
                "Use [f] to search in existing translations for a term, also supports regex (so you may want to escape characters). (e.g. f Accounting)"
            );
        };
        match cur_decision {
            Decision::Accept => {
                *decision = cur_decision;
                for (lang, id) in &self.full_id {
                    translated += 1;
                    translator.add_translation(*lang, *id, to_id)?;
                }
            }
            Decision::Yes => {
                for (lang, id) in &self.full_id {
                    translated += 1;
                    translator.add_translation(*lang, *id, to_id)?;
                }
            }
            Decision::Deny => {
                *decision = cur_decision;
            }
            Decision::No => {}
            Decision::Edit => {
                println!(
                    "Targeting {}:",
                    self.full_id
                        .iter()
                        .map(|f| format!("{}, ", f.0))
                        .collect::<String>()
                );
                loop {
                    let to = translator.resolve(to_id);
                    let Some(to) = inquire::Text::new("Enter translation:")
                        .with_initial_value(to)
                        .with_render_config(RenderConfig::default_colored().with_text_input(
                            StyleSheet::default().with_fg(inquire::ui::Color::DarkGreen),
                        ))
                        .prompt_skippable()?
                    else {
                        return self.decide(translator, decision);
                    };
                    let cur_decision = loop {
                        let result =
                            inquire::CustomType::<Decision>::new("Add this translation? [y, n, e]")
                                .prompt()
                                .unwrap();
                        if [Decision::Yes, Decision::No, Decision::Edit].contains(&result) {
                            break result;
                        }
                        println!("Use [y] to accept the current translation.");
                        println!("Use [n] to deny and skip the current translation.");
                        println!("Use [e] to edit the current translation.");
                    };
                    let to_id = translator.intern(to);
                    match cur_decision {
                        Decision::Yes => {
                            for (lang, id) in &self.full_id {
                                translated += 1;
                                translator.add_translation(*lang, *id, to_id)?;
                            }
                        }
                        Decision::No => {}
                        Decision::Edit => continue,
                        _ => unreachable!(),
                    }
                    break;
                }
            }
            Decision::Help => unreachable!(),
            Decision::Split => {
                for id in &self.full_id {
                    UndecidedTranslation {
                        full_id: vec![*id],
                        from: self.from,
                        is_german: self.is_german,
                        to: LazyTranslation::instant(to_id),
                    }
                    .decide(translator, &mut cur_decision)?;
                }
            }
            Decision::FindSource => {
                let related = translator.find_in_translations(from, language_hint);
                if related.is_empty() {
                    println!("No matches found for {from}.");
                } else {
                    println!(
                        "The following {} matches have been found for '{from}':",
                        related.len()
                    );
                    for r in related {
                        println!(
                            "{} => {}",
                            highlight(translator.resolve(r.from), &from),
                            highlight(translator.resolve(r.to), &from),
                        );
                    }
                }
                return self.decide(translator, decision);
            }
            Decision::Find(find) => {
                let related = translator.find_in_translations(&find, language_hint);
                if related.is_empty() {
                    println!("No matches found for {find}.");
                } else {
                    println!(
                        "The following {} matches have been found for '{find}':",
                        related.len()
                    );
                    for r in related {
                        println!(
                            "{} => {}",
                            highlight(translator.resolve(r.from), &find),
                            highlight(translator.resolve(r.to), &find),
                        );
                    }
                }
                return self.decide(translator, decision);
            }
            Decision::Context => {
                let related = translator.find_related(self.full_id[0].0, self.full_id[0].1);
                if related.is_empty() {
                    println!("Could not provide any context.");
                } else {
                    println!(
                        "The following {} texts are probably close by in the ui:",
                        related.len()
                    );
                    for r in related {
                        println!(
                            "{} => {}",
                            translator.resolve(r.from),
                            translator.resolve(r.to)
                        );
                    }
                }
                return self.decide(translator, decision);
            }
        }
        Ok(translated)
    }
}

fn run_auto(mut translator: Translator, auto: Auto) -> Result<(), anyhow::Error> {
    let mut added_translations = 0;
    let mut skipped_translations = 0;
    let mut made_translations: Vec<UndecidedTranslation> = Vec::new();
    for t in translator.find_missing_translations() {
        let (source, _) =
            translator.get_source_and_translation(LanguageStr::try_from_str("g").unwrap(), t.id)?;
        let options = translator.get_translation(t.language, source);
        let (should_translate, mut recommended) = if options.len() == 0 {
            if is_german(t.language) {
                (
                    auto.ai,
                    LazyTranslation::maybe_translate(
                        translator.resolve(source),
                        !made_translations.iter().any(|t| t.from == source),
                    ),
                )
            } else {
                (auto.ai, LazyTranslation::instant(source))
            }
        } else if options.len() == 1 {
            (auto.unique, LazyTranslation::instant(options[0]))
        } else {
            (auto.multiple, LazyTranslation::instant(options[0]))
        };

        match should_translate {
            AutoTranslationHandling::Skip => {
                if auto.edit {
                    if let Some(existing) = made_translations
                        .iter()
                        .position(|u| u.from == source && u.is_german == is_german(t.language))
                    {
                        made_translations[existing].full_id.push((t.language, t.id));
                    } else {
                        made_translations.push(UndecidedTranslation {
                            full_id: vec![(t.language, t.id)],
                            from: source,
                            to: LazyTranslation::instant(empty_string_id()),
                            is_german: is_german(t.language),
                        });
                    }
                } else {
                    skipped_translations += 1;
                }
            }
            AutoTranslationHandling::Translate => {
                added_translations += 1;
                let recommended = recommended.resolve(&mut translator);
                translator.add_translation(t.language, t.id, recommended)?;
            }
            AutoTranslationHandling::Ask => {
                if let Some(existing) = made_translations
                    .iter()
                    .position(|u| u.from == source && u.is_german == is_german(t.language))
                {
                    made_translations[existing].full_id.push((t.language, t.id));
                } else {
                    made_translations.push(UndecidedTranslation {
                        full_id: vec![(t.language, t.id)],
                        from: source,
                        to: recommended,
                        is_german: is_german(t.language),
                    });
                }
            }
        }
    }
    if skipped_translations > 0 {
        eprintln!(
            "Could not auto translate {skipped_translations} translations. Enable --edit flag to manually translate entries or accept more things automatically."
        );
    }
    made_translations.sort_by_key(|t| t.from);
    let mut decision = Decision::No;
    for mut undecided in made_translations {
        added_translations += undecided.decide(&mut translator, &mut decision)?;
    }
    println!("Added {} translations.", added_translations);
    translator.save_files()?;
    Ok(())
}

fn is_german(language: LanguageStr) -> bool {
    language.starts_with("de")
}

fn run_unify(translator: Translator, Unify: Unify) -> Result<(), anyhow::Error> {
    for language in translator.languages() {
        // TODO: Sort and dedup?
        let sources = translator.get_sources(language);
        for source in sources {
            let result = translator.get_translation(language, source);
            if result.len() <= 1 {
                continue;
            }
            eprintln!(
                "{} has {} options in language {language}.",
                translator.resolve(source),
                result.len()
            );
            for option in result {
                eprintln!(" - {}", translator.resolve(*option));
            }
        }
    }
    Ok(())
}

fn run_untranslated(mut translator: Translator, untranslated: Untranslated) -> anyhow::Result<()> {
    if untranslated == Untranslated::default() {
        bail!("Specify at least one option of --error or --fix");
    }
    let missing = translator.find_missing_translations();
    if untranslated.error && !missing.is_empty() {
        bail!("Found {} untranslated entries.", missing.len());
    }
    if untranslated.fix {
        for t in translator.find_missing_translations() {
            let (source, _) = translator
                .get_source_and_translation(LanguageStr::try_from_str("g").unwrap(), t.id)?;
            let recommended = translator
                .get_translation(t.language, source)
                .first()
                .copied()
                .unwrap_or_else(|| empty_string_id());

            print_bubble(translator.resolve(source), translator.resolve(t.id));
            let Some(translation) = inquire::Text::new("Add translation:")
                .with_default(translator.resolve(recommended))
                .prompt_skippable()?
            else {
                continue;
            };
            let translation = translator.intern(translation);
            if recommended != empty_string_id()
                && translation != recommended
                && inquire::Confirm::new("Should all other values also be overwritten?").prompt()?
            {
                // Do something smart here!
            }
            translator.add_translation(t.language, t.id, translation)?;
        }
        translator.save_files()?;
    }
    Ok(())
}

fn print_bubble(content: &str, source: &str) {
    let mut table = comfy_table::Table::new();
    let bubble = table
        .add_row([content])
        .load_preset(comfy_table::presets::UTF8_FULL);
    println!("{bubble}");
    println!(" \\");
    println!("  {source}");
}
