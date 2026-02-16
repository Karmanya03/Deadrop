use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::Write;
use std::path::{Path, PathBuf};
use indicatif::ProgressBar;

/// Maximum length for the root folder name inside the archive.
/// tar's ustar header supports 100 bytes for the full path, so we keep
/// the root prefix short to leave room for nested file/dir names.
const MAX_ARCHIVE_PREFIX: usize = 50;

/// Truncate a folder name to fit within tar path limits.
/// Preserves as many leading chars as possible.
fn truncate_archive_prefix(name: &str) -> String {
    if name.len() <= MAX_ARCHIVE_PREFIX {
        return name.to_string();
    }
    // Truncate and trim trailing whitespace/hyphens/underscores for cleanliness
    let truncated = &name[..MAX_ARCHIVE_PREFIX];
    truncated.trim_end_matches(|c: char| c == ' ' || c == '-' || c == '_').to_string()
}

/// Archives a folder into an in-memory .tar.gz and returns (bytes, display_name)
pub fn compress_folder(
    path: &Path,
    progress: &ProgressBar,
) -> anyhow::Result<(Vec<u8>, String)> {
    if !path.is_dir() {
        return Err(anyhow::anyhow!("{} is not a directory", path.display()));
    }

    let folder_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // FIX: Truncate long folder names to avoid tar 100-byte path limit
    let short_prefix = truncate_archive_prefix(&folder_name);
    let archive_name = format!("{}.tar.gz", short_prefix);

    progress.set_message(format!("Archiving {}...", folder_name));

    // Count total files for progress
    let total_files = count_files(path)?;
    progress.set_length(total_files);
    progress.set_position(0);

    // Create tar.gz in memory
    let mut compressed_bytes: Vec<u8> = Vec::new();
    {
        let encoder = GzEncoder::new(&mut compressed_bytes, Compression::fast());
        let mut tar_builder = tar::Builder::new(encoder);

        // Follow symlinks for safety, don't include parent dirs
        tar_builder.follow_symlinks(false);

        // Recursively add the folder — use short_prefix as root inside archive
        add_dir_recursive(&mut tar_builder, path, Path::new(&short_prefix), progress)?;

        // Finalize tar
        let encoder = tar_builder.into_inner()?;
        encoder.finish()?;
    }

    progress.finish_with_message(format!(
        "Archived {} ({} files → {})",
        folder_name,
        total_files,
        bytesize::ByteSize::b(compressed_bytes.len() as u64)
    ));

    Ok((compressed_bytes, archive_name))
}

/// Bundle multiple files and/or folders into a single .tar.gz on disk
pub fn bundle_files(paths: &[PathBuf], output: &Path) -> anyhow::Result<()> {
    let file = std::fs::File::create(output)?;
    let enc = GzEncoder::new(file, Compression::fast());
    let mut tar = tar::Builder::new(enc);
    tar.follow_symlinks(false);

    for path in paths {
        if path.is_dir() {
            let dir_name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            // FIX: Truncate long folder names in bundles too
            let short_name = truncate_archive_prefix(&dir_name);
            add_dir_recursive(
                &mut tar,
                path,
                Path::new(&short_name),
                &ProgressBar::hidden(),
            )?;
        } else if path.is_file() {
            let file_name = path.file_name().unwrap_or_default();
            let mut f = std::fs::File::open(path)?;
            let metadata = f.metadata()?;
            let mut header = tar::Header::new_gnu();
            header.set_path(file_name)?;
            header.set_size(metadata.len());
            header.set_mode(0o644);
            header.set_mtime(
                metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            );
            header.set_cksum();
            tar.append(&header, &mut f)?;
        }
    }

    let enc = tar.into_inner()?;
    enc.finish()?;
    Ok(())
}

/// Safely set the path on a tar header, falling back to append_data
/// if the path exceeds tar's 100-byte ustar limit.
///
/// GNU tar format supports long names via special extension headers.
/// `header.set_path()` only works for ≤100 bytes, but
/// `builder.append_data()` handles GNU long name extensions automatically.
fn safe_append_file<W: Write>(
    builder: &mut tar::Builder<W>,
    archive_path: &Path,
    src_path: &Path,
) -> anyhow::Result<()> {
    let mut file = std::fs::File::open(src_path)?;
    let metadata = file.metadata()?;
    let mut header = tar::Header::new_gnu();
    header.set_size(metadata.len());
    header.set_mode(0o644);
    header.set_mtime(
        metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0),
    );
    header.set_cksum();

    // append_data handles long paths via GNU extensions automatically
    builder.append_data(&mut header, archive_path, &mut file)?;
    Ok(())
}

/// Safely append a directory entry, handling long paths.
fn safe_append_dir<W: Write>(
    builder: &mut tar::Builder<W>,
    archive_path: &Path,
) -> anyhow::Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_mode(0o755);
    header.set_entry_type(tar::EntryType::Directory);
    header.set_cksum();

    // append_data handles long paths via GNU extensions automatically
    builder.append_data(&mut header, archive_path, &mut std::io::empty())?;
    Ok(())
}

/// Recursively add directory contents to tar archive
fn add_dir_recursive<W: Write>(
    builder: &mut tar::Builder<W>,
    src_path: &Path,
    archive_path: &Path,
    progress: &ProgressBar,
) -> anyhow::Result<()> {
    let entries = std::fs::read_dir(src_path)?;

    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let entry_name = entry.file_name();
        let src_child = entry.path();
        let archive_child = archive_path.join(&entry_name);

        if file_type.is_file() {
            safe_append_file(builder, &archive_child, &src_child)?;
            progress.inc(1);
        } else if file_type.is_dir() {
            // Skip hidden dirs and common junk
            let name_str = entry_name.to_string_lossy();
            if name_str.starts_with('.') || name_str == "node_modules" || name_str == "target" {
                continue;
            }

            safe_append_dir(builder, &archive_child)?;

            // Recurse
            add_dir_recursive(builder, &src_child, &archive_child, progress)?;
        }
        // Skip symlinks for security
    }

    Ok(())
}

/// Count total files in a directory (recursive)
fn count_files(path: &Path) -> anyhow::Result<u64> {
    let mut count = 0u64;
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        if ft.is_file() {
            count += 1;
        } else if ft.is_dir() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.starts_with('.') && name_str != "node_modules" && name_str != "target" {
                count += count_files(&entry.path())?;
            }
        }
    }
    Ok(count)
}
