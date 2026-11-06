use std::str::FromStr;

use anyhow::{Context as _, bail};
use clap::{Parser, Subcommand};
use xliff_translation::{LanguageStr, MissingTranslation, Translator};

use crate::{
    interactive::{ChangedSourceDecision, Decision, UndecidedTranslation},
    utils::LazyTranslation,
};

mod ai;
mod api;
mod edit;
mod interactive;
mod utils;

#[derive(Debug, Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,
    #[clap(short, long)]
    /// Path were translation files are stored. By default the current
    /// directory, and all its descendants.
    directory: Option<String>,
}

impl Args {
    pub fn create_translator(&self) -> anyhow::Result<xliff_translation::Translator> {
        let mut files = Vec::new();
        for file in glob::glob(&format!(
            "{}/*.xlf",
            self.directory
                .as_ref()
                .map(|s| s.as_str())
                .unwrap_or("**")
                .trim_end_matches('/')
        ))
        .context("Failed resolving glob")?
        {
            let file = file.context("Path could not get resolved")?;
            files.push(file);
        }
        let translator: Translator =
            Translator::new(&files).context("Failed creating translator")?;
        Ok(translator)
    }

    pub fn claims_stdout(&self) -> bool {
        matches!(&self.command, Command::Api(_))
    }
}

#[derive(Debug, Subcommand, Clone)]
enum Command {
    /// Test, that there are no untranslated entries left!
    Check(Check),
    /// Add translations, if there are some missing, has further options to select how translations are choosen.
    Add(Add),
    Edit(edit::Edit),
    /// The api interface, which allows to make chances via multiple calls to xliff-cli.
    Api(api::Api),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, strum::Display, strum::AsRefStr)]
#[strum(serialize_all = "kebab-case")]
enum AutoTranslationHandling {
    #[default]
    Skip,
    Translate,
    Ask,
}

impl FromStr for AutoTranslationHandling {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "s" | "skip" => Self::Skip,
            "a" | "ask" => Self::Ask,
            "t" | "translate" => Self::Translate,
            unknown => {
                return Err(format!(
                    "Unknown variant {unknown}, use skip, translate or ask instead!"
                ));
            }
        })
    }
}

#[derive(Debug, Parser, Clone, Copy, PartialEq, Eq, Default)]
/// Add translations.
struct Add {
    /// Automatically accept matches from existing translations, where there is
    /// only one possibility. Valid values are [skip, translate, ask].
    #[clap(short = 'u', long, default_value = "t")]
    unique: Option<AutoTranslationHandling>,
    /// Automatically accept matches from existing translations, where there are
    /// multiple possibilities. Whichone will be choosen is not specified. Valid
    /// values are [skip, translate, ask].
    #[clap(short, long)]
    multiple: Option<AutoTranslationHandling>,
    /// Automatically accept ai translations. Valid values are [skip, translate,
    /// ask].
    #[clap(short, long)]
    ai: Option<AutoTranslationHandling>,
    /// Gives you the chance to manually translate skipped entries, if you
    /// enabled.
    #[clap(short, long)]
    edit: bool,
}

impl Add {
    fn ai(&self) -> AutoTranslationHandling {
        if self.edit {
            AutoTranslationHandling::Ask
        } else if let Some(it) = self.ai {
            it
        } else {
            AutoTranslationHandling::Skip
        }
    }

    fn multiple(&self) -> AutoTranslationHandling {
        if self.edit {
            AutoTranslationHandling::Ask
        } else if let Some(it) = self.multiple {
            it
        } else {
            AutoTranslationHandling::Skip
        }
    }

    fn unique(&self) -> AutoTranslationHandling {
        if self.edit {
            AutoTranslationHandling::Ask
        } else if let Some(it) = self.unique {
            it
        } else {
            AutoTranslationHandling::Translate
        }
    }
}

