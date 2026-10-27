use flowtorrent::{app::targets::prepare_add, domain::*};
use std::path::{Path, PathBuf};

fn preview(paths: &[&str], multi_file: bool) -> TorrentPreview {
    TorrentPreview {
        id: TorrentId([7; 20]),
        name: if multi_file { "bundle" } else { "file.bin" }.into(),
        files: paths
            .iter()
            .enumerate()
            .map(|(index, path)| TorrentFile {
                id: FileId(index),
                relative_path: PathBuf::from(path),
                length: 4,
                is_padding: false,
            })
            .collect(),
        metadata: vec![],
        multi_file,
    }
}

fn request(preview: &TorrentPreview, folder: &Path) -> AddRequest {
    AddRequest::new(
        preview.id,
        folder.to_owned(),
        preview
            .files
            .iter()
            .filter(|f| !f.is_padding)
            .map(|f| f.id)
            .collect(),
    )
}

#[test]
fn empty_selection_is_rejected_before_creating_any_payload() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let folder = root.path().canonicalize()?;
    let preview = preview(&["file.bin"], false);
    let empty = AddRequest::new(preview.id, folder.clone(), vec![]);
    assert!(prepare_add(&preview, &empty).is_err());
    assert_eq!(std::fs::read_dir(folder)?.count(), 0);
    Ok(())
}

#[test]
fn single_and_multi_file_destinations_match_preview() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let folder = root.path().canonicalize()?;
    let single = preview(&["file.bin"], false);
    let prepared = prepare_add(&single, &request(&single, &folder))?;
    assert_eq!(prepared.destination, folder);
    assert!(!folder.join("file.bin").exists());
    let multi = preview(&["inner/file.bin"], true);
    let prepared = prepare_add(&multi, &request(&multi, &folder))?;
    assert_eq!(prepared.destination, folder.join("bundle"));
    assert!(!folder.join("bundle").exists());
    prepared.claim_files()?;
    assert_eq!(
        std::fs::metadata(folder.join("bundle/inner/file.bin"))?.len(),
        0
    );
    Ok(())
}

#[test]
fn existing_payload_is_not_overwritten_including_unselected_placeholders() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let folder = root.path().canonicalize()?;
    std::fs::create_dir(folder.join("bundle"))?;
    std::fs::write(folder.join("bundle/keep.bin"), b"irreplaceable")?;
    let preview = preview(&["new.bin", "keep.bin"], true);
    let request = AddRequest::new(preview.id, folder.clone(), vec![FileId(0)]);
    assert!(prepare_add(&preview, &request).is_err());
    assert_eq!(
        std::fs::read(folder.join("bundle/keep.bin"))?,
        b"irreplaceable"
    );
    assert!(!folder.join("bundle/new.bin").exists());
    Ok(())
}

#[test]
fn escape_reserved_and_normalized_collisions_are_rejected() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let folder = root.path().canonicalize()?;
    for path in [
        "../escape.bin",
        "/escape.bin",
        r"C:\escape.bin",
        r"\\host\share\escape.bin",
        "C:escape.bin",
        "NUL.txt",
        "file.bin.",
        "dir/file.bin ",
    ] {
        let preview = preview(&[path], false);
        assert!(
            prepare_add(&preview, &request(&preview, &folder)).is_err(),
            "{path}"
        );
    }
    for paths in [
        ["A.bin", "a.bin"],
        ["é.bin", "e\u{301}.bin"],
        ["node", "node/child.bin"],
    ] {
        let preview = preview(&paths, true);
        assert!(
            prepare_add(&preview, &request(&preview, &folder)).is_err(),
            "{paths:?}"
        );
    }
    assert_eq!(std::fs::read_dir(folder)?.count(), 0);
    Ok(())
}

#[test]
fn a_file_created_between_preview_and_claim_is_never_clobbered() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let folder = root.path().canonicalize()?;
    let preview = preview(&["file.bin"], false);
    let prepared = prepare_add(&preview, &request(&preview, &folder))?;
    std::fs::write(folder.join("file.bin"), b"foreign")?;
    assert!(prepared.claim_files().is_err());
    assert_eq!(std::fs::read(folder.join("file.bin"))?, b"foreign");
    Ok(())
}

fn link_directory(target: &Path, link: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        junction::create(target, link)
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
}

#[test]
fn targets_rejects_escape_and_links() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let folder = root.path().canonicalize()?;
    let target = folder.join("target");
    let outside = folder.join("outside");
    std::fs::create_dir(&target)?;
    std::fs::create_dir(&outside)?;
    std::fs::write(outside.join("file.bin"), b"foreign")?;
    link_directory(&outside, &target.join("inner"))?;
    let preview = preview(&["inner/file.bin"], false);
    assert!(prepare_add(&preview, &request(&preview, &target)).is_err());
    assert_eq!(std::fs::read(outside.join("file.bin"))?, b"foreign");
    Ok(())
}

#[test]
fn a_directory_link_swapped_in_after_preview_cannot_redirect_writes() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let folder = root.path().canonicalize()?;
    let target = folder.join("target");
    let outside = folder.join("outside");
    std::fs::create_dir_all(target.join("inner"))?;
    std::fs::create_dir(&outside)?;
    std::fs::write(outside.join("file.bin"), b"foreign")?;
    let preview = preview(&["inner/file.bin"], false);
    let prepared = prepare_add(&preview, &request(&preview, &target))?;
    std::fs::rename(target.join("inner"), target.join("before-swap"))?;
    link_directory(&outside, &target.join("inner"))?;
    assert!(prepared.claim_files().is_err());
    assert_eq!(std::fs::read(outside.join("file.bin"))?, b"foreign");
    Ok(())
}

#[test]
fn unicode_long_names_and_padding_keep_original_indices() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let folder = root.path().canonicalize()?;
    let long_name = format!("{}.bin", "Ж".repeat(90));
    let mut preview = preview(&["first.bin", "padding.bin", &long_name], true);
    preview.files[1].is_padding = true;
    assert_eq!(preview.default_selection(), vec![FileId(0), FileId(2)]);
    let prepared = prepare_add(&preview, &request(&preview, &folder))?;
    prepared.claim_files()?;
    assert!(folder.join("bundle").join(long_name).is_file());
    assert!(!folder.join("bundle/padding.bin").exists());
    Ok(())
}

#[test]
fn unavailable_target_does_not_create_a_substitute_folder() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let missing = root.path().canonicalize()?.join("disconnected-volume");
    let preview = preview(&["file.bin"], false);
    assert!(prepare_add(&preview, &request(&preview, &missing)).is_err());
    assert!(!missing.exists());
    Ok(())
}
