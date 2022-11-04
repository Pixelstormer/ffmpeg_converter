use anyhow::anyhow;
use clap::Parser;
use ignore::{types::TypesBuilder, DirEntry, WalkBuilder, WalkParallel, WalkState};
use std::{
    borrow::Cow,
    fmt::Display,
    ops::Deref,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU16, Ordering},
};

/// Recursively searches a given directory and its subdirectories for files with a given extension,
/// and uses ffmpeg to convert those files to a different extension.
///
/// Effectively functions as a shorthand for the following shell commands:
///
/// `fd -e mp3 -x ffmpeg -i {} {.}.opus && fd -e mp3 -x rm`.
#[derive(Parser, Debug)]
#[command(version)]
struct Args {
    /// If set, prints information about actions that would be taken, instead of actually doing anything.
    #[arg(short, long)]
    dry_run: bool,
    /// The output file extension to which files will be converted.
    #[arg(short, long, default_value = "opus")]
    output: String,
    /// The directory to search in.
    #[arg(short, long, default_value = "./")]
    target_dir: PathBuf,
    /// The maximum search depth. If unset, is infinite.
    #[arg(short, long)]
    max_depth: Option<usize>,
    /// If set, follows symbolic links.
    #[arg(short, long)]
    follow_links: bool,
    /// If set, avoids crossing file system boundries when searching.
    #[arg(short, long)]
    same_fs: bool,
    /// The number of threads to use for searching and processing files. If unset, defaults to the number of CPU cores.
    #[arg(short, long)]
    num_threads: Option<usize>,
    /// If set, does not delete files after successfully converting them.
    #[arg(short, long)]
    preserve_files: bool,
    /// The file extensions to convert from.
    #[arg(default_value = "mp3")]
    inputs: Vec<String>,
    /// Extra arguments to be passed to ffmpeg during execution.
    #[arg(raw = true)]
    ffmpeg_args: Vec<String>,
}

struct Converter {
    args: Args,
    current_dir: Option<PathBuf>,
    ok_count: AtomicU16,
    err_count: AtomicU16,
}

impl Converter {
    fn new(args: Args) -> Self {
        Self {
            args,
            current_dir: std::env::current_dir().ok(),
            ok_count: Default::default(),
            err_count: Default::default(),
        }
    }

    fn run(&mut self) -> anyhow::Result<()> {
        if self.args.dry_run {
            println!("Dry-run enabled");
        }

        println!(
            "Converting files from {} to '{}'",
            self.format_input_args(),
            self.args.output
        );

        let walker = self.build_walker()?;
        walker.run(|| {
            Box::new(|entry| match entry {
                Ok(e) => self.try_convert_entry(&e),
                Err(e) => self.handle_error(e),
            })
        });

        println!("Converted {} files.", self.ok_count.get_mut());
        println!("Finished with {} errors.", self.err_count.get_mut());

        Ok(())
    }

    fn format_input_args(&self) -> String {
        let mut result = String::new();
        if let Some((tail, head)) = self.args.inputs.split_last() {
            result.reserve_exact(head.iter().map(|s| s.len() + 4).sum::<usize>() + tail.len() + 2);
            result.extend(head.iter().map(|s| format!("'{}', ", s)));
            result.push_str(&format!("'{}'", tail));
        }
        result
    }

    /// Configures and builds a directory iterator over the files to be converted
    fn build_walker(&self) -> anyhow::Result<WalkParallel> {
        // Use the user-specified number of threads, or the number of available CPU cores if unspecified
        let num_threads = self.args.num_threads.unwrap_or_else(num_cpus::get);

        // Only match the files we want to convert
        let mut file_types = TypesBuilder::new();
        for input in &self.args.inputs {
            file_types.add(input, &format!("*.{}", input))?;
        }
        file_types.select("all");
        let file_types = file_types.build()?;

        // Configure the directory iterator according to the user-specified args
        Ok(WalkBuilder::new(&self.args.target_dir)
            .standard_filters(false)
            .max_depth(self.args.max_depth)
            .follow_links(self.args.follow_links)
            .same_file_system(self.args.same_fs)
            .threads(num_threads)
            .types(file_types)
            .build_parallel())
    }

    /// Transforms the input path into a form suitable for displaying
    fn get_display_path<'a>(&'a self, path: &'a Path) -> impl Deref<Target = Path> + '_ {
        self.current_dir
            .as_deref()
            .and_then(|base| pathdiff::diff_paths(path, base))
            .or_else(|| path.canonicalize().ok())
            .map_or_else(|| Cow::Borrowed(path), Cow::Owned)
    }

    fn try_convert_entry(&self, entry: &DirEntry) -> WalkState {
        if let Some(err) = entry.error() {
            return self.handle_error(err);
        }

        let Some(file_type) = entry.file_type() else {
            return self.handle_error(anyhow!(
                "Directory entry '{}' does not have a file type",
                entry.path().display()
            ));
        };

        if !file_type.is_file() {
            // Skip, but don't terminate, on entries that are not paths,
            // as these include the directories being searched
            return WalkState::Continue;
        }

        let path = entry.path();

        println!("Converting '{}'", self.get_display_path(path).display());

        match self.try_convert_path(path) {
            Ok(path) => {
                println!(
                    "Finished converting '{}'",
                    self.get_display_path(&path).display()
                );

                self.ok_count.fetch_add(1, Ordering::Relaxed);
                WalkState::Continue
            }
            Err(err) => self.handle_error(err),
        }
    }

    fn try_convert_path(&self, path: &Path) -> anyhow::Result<PathBuf> {
        let output_path = path.with_extension(&self.args.output);

        let mut command = Command::new("ffmpeg");
        command
            .arg("-i")
            .arg(path)
            .args(&self.args.ffmpeg_args)
            .arg(&output_path);

        if self.args.dry_run {
            // On a dry-run, just print what we would do instead of actually doing it
            println!("Dry_run: Running '{:?}'", command);
            if !self.args.preserve_files {
                println!(
                    "Dry_run: Removing file '{}'",
                    self.get_display_path(path).display()
                );
            }
            Ok(output_path)
        } else {
            // On a non-dry-run, actually run the command
            let output = command.output()?;
            if output.status.success() {
                if !self.args.preserve_files {
                    // Attempt to remove the input file if the command succeeded
                    std::fs::remove_file(path)?;
                }
                Ok(output_path)
            } else {
                // If the command didn't succeed, don't remove the input file to avoid potential data loss,
                // and return the command's error log
                Err(anyhow!(String::from_utf8_lossy(&output.stderr).to_string()))
            }
        }
    }

    fn handle_error(&self, err: impl Display) -> WalkState {
        self.err_count.fetch_add(1, Ordering::Relaxed);
        println!("{:#}", err);
        WalkState::Quit
    }
}

fn main() -> anyhow::Result<()> {
    Converter::new(Args::parse()).run()
}
