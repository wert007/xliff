use crate::Cleanup;

pub(crate) fn run_cleanup(
    translator: xliff_translation::Translator,
    Cleanup: Cleanup,
) -> Result<(), anyhow::Error> {
    let duplicated = translator.find_duplicated_ids(false);
    for (id, duplicates) in duplicated {
        println!("{} x {}", duplicates.len(), translator.resolve(id));
    }
    Ok(())
}
