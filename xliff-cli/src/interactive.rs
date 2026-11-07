use std::str::FromStr;

use xliff_translation::{LanguageStr, StringId, Translator, empty_string_id};

use crate::{
    Add,
    utils::{LazyTranslation, highlight},
};

#[derive(Debug)]
pub struct UndecidedTranslation {
    pub full_id: Vec<(LanguageStr, StringId)>,
    pub from: StringId,
    pub is_german: bool,
    pub to: LazyTranslation,
}

#[derive(Debug, Clone, strum::Display, PartialEq, Eq)]
pub enum Decision {
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
    Languages,
    HardExit,
}

impl std::str::FromStr for Decision {
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
            "l" => Self::Languages,
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

pub struct DecisionTakerContext<'a> {
    translated: &'a mut usize,
    translator: &'a mut Translator,
    overarching_decision: &'a mut Decision,
}

pub enum DecisionControlFlow {
    Repeat,
    Continue,
    Exit,
}

impl DecisionTaker for Add {
    fn decision_made(
        &self,
        decision: Decision,
        undecided: &mut UndecidedTranslation,
        ctx: &mut DecisionTakerContext<'_>,
    ) -> anyhow::Result<DecisionControlFlow> {
        let language_hint = (undecided.full_id.len() == 1).then(|| undecided.full_id[0].0);
        let to_id = undecided.to.resolve(ctx.translator);
        let from = ctx.translator.resolve(undecided.from);
        match &decision {
            Decision::Yes | Decision::Accept => {
                if decision == Decision::Accept {
                    *ctx.overarching_decision = decision.clone();
                }
                let Some(selected) = ask_user_for_translation(ctx.translator, from, to_id)? else {
                    if decision == Decision::Yes {
                        println!("No translation was added.");
                    }
                    return Ok(DecisionControlFlow::Continue);
                };
                for (lang, id) in &undecided.full_id {
                    *ctx.translated += 1;
                    ctx.translator.add_translation(*lang, *id, selected)?;
                }
            }
            Decision::Deny => {
                *ctx.overarching_decision = decision;
            }
            Decision::No => {}
            Decision::Languages => {
                let mut langs: Vec<_> = undecided.full_id.iter().map(|f| f.0.to_string()).collect();
                langs.sort();
                langs.dedup();
                println!("Targeting these languages: {}", langs.join(", "));
                return Ok(DecisionControlFlow::Repeat);
            }
            Decision::Edit => {
                let mut langs: Vec<_> = undecided.full_id.iter().map(|f| f.0.to_string()).collect();
                langs.sort();
                langs.dedup();
                println!("Targeting {}:", langs.join(", "),);
                let selected = if to_id.len() > 1 {
                    todo!()
                } else {
                    to_id.first().copied().unwrap_or(empty_string_id())
                };
                loop {
                    let to = if selected == empty_string_id() {
                        ctx.translator.resolve(undecided.from)
                    } else {
                        ctx.translator.resolve(selected)
                    };
                    let Some(to) = inquire::Text::new("Enter translation:")
                        .with_initial_value(to)
                        .with_render_config(
                            inquire::ui::RenderConfig::default_colored().with_text_input(
                                inquire::ui::StyleSheet::default()
                                    .with_fg(inquire::ui::Color::DarkGreen),
                            ),
                        )
                        .prompt_skippable()?
                    else {
                        return Ok(DecisionControlFlow::Repeat);
                    };
                    let cur_decision = ask_for_decision(
                        ctx.overarching_decision, // Actually unused here.. hehe.
                        undecided.full_id.len(),
                        true,
                        to_id.len(),
                    );
                    let to_id = ctx.translator.intern(to);
                    match cur_decision {
                        Decision::Yes => {
                            for (lang, id) in &undecided.full_id {
                                *ctx.translated += 1;
                                ctx.translator.add_translation(*lang, *id, to_id)?;
                            }
                        }
                        Decision::No => {}
                        Decision::Edit => continue,
                        _ => unreachable!(),
                    }
                    break;
                }
            }
            Decision::Split => {
                for id in &undecided.full_id {
                    UndecidedTranslation {
                        full_id: vec![*id],
                        from: undecided.from,
                        is_german: undecided.is_german,
                        to: LazyTranslation::multiple(to_id),
                    }
                    .decide(ctx.translator, ctx.overarching_decision, self)?;
                }
            }
            Decision::Context => {
                let mut related: Vec<_> = undecided
                    .full_id
                    .iter()
                    .flat_map(|(lang, id)| ctx.translator.find_related(*lang, *id))
                    .filter(|r| (r.from != undecided.from) != (r.to != empty_string_id()))
                    .collect();
                related.sort_by_key(|r| r.from);
                related.dedup_by_key(|r| (r.from, r.to));
                let entry_count = undecided.full_id.len();
                found_context(ctx.translator, related, entry_count);
                return Ok(DecisionControlFlow::Repeat);
            }
            Decision::FindSource => {
                let related = ctx
                    .translator
                    .find_in_translations(&regex::escape(from), language_hint, false)
                    .unwrap();
                found_related(ctx.translator, from, related);
                return Ok(DecisionControlFlow::Repeat);
            }
            Decision::Find(find) => {
                match ctx
                    .translator
                    .find_in_translations(&find, language_hint, false)
                {
                    Ok(it) => {
                        found_related(ctx.translator, &find, it);
                    }
                    Err(err) => {
                        eprintln!("Failed parsing search term: {err}");
                    }
                };
                return Ok(DecisionControlFlow::Repeat);
            }
            Decision::Help => todo!(),
            Decision::HardExit => {
                *ctx.overarching_decision = decision;
                return Ok(DecisionControlFlow::Exit);
            }
        }
        Ok(DecisionControlFlow::Continue)
    }
}

