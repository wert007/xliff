use std::borrow::Cow;

use clap::{Parser, builder::styling::AnsiColor};
use inquire::ui::{Attributes, RenderConfig, Styled};
use regex::{Regex, RegexBuilder};
use xliff_translation::{LanguageStr, TranslationEntry, Translator};

use crate::interactive::print_translation_entry_as_bullet_point;

#[derive(Debug, Parser, Clone, Copy, PartialEq, Eq, Default)]
pub struct Edit {
    #[clap(short, long)]
    regex: bool,
    #[clap(short, long)]
    case_sensitive: bool,
}

fn command_render_config() -> RenderConfig<'static> {
    let mut result = RenderConfig::default_colored().with_prompt_prefix(Styled::new("> "));
    result.placeholder = result
        .placeholder
        .with_fg(inquire::ui::Color::Grey)
        .with_attr(Attributes::ITALIC);
    result
}

pub fn run_edit(
    mut translator: Translator,
    Edit {
        mut regex,
        mut case_sensitive,
    }: Edit,
) -> Result<(), anyhow::Error> {
    println!(
        "There are {} entries in total",
        translator.total_translation_count()
    );
    let render_config = command_render_config();
    let mut currently_searched = Vec::new();
    let mut unsaved_changes = 0;
    let mut lang_filter = "";
    loop {
        let cmd = inquire::Text::new("")
            .with_placeholder("[Search mode] Confused? Type help!")
            .with_render_config(render_config)
            .prompt()?;
        let cmd_parts: Vec<_> = cmd.trim().split(' ').filter(|s| !s.is_empty()).collect();
        match cmd_parts.as_slice() {
            [] => {
                println!("Nothing was entered. Try again.");
            }
            ["h" | "?" | "help"] => {
                println!("Current lang set to '{lang_filter}'.");
                print_regex_state(regex);
                println!("l|lang\t\t\tReset the language filter.");
                println!("regex\t\t\tToggle regex usage in search command.");
                println!("l|lang\t\ten|de\tSet current language filter to english/german.");
                println!("e|edit\t\tall|*\tEdit all entries from your previous search");
                println!(
                    "r|replace|/\t\t<from/to>\tStart replacement process. For more details type help replace."
                );
                println!("e|edit\t\t<id>\tEdit a single entry from your previous search");
                println!("f|find\t\t\tShow results from previous search again");
                println!("f|find\t\t<text>\tSearch existing translations");
                println!("q|quit|^C\t\tQuit editing");
                println!("h|?|help\t\tThis help message");
            }
            ["h" | "?" | "help", "r" | "replace" | "/"] => {
                println!("Replace");
                println!();
                println!("Examples");
                println!("  replace the/a");
                println!("  /tough/though");
                println!("  r \\w+/\\w");
                println!("  r Name of \\W/$1");
                println!(
                    "After executing this command, you can check out the values that would be changed or make the changes directly. You can also decide for each entry individually if you want to change it."
                )
            }
            ["regex"] => {
                regex = !regex;
                print_regex_state(regex);
            }
            ["q" | "quit"] => {
                break;
            }
            ["f" | "find"] => {
                for (id, m) in currently_searched.iter().enumerate() {
                    print_translation_entry_as_bullet_point(
                        *m,
                        &translator,
                        "",
                        &format!("{id:02}: "),
                    );
                }
            }
            ["l" | "lang" | "language", "de" | "en"] | ["l" | "lang" | "language"] => {
                lang_filter = cmd_parts
                    .get(1)
                    .copied()
                    .unwrap_or_default()
                    .to_string()
                    .leak();
            }
            ["l" | "lang" | "language", ..] => {
                println!(
                    "Failed setting language filter. Either use 'de', 'en' or no argument at all."
                );
            }
            ["r" | "replace" | "/", ..] => {
                let remaining = cmd
                    .strip_prefix("/")
                    .or(cmd.strip_prefix("replace"))
                    .or(cmd.strip_prefix("r"))
                    .unwrap();
                let Some((from, to)) = remaining.split_once('/') else {
                    println!("Usage: replace old/new");
                    continue;
                };
                let changed = replace(
                    from.trim(),
                    to.trim(),
                    lang_filter,
                    regex,
                    !case_sensitive,
                    &mut translator,
                )?;
                if changed > 0 {
                    unsaved_changes += changed;
                    println!("Replaced in {changed} translations.");
                } else {
                    print!("No changes made.");
                }
            }
            ["f" | "find", ..] => {
                if let Some(text) = cmd.strip_prefix("find") {
                    currently_searched = search(text.trim(), lang_filter, regex, &translator)?;
                } else {
                    currently_searched = search(&cmd[2..].trim(), lang_filter, regex, &translator)?;
                }
            }
            ["e" | "edit", n] => {
                if let Ok(index) = n.parse::<usize>() {
                    if index > currently_searched.len() {
                        println!(
                            "No entry with index {index} found. There are only {} entries in current search.",
                            currently_searched.len()
                        );
                    } else {
                        let edited = edit(&mut currently_searched[index], &mut translator)?;
                        if edited {
                            unsaved_changes += 1;
                        }
                    }
                } else if ["all", "*"].contains(n) {
                    for entry in &mut currently_searched {
                        if edit(entry, &mut translator)? {
                            unsaved_changes += 1;
                        }
                    }
                } else {
                    println!("Usage: `e 123` to edit 123th entry.");
                }
            }
            _ => {
                currently_searched = search(&cmd.trim(), lang_filter, regex, &translator)?;
            }
        }
    }
    if unsaved_changes > 0 {
        let should_save = inquire::Confirm::new(&format!(
            "There are {unsaved_changes} unsaved changes, do you wanna save them?"
        ))
        .with_default(true)
        .prompt()?;
        if should_save {
            translator.save_files()?;
            println!("Changes were saved.");
        } else {
            println!("Changes were discarded.");
        }
    }
    Ok(())
}

