use anyhow::anyhow;
use rust_bert::pipelines::translation::{Language, TranslationModelBuilder};

pub fn english_to_german(english: &str) -> anyhow::Result<String> {
    let m = TranslationModelBuilder::new()
        .with_medium_model()
        .with_source_languages([Language::English])
        .with_target_languages([Language::German])
        .create_model()?;
    let t = m
        .translate(&[english], Language::English, Language::German)?
        .pop()
        .ok_or(anyhow!("No translation was generated for {english}"))?
        .trim()
        .into();
    Ok(t)
}
