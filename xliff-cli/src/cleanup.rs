use crate::Cleanup;

pub(crate) fn run_cleanup(
    mut translator: xliff_translation::Translator,
    cleanup: Cleanup,
) -> Result<(), anyhow::Error> {
    let mut cleaned_something_up = false;
    let orphaned = translator.find_orphaned_translation_units();
    if !orphaned.is_empty() {
        cleaned_something_up = true;
        println!(
            "There are {} lost entries over {} files.",
            orphaned.len(),
            translator.loaded_files().len()
        );
        for orphan in orphaned {
            translator.delete_entry(orphan);
        }
    }

    let duplicated = translator.find_duplicated_ids(false);
    if !duplicated.is_empty() {
        cleaned_something_up = true;
        for (id, duplicates) in duplicated {
            println!("{} x {}", duplicates.len(), translator.resolve(id));
        }
    }
    if cleaned_something_up && !cleanup.dry {
        translator.save_files()?;
    } else if !cleaned_something_up {
        println!("Translations are structurally clean. Nothing to do.");
    }
    Ok(())
}
