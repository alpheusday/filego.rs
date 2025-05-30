use std::path::{Path, PathBuf};

use tokio::{
    fs::{self, ReadDir},
    io::{self, AsyncReadExt, AsyncWriteExt},
};

use crate::merge::Merge;

/// Trait for running the merge process.
pub trait MergeAsyncExt {
    /// Run the check process asynchronously.
    fn run_async(
        &self
    ) -> impl std::future::Future<Output = io::Result<bool>> + Send;
}

impl MergeAsyncExt for Merge {
    async fn run_async(&self) -> io::Result<bool> {
        let in_dir: &Path = match self.in_dir {
            | Some(ref p) => {
                let p: &Path = p.as_ref();

                // if in_dir not exists
                if !p.exists() {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        "in_dir path not found",
                    ));
                }

                // if in_dir not a directory
                if !p.is_dir() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "in_dir is not a directory",
                    ));
                }

                p
            },
            | None => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "in_dir is not set",
                ));
            },
        };

        let out_file: &Path = match self.out_file {
            | Some(ref p) => p.as_ref(),
            | None => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "out_file is not set",
                ));
            },
        };

        // check file size for buffer capacity
        let mut read_dir: ReadDir = fs::read_dir(in_dir).await?;

        let input_size: usize =
            if let Some(entry) = read_dir.next_entry().await? {
                let metadata = fs::metadata(entry.path()).await?;
                if metadata.is_file() {
                    metadata.len() as usize
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        "No files found in in_dir",
                    ));
                }
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "No files found in in_dir",
                ));
            };

        let buffer_capacity: usize = input_size.min(self.cap_max);

        // delete outpath target if exists
        if out_file.exists() {
            if out_file.is_dir() {
                fs::remove_dir_all(&out_file).await?;
            } else {
                fs::remove_file(&out_file).await?;
            }
        }

        // create outpath
        if let Some(parent) = out_file.parent() {
            fs::create_dir_all(parent).await?;
        }

        let output: fs::File = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(out_file)
            .await?;

        // writer
        let mut writer: io::BufWriter<fs::File> =
            io::BufWriter::with_capacity(buffer_capacity, output);

        // get inputs
        let mut entries: Vec<PathBuf> = Vec::new();

        let mut read_dir: ReadDir = fs::read_dir(in_dir).await?;

        while let Some(entry) = read_dir.next_entry().await? {
            let path: PathBuf = entry.path();

            if path.is_file() {
                entries.push(path);
            }
        }

        entries.sort_by_key(|entry| {
            entry
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .parse::<usize>()
                .unwrap()
        });

        // merge
        for entry in entries {
            let input: fs::File =
                fs::OpenOptions::new().read(true).open(&entry).await?;

            let mut reader: io::BufReader<fs::File> =
                io::BufReader::with_capacity(buffer_capacity, input);

            let mut buffer: Vec<u8> = vec![0; buffer_capacity];

            loop {
                let read: usize = reader.read(&mut buffer).await?;

                if read == 0 {
                    break;
                }

                writer.write_all(&buffer[..read]).await?;
            }
        }

        writer.flush().await?;

        Ok(true)
    }
}
