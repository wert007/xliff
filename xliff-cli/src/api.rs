use ambassador::{Delegate, delegatable_trait};
use clap::{Parser, Subcommand};

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
