use std::{
    borrow::Cow,
    str::FromStr,
    thread::{self, JoinHandle},
};

use anyhow::{Context as _, bail};
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
impl Auto {
    fn edit(&self, unique: AutoTranslationHandling) -> AutoTranslationHandling {
        if self.edit {
            AutoTranslationHandling::Ask
        } else {
            unique
        }
    }
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
    println!("Working on these files:");
    println!(" - {} [base]", translator.base_file().display());
    for file in translator.loaded_files() {
        println!(" - {}", file.display());
    }
    match args.command {
        Command::Untranslated(untranslated) => run_untranslated(translator, untranslated),
        Command::Unify(unify) => run_unify(translator, unify),
        Command::Auto(auto) => run_auto(translator, auto),
    }
}

#[derive(Debug)]
struct LazyTranslation {
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
            handle: Some(thread::spawn(move || {
                ai::english_to_german(&source)
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

    fn resolve(&mut self, translator: &mut Translator) -> &[StringId] {
        let Some(handle) = self.handle.take() else {
            return &self.results;
        };
        let result = handle.join().unwrap();
        if let Some(result) = result {
            self.results.push(translator.intern(result));
        }
        &self.results
    }

    fn combine(&mut self, mut other: LazyTranslation) {
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

struct Context {}
impl Context {
    fn new() -> Self {
        Self {}
    }
}

#[derive(Debug)]
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
        context: &Context,
    ) -> anyhow::Result<usize> {
        let mut translated = 0;
        let language_hint = (self.full_id.len() == 1).then(|| self.full_id[0].0);
        let is_silent = *decision == Decision::Accept || *decision == Decision::Deny;
        let to_id = self.to.resolve(translator);
        let from = translator.resolve(self.from);
        let count = self.full_id.len();
        if !is_silent {
            print_translation(translator, self.full_id[0].0, to_id, from, count);
        }
        let mut cur_decision = ask_for_decision(decision, count, false, to_id.len());
        match cur_decision {
            Decision::Accept => {
                *decision = cur_decision;
                let Some(selected) = ask_user_for_translation(translator, from, to_id)? else {
                    return Ok(translated);
                };
                for (lang, id) in &self.full_id {
                    translated += 1;
                    translator.add_translation(*lang, *id, selected)?;
                }
            }
            Decision::Yes => {
                let Some(selected) = ask_user_for_translation(translator, from, to_id)? else {
                    println!("No translation was added.");
                    return Ok(translated);
                };
                for (lang, id) in &self.full_id {
                    translated += 1;
                    translator.add_translation(*lang, *id, selected)?;
                }
            }
            Decision::Deny => {
                *decision = cur_decision;
            }
            Decision::No => {}
            Decision::Edit => {
                let mut langs: Vec<_> = self.full_id.iter().map(|f| f.0.to_string()).collect();
                langs.sort();
                langs.dedup();
                println!("Targeting {}:", langs.join(", "),);
                let selected = if to_id.len() > 1 {
                    todo!()
                } else {
                    to_id.get(0).copied().unwrap_or(empty_string_id())
                };
                loop {
                    let to = if selected == empty_string_id() {
                        translator.resolve(self.from)
                    } else {
                        translator.resolve(selected)
                    };
                    let Some(to) = inquire::Text::new("Enter translation:")
                        .with_initial_value(to)
                        .with_render_config(RenderConfig::default_colored().with_text_input(
                            StyleSheet::default().with_fg(inquire::ui::Color::DarkGreen),
                        ))
                        .prompt_skippable()?
                    else {
                        return self.decide(translator, decision, context);
                    };
                    let cur_decision = ask_for_decision(decision, count, true, to_id.len());
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
                        to: LazyTranslation::multiple(to_id.into()),
                    }
                    .decide(translator, &mut cur_decision, context)?;
                }
            }
            Decision::FindSource => {
                let related = translator
                    .find_in_translations(&regex::escape(from), language_hint)
                    .unwrap();
                found_related(translator, from, related);
                return self.decide(translator, decision, context);
            }
            Decision::Find(find) => {
                let related = match translator.find_in_translations(&find, language_hint) {
                    Ok(it) => it,
                    Err(err) => {
                        eprintln!("Failed parsing search term: {err}");
                        return self.decide(translator, decision, context);
                    }
                };
                found_related(translator, from, related);
                return self.decide(translator, decision, context);
            }
            Decision::Context => {
                let mut related: Vec<_> = self
                    .full_id
                    .iter()
                    .flat_map(|(lang, id)| translator.find_related(*lang, *id))
                    .filter(|r| (r.from != self.from) != (r.to != empty_string_id()))
                    .collect();
                related.sort_by_key(|r| r.from);
                related.dedup_by_key(|r| (r.from, r.to));
                let entry_count = self.full_id.len();
                found_context(translator, related, entry_count);
                return self.decide(translator, decision, context);
            }
        }
        Ok(translated)
    }

    fn integrate(
        &mut self,
        t: xliff_translation::MissingTranslation,
        recommended: LazyTranslation,
    ) {
        self.full_id.push((t.language, t.id));
        self.to.combine(recommended);
    }
}

fn ask_user_for_translation(
    translator: &Translator,
    from: &str,
    to_id: &[StringId],
) -> anyhow::Result<Option<StringId>> {
    Ok(if to_id.len() == 1 {
        Some(to_id[0])
    } else if to_id.is_empty() {
        None
    } else {
        let options = to_id.iter().map(|t| translator.resolve(*t)).collect();
        inquire::Select::new("There are multiple options availabe, choose one:", options)
            .prompt_skippable()?
            .map(|selection| to_id.iter().find(|t| translator.resolve(**t) == selection))
            .flatten()
            .copied()
    })
}

fn print_translation(
    translator: &Translator,
    lang: LanguageStr,
    to_id: &[StringId],
    from: &str,
    count: usize,
) {
    let count_txt = if count > 1 {
        count.to_string()
    } else {
        lang.to_string()
    };
    if to_id.len() == 0 {
        let text = [
            "'",
            from,
            "' => ",
            "\u{1b}[0;31m",
            "No translation available",
            "\u{1b}[0m",
        ]
        .concat();
        let text = textwrap::wrap(
            &text,
            textwrap::Options::with_termwidth()
                .initial_indent(&(["(", &count_txt, ") "].concat()))
                .subsequent_indent(&" ".repeat(count_txt.len() + 3))
                .break_words(false),
        );
        for text in text {
            println!("{text}");
        }
    } else if to_id.len() == 1 {
        let to = translator.resolve(to_id[0]);
        let text = ["'", from, "' => '", to, "'"].concat();
        let text = textwrap::wrap(
            &text,
            textwrap::Options::with_termwidth()
                .initial_indent(&(["(", &count_txt, ") "].concat()))
                .subsequent_indent(&" ".repeat(count_txt.len() + 3))
                .break_words(false),
        );
        for text in text {
            println!("{text}");
        }
    } else {
        let text = ["'", from, "'"].concat();
        let indent = count_txt.len() + 3;
        let indent_string = " ".repeat(indent);
        let indent_string_short = " ".repeat(indent - 3);
        let text = textwrap::wrap(
            &text,
            textwrap::Options::with_termwidth()
                .initial_indent(&(["(", &count_txt, ") "].concat()))
                .subsequent_indent(&indent_string)
                .break_words(false),
        );
        for text in text {
            println!("{text}");
        }
        for to in to_id {
            let to = translator.resolve(*to);
            let text = ["'", to, "'"].concat();
            let text = textwrap::wrap(
                &text,
                textwrap::Options::with_termwidth()
                    .initial_indent(&([&indent_string_short, "=> "].concat()))
                    .subsequent_indent(&indent_string)
                    .break_words(false),
            );
            for text in text {
                println!("{text}");
            }
        }
    }
}

fn found_context(
    translator: &mut Translator,
    related: Vec<xliff_translation::TranslationEntry>,
    entry_count: usize,
) {
    if related.is_empty() {
        println!("Could not provide any context.");
    } else {
        println!(
            "The following {} texts are probably close by in the ui:",
            related.len()
        );
        let count = related.len();
        for r in related {
            print_translation_entry_as_bullet_point(r, translator);
        }
        if count > 20 {
            println!("The previous {count} texts are probably close by in the ui.",);
            if entry_count > 1 {
                println!(
                    "Seems like you found a lot of related entries, [s]plitting may help to find more relevant entries to each single missing translation."
                )
            }
        }
    }
}

fn found_related(
    translator: &Translator,
    from: &str,
    related: Vec<xliff_translation::TranslationEntry>,
) {
    if related.is_empty() {
        println!("No matches found for {from}.");
    } else {
        println!(
            "The following {} matches have been found for '{from}':",
            related.len()
        );
        for r in related {
            let rfrom = translator.resolve(r.from);
            let rto = translator.resolve(r.to);
            println!("{} => {}", highlight(rfrom, &from), highlight(rto, &from),);
        }
    }
}

fn ask_for_decision(
    decision: &mut Decision,
    count: usize,
    small: bool,
    available_translations: usize,
) -> Decision {
    if small {
        loop {
            let result = inquire::CustomType::<Decision>::new("Add this translation? [y, n, e]")
                .prompt()
                .unwrap();
            if [Decision::Yes, Decision::No, Decision::Edit].contains(&result) {
                break result;
            }
            println!("Use [y] to accept the current translation.");
            println!("Use [n] to deny and skip the current translation.");
            println!("Use [e] to edit the current translation.");
        }
    } else {
        loop {
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
            if available_translations > 0 {
                println!("Use [y] to accept the current translation.");
            }
            println!("Use [n] to deny and skip the current translation.");
            println!("Use [a] to accept all translations.");
            println!("Use [d] to deny all translations.");
            println!("Use [e] to edit the current translation.");
            if count > 1 {
                println!(
                    "Use [s] to decide for each translation seperatly of the {} translations.",
                    count
                );
            }
            println!("Use [c] to show context with similar textes on the page/table/etc.");
            println!(
                "Use [f] to search in existing translations for a term, also supports regex (so you may want to escape characters). (e.g. f Accounting)"
            );
        }
    }
}

fn print_translation_entry_as_bullet_point(
    r: xliff_translation::TranslationEntry,
    translator: &mut Translator,
) {
    let rfrom = translator.resolve(r.from);
    let rto = translator.resolve(r.to);
    let text = if rto == "" {
        [
            "'",
            rfrom,
            "' => ",
            "\u{1b}[0;31m",
            "No translation available",
            "\u{1b}[0m",
        ]
        .concat()
    } else {
        ["'", rfrom, "' => '", rto, "'"].concat()
    };
    let text = textwrap::wrap(
        &text,
        textwrap::Options::with_termwidth()
            .initial_indent(" - ")
            .subsequent_indent("   ")
            .break_words(false),
    );
    for text in text {
        println!("{text}");
    }
}

// fn get_recommended(translator: &Translator, language: LanguageStr, source: StringId) -> Vec<StringId> {
//     todo!
// }

fn run_auto(mut translator: Translator, auto: Auto) -> Result<(), anyhow::Error> {
    let mut added_translations = 0;
    let mut skipped_translations = 0;
    let mut made_translations: Vec<UndecidedTranslation> = Vec::new();
    let german_languages: Vec<_> = translator.languages().filter(|l| is_german(*l)).collect();
    for t in translator.find_missing_translations() {
        let (source, _) =
            translator.get_source_and_translation(LanguageStr::try_from_str("g").unwrap(), t.id)?;
        let options = translator.get_translation(t.language, source);
        let (should_translate, mut recommended) = if options.len() == 0 {
            if is_german(t.language) {
                let all_options: Vec<_> = german_languages
                    .iter()
                    .flat_map(|l| translator.get_translation(*l, source))
                    .copied()
                    .collect();
                if all_options.is_empty() {
                    (
                        auto.ai,
                        LazyTranslation::maybe_translate(
                            translator.resolve(source),
                            !made_translations.iter().any(|t| t.from == source),
                        ),
                    )
                } else {
                    (
                        auto.multiple,
                        LazyTranslation::multiple(all_options.as_slice()),
                    )
                }
            } else {
                (auto.edit(auto.unique), LazyTranslation::instant(source))
            }
        } else if options.len() == 1 {
            (auto.unique, LazyTranslation::instant(options[0]))
        } else {
            (auto.multiple, LazyTranslation::multiple(options))
        };

        match should_translate {
            AutoTranslationHandling::Skip => {
                if auto.edit {
                    let recommended = LazyTranslation::multiple(&[]);
                    if let Some(existing) = made_translations
                        .iter()
                        .position(|u| u.from == source && u.is_german == is_german(t.language))
                    {
                        made_translations[existing].integrate(t, recommended);
                    } else {
                        made_translations.push(UndecidedTranslation {
                            full_id: vec![(t.language, t.id)],
                            from: source,
                            to: recommended,
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
                assert_eq!(recommended.len(), 1);
                translator.add_translation(t.language, t.id, recommended[0])?;
            }
            AutoTranslationHandling::Ask => {
                if let Some(existing) = made_translations
                    .iter()
                    .position(|u| u.from == source && u.is_german == is_german(t.language))
                {
                    made_translations[existing].integrate(t, recommended);
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
    let context = Context::new();
    for mut undecided in made_translations {
        added_translations += undecided.decide(&mut translator, &mut decision, &context)?;
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
