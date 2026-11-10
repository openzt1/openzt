use std::{
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
    str,
};

use anyhow::Context;
use zip::ZipArchive;

pub struct ZtdArchive {
    archive: ZipArchive<BufReader<File>>,
    archive_name: String,
    archive_path: PathBuf,
}

impl ZtdArchive {
    pub fn new(archive_path: &Path) -> anyhow::Result<Self> {
        let archive_name = archive_path
            .to_str()
            .with_context(|| format!("Error reading archive path {}", archive_path.display()))?
            .to_string();
        let archive = ZipArchive::new(BufReader::new(
            File::open(archive_path).with_context(|| format!("Failed to open archive {}", archive_path.display()))?,
        ))
        .with_context(|| format!("Failed to read archive {}", archive_path.display()))?;

        Ok(Self {
            archive,
            archive_name,
            archive_path: archive_path.to_path_buf(),
        })
    }

    pub fn name(&self) -> &str {
        &self.archive_name
    }

    pub fn by_name(&mut self, file_name: &str) -> anyhow::Result<ZtdFile<'_>> {
        let zip_file = self
            .archive
            .by_name(file_name)
            .with_context(|| format!("Error finding file in archive: {}", file_name))?;
        Ok(ZtdFile { inner: zip_file })
    }

    pub fn len(&self) -> usize {
        self.archive.len()
    }

    pub fn by_index(&mut self, index: usize) -> anyhow::Result<ZtdFile<'_>> {
        let zip_file = self
            .archive
            .by_index(index)
            .with_context(|| format!("Error finding file in archive at index: {}", index))?;
        Ok(ZtdFile { inner: zip_file })
    }

    pub fn file_names(&self) -> impl Iterator<Item = String> + '_ {
        self.archive.file_names().filter_map(|name| name.ok()).map(|name| name.into_owned())
    }
}

pub struct ZtdFile<'a, R: Read = BufReader<File>> {
    inner: zip::read::ZipFile<'a, R>,
}

impl<'a, R: Read> ZtdFile<'a, R> {
    pub fn new(inner: zip::read::ZipFile<'a, R>) -> Self {
        Self { inner }
    }
}

impl<R: Read> ZtdFile<'_, R> {
    pub fn name(&self) -> String {
        self.inner.name().map(|name| name.into_owned()).unwrap_or_default()
    }

    pub fn read_exact(&mut self, buffer: &mut [u8]) -> anyhow::Result<()> {
        self.inner.read_exact(buffer).with_context(|| format!("Error reading file: {}", self.name()))?;
        Ok(())
    }

    pub fn size(&self) -> u64 {
        self.inner.size()
    }

    pub fn is_dir(&self) -> bool {
        self.inner.is_dir()
    }

    pub fn read_to_string(&mut self) -> anyhow::Result<String> {
        let mut buffer = vec![0u8; self.inner.size() as usize].into_boxed_slice();
        self.inner
            .read_exact(&mut buffer)
            .with_context(|| format!("Error reading file: {}", self.name()))?;

        Ok(crate::encoding_utils::decode_game_text(&buffer))
    }
}

impl<R: Read> TryFrom<ZtdFile<'_, R>> for String {
    type Error = anyhow::Error;

    fn try_from(mut file: ZtdFile<'_, R>) -> Result<String, Self::Error> {
        let mut buffer = vec![0u8; file.size() as usize].into_boxed_slice();
        file.inner
            .read_exact(&mut buffer)
            .with_context(|| format!("Error reading file: {}", file.name()))?;

        Ok(crate::encoding_utils::decode_game_text(&buffer))
    }
}
