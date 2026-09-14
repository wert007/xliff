use std::str::FromStr;

use anyhow::{Context as _, bail};
use clap::{Parser, Subcommand};
use xliff_translation::{LanguageStr, Translator};

use crate::{
    interactive::{Decision, UndecidedTranslation},
    utils::LazyTranslation,
};

mod ai;
mod interactive;
mod utils;

#[derive(Debug, Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,
    #[clap(short, long)]
    /// Path were translation files are stored. By default
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
}

#[derive(Debug, Subcommand, Clone)]
enum Command {
    /// Test, that there are no untranslated entries left!
    Untranslated(Untranslated),
    /// Add translations, if there are some missing, has further options to select how translations are choosen.
    Add(Add),
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
struct Untranslated;

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
        Command::Add(auto) => run_auto(translator, auto),
    }
}

struct Context {}
impl Context {
    fn new() -> Self {
        Self {}
    }
}

fn run_auto(mut translator: Translator, auto: Add) -> Result<(), anyhow::Error> {
    let mut added_translations = 0;
    let mut skipped_translations = 0;
    let mut made_translations: Vec<UndecidedTranslation> = Vec::new();
    let german_languages: Vec<_> = translator.languages().filter(|l| is_german(*l)).collect();
    let missing_translations = translator.find_missing_translations();
    println!(
        "There are {} translations missing.",
        missing_translations.len()
    );
    for t in missing_translations {
        let (source, _) =
            translator.get_source_and_translation(LanguageStr::try_from_str("g").unwrap(), t.id)?;
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

fn is_german(language: LanguageStr) -> bool {
    language.starts_with("de")
}

fn run_untranslated(translator: Translator, Untranslated: Untranslated) -> anyhow::Result<()> {
    let missing = translator.find_missing_translations();
    if !missing.is_empty() {
        bail!("Found {} untranslated entries.", missing.len());
    } else {
        Ok(())
    }
}