#[derive(Debug, Parser, Clone, Copy, PartialEq, Eq, Default)]
struct Check;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let translator = args.create_translator()?;
    if !args.claims_stdout() {
        println!("Working on these files:");
        println!(" - {} [base]", translator.base_file().display());
        for file in translator.loaded_files() {
            println!(" - {}", file.display());
        }
    }
    match args.command {
        Command::Check(check) => run_check(translator, check),
        Command::Add(auto) => run_add(translator, auto),
        Command::Edit(edit) => edit::run_edit(translator, edit),
        Command::Api(api) => api.run(translator),
    }
}

fn run_add(mut translator: Translator, add: Add) -> Result<(), anyhow::Error> {
    let mismatched_sources = translator.find_changed_source_text();
    let mut missing_translations = translator.find_missing_translations();
    if !mismatched_sources.is_empty() {
        println!(
            "The source text of {} translations changed.",
            mismatched_sources.len()
        );
        if handle_changed_source_texts(
            mismatched_sources,
            &mut translator,
            &mut missing_translations,
        )? {
            println!(
                "\u{1b}[0;31mProgram was stopped. Cancelling execution, no translation was saved.\u{1b}[0m",
            );
            return Ok(());
        }
    }
    println!(
        "There are {} translations missing.",
        missing_translations.len()
    );
    let (mut added_translations, skipped_translations, mut made_translations) =
        handle_missing_translations(&mut translator, add, missing_translations)?;
    if skipped_translations > 0 {
        eprintln!(
            "Could not auto translate {skipped_translations} translations. Enable --edit flag to manually translate entries or accept more things automatically."
        );
    }
    // made_translations.append(&mut mismatched_sources_translations);
    made_translations.sort_by_key(|t| t.from);
    let mut decision = Decision::No;
    for mut undecided in made_translations {
        added_translations += undecided.decide(&mut translator, &mut decision, &add)?;
        if decision == Decision::HardExit {
            println!(
                "\u{1b}[0;31mProgram was stopped. Cancelling execution, no translation was saved.\u{1b}[0m",
            );
            return Ok(());
        }
    }
    println!("Added {} translations.", added_translations);

    translator.save_files()?;
    Ok(())
}

fn handle_changed_source_texts(
    mismatched_sources: Vec<xliff_translation::MissingTranslation>,
    translator: &mut Translator,
    missing_translations: &mut Vec<xliff_translation::MissingTranslation>,
) -> anyhow::Result<bool> {
    let mut skipped = 0;
    let mut result = Vec::new();

    let mut decision = ChangedSourceDecision::Ignore;
    for mismatched in mismatched_sources {
        if decision == ChangedSourceDecision::HardExit {
            return Ok(true);
        }
        let Some((old_source, old_translation)) =
            translator.get_source_and_translation(mismatched.language, mismatched.id)?
        else {
            todo!("How Reachable?");
        };
        let Some(old_translation) = old_translation else {
            skipped += 1;
            continue;
        };
        let to = LazyTranslation::instant(old_translation);
        let is_german = is_german(mismatched.language);
        if result.last().is_some_and(|l: &UndecidedTranslation| {
            l.from == old_source && l.is_german == is_german
        }) {
            let last = result.last_mut().unwrap();
            last.full_id.push((mismatched.language, mismatched.id));
            last.to.combine(to);
        } else {
            if let Some(last) = result.last_mut() {
                handle_changed_source_single(
                    translator,
                    &mut decision,
                    last,
                    missing_translations,
                )?;
            }
            result.push(UndecidedTranslation {
                to,
                full_id: vec![(mismatched.language, mismatched.id)],
                from: old_source,
                is_german,
            });
        }
    }
    if let Some(last) = result.last_mut() {
        handle_changed_source_single(translator, &mut decision, last, missing_translations)?;
    }
    if skipped > 0 {
        println!(
            "Skipped {skipped} entries, since they did not have a translation. Translations will be added next."
        );
    }
    Ok(false)
}

