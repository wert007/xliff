use ambassador::{Delegate, delegatable_trait};
use clap::{Parser, Subcommand};
use xliff_translation::{LanguageStr, string_id_from_u32};

#[derive(Debug, Parser, Clone)]
pub struct Api {
    #[command(subcommand)]
    cmd: ApiCommand,
}
impl Api {
    pub(crate) fn run(
        &self,
        translator: xliff_translation::Translator,
    ) -> Result<(), anyhow::Error> {
        let value = self.cmd.run(translator)?;
        serde_json::to_writer_pretty(std::io::stdout(), &value)?;
        Ok(())
    }
}

#[delegatable_trait]
trait Runnable {
    fn run(&self, translator: xliff_translation::Translator) -> anyhow::Result<serde_json::Value>;
}

#[derive(Debug, Subcommand, Clone, Delegate)]
#[delegate(Runnable)]
pub enum ApiCommand {
    GetTranslationFiles(GetTranslationFiles),
    GetMissingTranslations(GetMissingTranslations),
    AddTranslation(AddTranslation),
}

#[derive(Debug, Clone, Default, clap::Parser)]
pub struct GetTranslationFiles {
    #[clap(long)]
    include_base: bool,
}

impl Runnable for GetTranslationFiles {
    fn run(&self, translator: xliff_translation::Translator) -> anyhow::Result<serde_json::Value> {
        let mut files = translator.loaded_files();
        if self.include_base {
            files.insert(0, translator.base_file().into());
        }
        Ok(serde_json::to_value(files)?)
    }
}

#[derive(Debug, Clone, Default, clap::Parser)]
pub struct GetMissingTranslations {
    #[clap(short, long)]
    language: Option<String>,
}
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MissingTranslationApi {
    language: String,
    source_id: u32,
    source: String,
    id: u32,
    existing_translations: Vec<(u32, String)>,
}

impl Runnable for GetMissingTranslations {
    fn run(&self, translator: xliff_translation::Translator) -> anyhow::Result<serde_json::Value> {
        Ok(serde_json::to_value(
            &translator
                .find_missing_translations()
                .into_iter()
                .filter_map(|m| {
                    if let Some(l) = &self.language
                        && !m.language.contains(l)
                    {
                        return None;
                    }
                    let Ok(Some((source_id, _))) = translator
                        .get_source_and_translation(LanguageStr::try_from_str("g").unwrap(), m.id)
                    else {
                        return None;
                    };
                    let source = translator.resolve(source_id).into();
                    let existing_translations = translator
                        .get_translation(m.language, source_id)
                        .iter()
                        .map(|s| (s.into_inner().get(), translator.resolve(*s).into()))
                        .collect();

                    Some(anyhow::Result::Ok(MissingTranslationApi {
                        language: m.language.to_string(),
                        source_id: source_id.into_inner().get(),
                        source,
                        existing_translations,
                        id: m.id.into_inner().get(),
                    }))
                })
                .collect::<anyhow::Result<Vec<MissingTranslationApi>>>()?,
        )?)
    }
}

#[derive(Debug, Clone, Default, clap::Parser)]
pub struct AddTranslation {
    target_languages: Vec<String>,
    id: u32,
    translation: String,
}

impl Runnable for AddTranslation {
    fn run(
        &self,
        mut translator: xliff_translation::Translator,
    ) -> anyhow::Result<serde_json::Value> {
        let target = translator.intern(self.translation.clone());
        for l in &self.target_languages {
            translator.add_translation(
                LanguageStr::try_from_str(l)?,
                unsafe { string_id_from_u32(self.id) },
                target,
            )?;
        }
        translator.save_files()?;
        Ok(serde_json::Value::Null)
    }
}
