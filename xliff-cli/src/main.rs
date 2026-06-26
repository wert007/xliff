use anyhow::{Context, bail};
use clap::{Parser, Subcommand};
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

#[derive(Debug, Parser, Clone, Copy, PartialEq, Eq, Default)]
struct Auto;

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

fn generate_translation(translator: &mut Translator, source: StringId) -> StringId {
    let english = translator.resolve(source);
    ai::english_to_german(english)
        .inspect_err(|e| {
            dbg!(e);
        })
        .map(|s| translator.intern(s))
        .unwrap_or(empty_string_id())
}

fn run_auto(mut translator: Translator, auto: Auto) -> Result<(), anyhow::Error> {
    for t in translator.find_missing_translations() {
        let (source, target) =
            translator.get_source_and_translation(LanguageStr::try_from_str("g").unwrap(), t.id)?;
        let recommended = translator
            .get_translation(t.language, source)
            .first()
            .copied()
            .unwrap_or_else(|| generate_translation(&mut translator, source));

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
    Ok(())
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
            let (source, target) = translator
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