fn handle_changed_source_single(
    translator: &mut Translator,
    decision: &mut ChangedSourceDecision,
    last: &mut UndecidedTranslation,
    missing_translations: &mut Vec<xliff_translation::MissingTranslation>,
) -> anyhow::Result<()> {
    let id = last.full_id.first().map(|i| i.1).unwrap();
    let from = translator.resolve(last.from);
    let to = translator.resolve(
        translator
            .get_source_and_translation(LanguageStr::base(), id)?
            .map(|b| b.0)
            .unwrap(),
    );
    let diffs = dissimilar::diff(from, to);
    print!("(source) ");
    for diff in diffs {
        match diff {
            dissimilar::Chunk::Equal(it) => print!("{it}"),
            dissimilar::Chunk::Delete(del) => print!(
                "{}{del}{}",
                oxink::styles::BG_RED.open_escape(),
                oxink::styles::BG_RED.close_escape()
            ),
            dissimilar::Chunk::Insert(ins) => {
                print!(
                    "{}{ins}{}",
                    oxink::styles::BG_GREEN.open_escape(),
                    oxink::styles::BG_GREEN.close_escape()
                )
            }
        }
    }
    println!();
    let to = *last.to.resolve(translator).first().unwrap();
    println!(" => {}", translator.resolve(to));

    loop {
        let cur_decision = interactive::ask_for_changed_source_decision(decision);
        match cur_decision {
            ChangedSourceDecision::Help => ChangedSourceDecision::print_help(),
            ChangedSourceDecision::KeepTranslation | ChangedSourceDecision::DeleteTranslation => {
                let keep = cur_decision == ChangedSourceDecision::KeepTranslation;

                let (new_source, _) = translator
                    .get_source_and_translation(
                        LanguageStr::base(),
                        last.full_id.first().unwrap().1,
                    )?
                    .unwrap();
                for (lang, id) in &last.full_id {
                    translator.change_source(*lang, *id, new_source)?;
                }
                if !keep {
                    for (language, id) in last.full_id.iter().copied() {
                        missing_translations.push(MissingTranslation { language, id });
                    }
                }
                break;
            }
            ChangedSourceDecision::Ignore => break,
            ChangedSourceDecision::IgnoreAll | ChangedSourceDecision::HardExit => {
                *decision = cur_decision;
                break;
            }
        }
    }
    Ok(())
}

fn handle_missing_translations(
    translator: &mut Translator,
    auto: Add,
    missing_translations: Vec<xliff_translation::MissingTranslation>,
) -> Result<(usize, usize, Vec<UndecidedTranslation>), anyhow::Error> {
    let german_languages: Vec<_> = translator.languages().filter(|l| is_german(*l)).collect();
    let mut added_translations = 0;
    let mut skipped_translations = 0;
    let mut made_translations: Vec<UndecidedTranslation> = Vec::new();
    for t in missing_translations {
        let (source, _) = translator
            .get_source_and_translation(LanguageStr::try_from_str("g").unwrap(), t.id)?
            .expect("Base file has to have translation");
        let options = translator.get_translation(t.language, source);
        let (should_translate, mut recommended) = if options.is_empty() {
            if is_german(t.language) {
                let all_options: Vec<_> = german_languages
                    .iter()
                    .flat_map(|l| translator.get_translation(*l, source))
                    .copied()
                    .collect();
                if all_options.is_empty() {
                    (
                        auto.ai(),
                        LazyTranslation::maybe_translate(
                            translator.resolve(source),
                            !made_translations.iter().any(|t| t.from == source),
                        ),
                    )
                } else {
                    (
                        auto.multiple(),
                        LazyTranslation::multiple(all_options.as_slice()),
                    )
                }
            } else {
                (auto.unique(), LazyTranslation::instant(source))
            }
        } else if options.len() == 1 {
            (auto.unique(), LazyTranslation::instant(options[0]))
        } else {
            (auto.multiple(), LazyTranslation::multiple(options))
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
                let recommended = recommended.resolve(translator);
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
    Ok((added_translations, skipped_translations, made_translations))
}

fn is_german(language: LanguageStr) -> bool {
    language.starts_with("de")
}

fn run_check(translator: Translator, Check: Check) -> anyhow::Result<()> {
    let missing = translator.find_missing_translations();
    if !missing.is_empty() {
        bail!("Found {} untranslated entries.", missing.len());
    }
    let changed = translator.find_changed_source_text();
    if !changed.is_empty() {
        bail!(
            "Found {} entries, with outdated source text.",
            changed.len()
        );
    }
    Ok(())
}