fn replace(
    from: &str,
    to: &str,
    lang: &str,
    regex: bool,
    case_insensitive: bool,
    translator: &mut Translator,
) -> anyhow::Result<usize> {
    let escaped = regex::escape(from);
    let find = if regex { from } else { &escaped };
    let matches = translator.find_in_translations(
        find,
        (!lang.is_empty()).then_some(LanguageStr::try_from_str(lang)?),
        true,
    )?;
    println!("Found {} matches.", matches.len());
    const ALL: &'static str = "Make all changes";
    const DECIDE: &'static str = "Decide for each changes";
    let selection = inquire::Select::new(
        "What you wanna do?",
        vec!["Show results", ALL, DECIDE, "Cancel"],
    )
    .prompt()?;

    match selection {
        "Show results" => {
            for m in matches.iter() {
                print_translation_entry_as_bullet_point(*m, &translator, from, "");
            }
            let selection =
                inquire::Select::new("What you wanna do?", vec![ALL, DECIDE, "Cancel"]).prompt()?;
            match selection {
                ALL => replace_all(to, regex, translator, find, matches),
                DECIDE => replace_decide(find, to, matches, regex, case_insensitive, translator),
                _ => Ok(0),
            }
        }
        DECIDE => replace_decide(find, to, matches, regex, case_insensitive, translator),
        ALL => replace_all(to, regex, translator, find, matches),
        _ => Ok(0),
    }
}

fn replace_decide(
    find: &str,
    to: &str,
    mut matches: Vec<TranslationEntry>,
    regex: bool,
    case_insensitive: bool,
    translator: &mut Translator,
) -> Result<usize, anyhow::Error> {
    let mut changes = 0;
    for entry in matches.iter_mut() {
        let translation = translator.resolve(entry.to);
        let new_translation = if regex {
            let Cow::Owned(new_translation) = regex::RegexBuilder::new(find)
                .case_insensitive(case_insensitive)
                .build()?
                .replace_all(translation, to)
            else {
                continue;
            };
            new_translation
        } else {
            let Cow::Owned(new_translation) = regex::RegexBuilder::new(find)
                .case_insensitive(case_insensitive)
                .build()?
                .replace_all(translation, regex::NoExpand(to))
            else {
                continue;
            };
            new_translation
        };
        // Since regex does not guarantee, that nothing changed
        if new_translation == translation {
            continue;
        }
        print_replacement(translation, find, to, regex, case_insensitive)?;
        let spur = translator.intern(new_translation);
        loop {
            let decision =
                inquire::Text::new(&format!("Do you wanna replace {find} with {to}? [y, n, q]"))
                    .prompt()?;
            match decision.as_str() {
                "y" | "yes" | "j" | "ja" => {
                    entry.to = spur;
                    translator.add_translation(entry.lang, entry.id, spur)?;
                    changes += 1;
                    break;
                }
                "n" | "no" | "nein" => {
                    break;
                }
                "q" | "quit" | "c" | "cancel" => {
                    return Ok(changes);
                }
                _ => {}
            }
        }
    }
    Ok(changes)
}