pub trait DecisionTaker {
    fn decision_made(
        &self,
        decision: Decision,
        undecided: &mut UndecidedTranslation,
        ctx: &mut DecisionTakerContext<'_>,
    ) -> anyhow::Result<DecisionControlFlow>;
}

impl UndecidedTranslation {
    #[allow(clippy::only_used_in_recursion)]
    pub fn decide<DT: DecisionTaker>(
        &mut self,
        translator: &mut Translator,
        decision: &mut Decision,
        decision_taker: &DT,
    ) -> anyhow::Result<usize> {
        let mut translated = 0;
        let is_silent = *decision == Decision::Accept || *decision == Decision::Deny;
        let to_id = self.to.resolve(translator);
        let from = translator.resolve(self.from);
        let count = self.full_id.len();
        let to_count = to_id.len();
        if !is_silent {
            print_translation(translator, self.full_id[0].0, to_id, from, count);
        }
        loop {
            let cur_decision = ask_for_decision(decision, count, false, to_count);
            let mut ctx = DecisionTakerContext {
                overarching_decision: decision,
                translated: &mut translated,
                translator,
            };
            match decision_taker.decision_made(cur_decision, self, &mut ctx)? {
                DecisionControlFlow::Continue => {
                    break;
                }
                DecisionControlFlow::Exit => {
                    return Ok(translated);
                }
                DecisionControlFlow::Repeat => {}
            }
        }
        Ok(translated)
    }

    pub fn integrate(&mut self, t: xliff_translation::TranslationId, recommended: LazyTranslation) {
        self.full_id.push((t.language, t.id));
        self.to.combine(recommended);
    }
}

fn ask_user_for_translation(
    translator: &Translator,
    _from: &str,
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
            .and_then(|selection| to_id.iter().find(|t| translator.resolve(**t) == selection))
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
    if to_id.is_empty() {
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
            print_translation_entry_as_bullet_point(r, translator, "", "");
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
            print_translation_entry_as_bullet_point(r, translator, from, "");
        }
    }
}

pub fn ask_for_decision(
    decision: &mut Decision,
    count: usize,
    small: bool,
    available_translations: usize,
) -> Decision {
    if small {
        loop {
            let Ok(result) =
                inquire::CustomType::<Decision>::new("Add this translation? [y, n, e]")
                    .with_default(Decision::Yes)
                    .prompt()
            else {
                break Decision::HardExit;
            };
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
            let Ok(result) = inquire::CustomType::<Decision>::new(
                "Add this translation? [y, n, a, d, e, s, l, c, f, ?]",
            )
            .prompt() else {
                break Decision::HardExit;
            };
            if result != Decision::Help
                || (count <= 1 && [Decision::Split, Decision::Languages].contains(&result))
            {
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
                println!("Use [l] to show the target languages involved");
            }
            println!("Use [c] to show context with similar textes on the page/table/etc.");
            println!(
                "Use [f] to search in existing translations for a term, also supports regex (so you may want to escape characters). (e.g. f Accounting)"
            );
        }
    }
}

pub fn print_translation_entry_as_bullet_point(
    r: xliff_translation::TranslationEntry,
    translator: &Translator,
    mark: &str,
    prefix: &str,
) {
    let rfrom = translator.resolve(r.from);
    let rto = translator.resolve(r.to);
    let text = if rto.is_empty() {
        [
            prefix,
            "'",
            &highlight(rfrom, mark),
            "' => ",
            "\u{1b}[0;31m",
            "No translation available",
            "\u{1b}[0m",
        ]
        .concat()
    } else {
        [
            prefix,
            "'",
            &highlight(rfrom, mark),
            "' => '",
            &highlight(rto, mark),
            "'",
        ]
        .concat()
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display)]
pub enum ChangedSourceDecision {
    Help,
    DeleteTranslation,
    KeepTranslation,
    Ignore,
    IgnoreAll,
    HardExit,
}
impl ChangedSourceDecision {
    pub(crate) fn print_help() {
        println!("Use [i] to ignore this entry and do nothing.");
        println!("Use [d] to ignore all entries and do nothing at all.");
        println!("Use [y] to keep the translation.");
        println!("Use [n] to reset the translation.");
        println!("Use [q] to stop and exit early.");
        println!("Use [?] to display this help.");
    }
}

impl FromStr for ChangedSourceDecision {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "i" => Self::Ignore,
            "d" => Self::IgnoreAll,
            "y" => Self::KeepTranslation,
            "n" => Self::DeleteTranslation,
            "?" | "h" => Self::Help,
            "q" => Self::HardExit,
            _ => return Err(()),
        })
    }
}

pub(crate) fn ask_for_changed_source_decision(
    decision: &mut ChangedSourceDecision,
) -> ChangedSourceDecision {
    if *decision == ChangedSourceDecision::IgnoreAll {
        return ChangedSourceDecision::IgnoreAll;
    }
    let Ok(decision) =
        inquire::CustomType::<ChangedSourceDecision>::new("What now? [i, d, y, n, ?]").prompt()
    else {
        return ChangedSourceDecision::HardExit;
    };
    decision
}
