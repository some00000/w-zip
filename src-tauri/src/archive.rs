use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Cursor};
use std::path::{Component, Path, PathBuf};

use zeroize::Zeroize;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::error::Error;
use crate::format::{self, KdfParams};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportReport {
    pub destination: PathBuf,
    pub file_count: usize,
    pub byte_count: u64,
}

pub fn export_wzip(
    sources: &[PathBuf],
    password: &str,
    destination: &Path,
) -> Result<ExportReport, Error> {
    export_wzip_with(sources, password, destination, KdfParams::interactive())
}

pub fn export_wzip_with(
    sources: &[PathBuf],
    password: &str,
    destination: &Path,
    kdf: KdfParams,
) -> Result<ExportReport, Error> {
    if password.is_empty() {
        return Err(Error::EmptyPassword);
    }
    if sources.is_empty() {
        return Err(Error::EmptySelection);
    }

    let mut zipped = zip_files(sources)?;
    let sealed = match format::seal(&zipped, password, kdf) {
        Ok(sealed) => sealed,
        Err(err) => {
            zipped.zeroize();
            return Err(err);
        }
    };
    zipped.zeroize();

    let destination = with_wzip_extension(destination);
    if let Some(parent) = destination.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|err| Error::io(parent, err))?;
        }
    }
    fs::write(&destination, &sealed).map_err(|err| Error::io(&destination, err))?;
    Ok(ExportReport {
        file_count: sources.len(),
        byte_count: sealed.len() as u64,
        destination,
    })
}

pub fn import_wzip(
    archive: &Path,
    password: &str,
    destination: &Path,
) -> Result<Vec<String>, Error> {
    if password.is_empty() {
        return Err(Error::EmptyPassword);
    }
    let bytes = fs::read(archive).map_err(|err| Error::io(archive, err))?;
    let mut plain = format::open(&bytes, password)?;
    let names = match extract_zip(&plain, destination) {
        Ok(names) => names,
        Err(err) => {
            plain.zeroize();
            return Err(err);
        }
    };
    plain.zeroize();
    Ok(names)
}

fn zip_files(sources: &[PathBuf]) -> Result<Vec<u8>, Error> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = ZipWriter::new(&mut cursor);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        let mut used = HashSet::new();
        for source in sources {
            if !source.is_file() {
                return Err(Error::NotAFile(source.display().to_string()));
            }
            let original = source
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .filter(|name| !name.is_empty())
                .ok_or_else(|| Error::NotAFile(source.display().to_string()))?;
            let entry_name = unique_name(&mut used, &original);
            writer
                .start_file(entry_name, options)
                .map_err(Error::archive)?;
            let mut file = File::open(source).map_err(|err| Error::io(source, err))?;
            io::copy(&mut file, &mut writer).map_err(|err| Error::io(source, err))?;
        }
        writer.finish().map_err(Error::archive)?;
    }
    Ok(cursor.into_inner())
}

fn extract_zip(bytes: &[u8], destination: &Path) -> Result<Vec<String>, Error> {
    fs::create_dir_all(destination).map_err(|err| Error::io(destination, err))?;
    let dest_root = destination
        .canonicalize()
        .map_err(|err| Error::io(destination, err))?;

    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(Error::archive)?;
    let mut names = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(Error::archive)?;
        if file.is_dir() {
            continue;
        }
        let raw_name = file.name().to_string();
        let relative = file
            .enclosed_name()
            .map(|path| path.to_path_buf())
            .ok_or_else(|| Error::UnsafePath(raw_name))?;
        if relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err(Error::UnsafePath(relative.display().to_string()));
        }

        let out_path = dest_root.join(&relative);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|err| Error::io(parent, err))?;
        }
        let mut output = File::create(&out_path).map_err(|err| Error::io(&out_path, err))?;
        io::copy(&mut file, &mut output).map_err(|err| Error::io(&out_path, err))?;
        let written = out_path
            .canonicalize()
            .map_err(|err| Error::io(&out_path, err))?;
        if !written.starts_with(&dest_root) {
            let _ = fs::remove_file(&written);
            return Err(Error::UnsafePath(relative.display().to_string()));
        }
        names.push(relative.to_string_lossy().replace('\\', "/"));
    }
    Ok(names)
}

fn unique_name(used: &mut HashSet<String>, file_name: &str) -> String {
    if used.insert(file_name.to_string()) {
        return file_name.to_string();
    }
    let (stem, ext) = match file_name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() => (stem, Some(ext)),
        _ => (file_name, None),
    };
    let mut index = 2u32;
    loop {
        let candidate = match ext {
            Some(ext) => format!("{stem} ({index}).{ext}"),
            None => format!("{stem} ({index})"),
        };
        if used.insert(candidate.clone()) {
            return candidate;
        }
        index += 1;
    }
}