fn print_replacement(
    base: &str,
    original: &str,
    replacement: &str,
    regex: bool,
    case_insensitive: bool,
) -> anyhow::Result<()> {
    let mut printed_start = false;
    let mut end = 0usize;
    for m in regex::RegexBuilder::new(original)
        .case_insensitive(case_insensitive)
        .build()?
        .find_iter(base)
    {
        if !printed_start {
            print!("{}", &base[..m.start()]);
            printed_start = true;
        }
        end = m.end();
        print!(
            "{}{}{}",
            oxink::styles::BG_RED.open_escape(),
            m.as_str(),
            oxink::styles::BG_RED.close_escape()
        );
        print!(
            "{}{}{}",
            oxink::styles::BG_GREEN.open_escape(),
            if regex {
                Regex::new(original)?.replace(base, replacement)
            } else {
                replacement.into()
            },
            oxink::styles::BG_GREEN.close_escape(),
        )
    }
    println!("{}", &base[end..]);
    Ok(())
}

fn replace_all(
    to: &str,
    regex: bool,
    translator: &mut Translator,
    find: &str,
    mut matches: Vec<TranslationEntry>,
) -> Result<usize, anyhow::Error> {
    let mut changes = 0;
    for entry in matches.iter_mut() {
        let translation = translator.resolve(entry.to);
        let new_translation = if regex {
            let Cow::Owned(new_translation) = regex::RegexBuilder::new(find)
                .build()?
                .replace_all(translation, to)
            else {
                continue;
            };
            new_translation
        } else {
            let Cow::Owned(new_translation) = regex::RegexBuilder::new(find)
                .build()?
                .replace_all(translation, regex::NoExpand(to))
            else {
                continue;
            };
            new_translation
        };
        // Since regex does not guarantee, that nothing changed
        if new_translation == translation {
            continue;
        }
        let spur = translator.intern(new_translation);
        entry.to = spur;
        translator.add_translation(entry.lang, entry.id, spur)?;
        changes += 1;
    }
    Ok(changes)
}

fn print_regex_state(regex: bool) {
    if regex {
        println!("Regex is enabled now.")
    } else {
        println!("Regex is disabled now.")
    }
}

fn edit(entry: &mut TranslationEntry, translator: &mut Translator) -> anyhow::Result<bool> {
    let source = translator.resolve(entry.from);
    let translation = translator.resolve(entry.to);
    println!("Lang: {}", entry.lang);
    println!("Source: {source}");
    let new_translation = inquire::Text::new("Translation:")
        .with_initial_value(translation)
        .prompt()?;
    if new_translation == translation {
        return Ok(false);
    }
    let spur = translator.intern(new_translation);
    entry.to = spur;
    translator.add_translation(entry.lang, entry.id, spur)?;
    Ok(true)
}

fn search(
    needle: &str,
    lang: &str,
    regex: bool,
    translator: &Translator,
) -> anyhow::Result<Vec<TranslationEntry>> {
    let escaped = regex::escape(needle);
    let matches = translator.find_in_translations(
        if regex { needle } else { &escaped },
        (!lang.is_empty()).then_some(LanguageStr::try_from_str(lang)?),
        true,
    )?;
    for (id, m) in matches.iter().enumerate() {
        print_translation_entry_as_bullet_point(*m, &translator, needle, &format!("{id:02}: "));
    }
    Ok(matches)
}

// pub fn print_translation_entry_as_bullet_point_edit<const N: usize>(
//     r: xliff_translation::TranslationEntry,
//     translator: &Translator,
//     mark: [&str; N],
// ) {
//     let rfrom = translator.resolve(r.from);
//     let rto = translator.resolve(r.to);
//     let text = if rto.is_empty() {
//         [
//             "'",
//             &highlight(rfrom, mark),
//             "' => ",
//             "\u{1b}[0;31m",
//             "No translation available",
//             "\u{1b}[0m",
//         ]
//         .concat()
//     } else {
//         [
//             prefix,
//             "'",
//             &highlight(rfrom, mark),
//             "' => '",
//             &highlight(rto, mark),
//             "'",
//         ]
//         .concat()
//     };
//     let text = textwrap::wrap(
//         &text,
//         textwrap::Options::with_termwidth()
//             .initial_indent(" - ")
//             .subsequent_indent("   ")
//             .break_words(false),
//     );
//     for text in text {
//         println!("{text}");
//     }
// }