fn with_wzip_extension(path: &Path) -> PathBuf {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("wzip") => path.to_path_buf(),
        _ => {
            let mut name = path.as_os_str().to_owned();
            name.push(".wzip");
            PathBuf::from(name)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static SCRATCH: AtomicU64 = AtomicU64::new(0);

    fn scratch(label: &str) -> PathBuf {
        let n = SCRATCH.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("wzip-{label}-{}-{nanos}-{n}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_file(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn seals_files_and_extracts_them() {
        let root = scratch("roundtrip");
        let source_a = root.join("in").join("notes.txt");
        let source_b = root.join("in").join("other.bin");
        write_file(&source_a, b"alpha");
        write_file(&source_b, b"beta-bytes");

        let report = export_wzip_with(
            &[source_a, source_b],
            "test-pass",
            &root.join("out").join("bundle"),
            KdfParams::minimal(),
        )
        .unwrap();
        assert!(report.destination.ends_with("bundle.wzip"));
        assert_eq!(report.file_count, 2);

        let sealed = fs::read(&report.destination).unwrap();
        assert!(sealed.starts_with(b"WZIP"));
        assert!(!sealed.windows(b"notes.txt".len()).any(|w| w == b"notes.txt"));
        assert!(!sealed.windows(b"alpha".len()).any(|w| w == b"alpha"));
        assert_ne!(&sealed[format::HEADER_LEN..format::HEADER_LEN + 2], b"PK");

        let extracted = root.join("extracted");
        let names = import_wzip(&report.destination, "test-pass", &extracted).unwrap();
        assert_eq!(names, vec!["notes.txt".to_string(), "other.bin".to_string()]);
        assert_eq!(fs::read(extracted.join("notes.txt")).unwrap(), b"alpha");
        assert_eq!(fs::read(extracted.join("other.bin")).unwrap(), b"beta-bytes");
        assert!(matches!(
            import_wzip(&report.destination, "wrong", &root.join("nope")),
            Err(Error::Decrypt)
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn duplicate_basenames_are_kept() {
        let root = scratch("dupes");
        let first = root.join("a").join("note.txt");
        let second = root.join("b").join("note.txt");
        write_file(&first, b"one");
        write_file(&second, b"two");
        let archive = root.join("dupes.wzip");
        export_wzip_with(&[first, second], "pw", &archive, KdfParams::minimal()).unwrap();
        let dest = root.join("out");
        let names = import_wzip(&archive, "pw", &dest).unwrap();
        assert_eq!(names, vec!["note.txt".to_string(), "note (2).txt".to_string()]);
        assert_eq!(fs::read(dest.join("note.txt")).unwrap(), b"one");
        assert_eq!(fs::read(dest.join("note (2).txt")).unwrap(), b"two");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_directories_empty_input_and_zip_slip() {
        let root = scratch("reject");
        let dir = root.join("folder");
        fs::create_dir_all(&dir).unwrap();
        assert!(matches!(
            export_wzip_with(&[dir], "pw", &root.join("x.wzip"), KdfParams::minimal()),
            Err(Error::NotAFile(_))
        ));
        assert!(matches!(
            export_wzip_with(&[], "pw", &root.join("x.wzip"), KdfParams::minimal()),
            Err(Error::EmptySelection)
        ));

        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = ZipWriter::new(&mut cursor);
            let options =
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            writer.start_file("../evil.txt", options).unwrap();
            writer.write_all(b"nope").unwrap();
            writer.finish().unwrap();
        }
        let sealed = format::seal(&cursor.into_inner(), "pw", KdfParams::minimal()).unwrap();
        let archive = root.join("slip.wzip");
        fs::write(&archive, sealed).unwrap();
        let err = import_wzip(&archive, "pw", &root.join("slip-out")).unwrap_err();
        assert!(matches!(err, Error::UnsafePath(_)), "{err}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn interactive_kdf_roundtrip() {
        let root = scratch("interactive");
        let source = root.join("hello.txt");
        write_file(&source, b"hello wzip");
        let report = export_wzip(&[source], "long passphrase", &root.join("hello.wzip")).unwrap();
        let sealed = fs::read(&report.destination).unwrap();
        assert_eq!(&sealed[..4], b"WZIP");
        assert_eq!(sealed[4], 1);
        let memory = u32::from_le_bytes(sealed[5..9].try_into().unwrap());
        assert_eq!(memory, format::INTERACTIVE_MEMORY_KIB);
        let dest = root.join("back");
        let names = import_wzip(&report.destination, "long passphrase", &dest).unwrap();
        assert_eq!(names, vec!["hello.txt".to_string()]);
        assert_eq!(fs::read(dest.join("hello.txt")).unwrap(), b"hello wzip");
        let _ = fs::remove_dir_all(root);
    }
}
