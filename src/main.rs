use anyhow::bail;
use clap::Parser;
use ignore::{types::TypesBuilder, DirEntry, WalkBuilder, WalkParallel, WalkState};
use std::{
    path::PathBuf,
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
#[clap(version = clap::crate_version!())]
struct Args {
    /// If set, prints information about actions that would be taken, instead of actually doing anything.
    #[clap(short, long)]
    dry_run: bool,
    /// The maximum search depth. If unset, is infinite.
    #[clap(short, long)]
    max_depth: Option<usize>,
    /// If set, follows symbolic links.
    #[clap(short, long)]
    follow_links: bool,
    /// If set, avoids crossing file system boundries when searching.
    #[clap(short, long)]
    same_fs: bool,
    /// The file extension to convert from.
    #[clap(default_value = "mp3")]
    from: String,
    /// The file extension to convert to.
    #[clap(default_value = "opus")]
    to: String,
    /// The directory to search in.
    #[clap(last = true, default_value = "./")]
    target_dir: PathBuf,
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
            "Converting files from '{}' to '{}'",
            self.args.from, self.args.to
        );

        let walker = self.build_walker()?;
        walker.run(|| {
            Box::new(|entry| match entry {
                Ok(e) => self.try_convert_entry(e),
                Err(e) => self.handle_error(e.into()),
            })
        });

        println!("Converted {} files.", self.ok_count.get_mut());
        println!("Finished with {} errors.", self.err_count.get_mut());

        Ok(())
    }

    /// Configures and builds a directory iterator over the files to be converted
    fn build_walker(&self) -> anyhow::Result<WalkParallel> {
        // Utilise all available CPU cores
        let num_threads = num_cpus::get();

        // Only match the files we want to convert
        let mut file_types = TypesBuilder::new();
        file_types.add("from", &format!("*.{}", self.args.from))?;
        file_types.select("from");
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

    fn try_convert_entry(&self, entry: DirEntry) -> WalkState {
        if entry.file_type().map(|f| f.is_file()).unwrap_or_default() {
            let input_path = entry.into_path();
            let output_path = input_path.with_extension(&self.args.to);

            println!(
                "Converting '{}'",
                self.current_dir
                    .as_ref()
                    .and_then(|p| pathdiff::diff_paths(&input_path, p))
                    .as_ref()
                    .unwrap_or(&input_path)
                    .display()
            );

            let mut command = Command::new("ffmpeg");
            command.arg("-i").arg(&input_path).arg(&output_path);

            if self.args.dry_run {
                println!("Dry_run: Running '{:?}'", command);
                println!("Dry_run: Removing file '{}'", input_path.display());
            } else if let Err(e) = command.output().map_err(anyhow::Error::new).and_then(|o| {
                o.status
                    .success()
                    .then(|| std::fs::remove_file(input_path))
                    .map_or_else(
                        || bail!(String::from_utf8_lossy(&o.stderr).into_owned()),
                        |e| e.map_err(anyhow::Error::new),
                    )
            }) {
                self.err_count.fetch_add(1, Ordering::Relaxed);
                println!("{}", e);
            } else {
                self.ok_count.fetch_add(1, Ordering::Relaxed);

                println!(
                    "Finished converting '{}'",
                    self.current_dir
                        .as_ref()
                        .and_then(|p| pathdiff::diff_paths(&output_path, p))
                        .as_ref()
                        .unwrap_or(&output_path)
                        .display()
                );
            }
        }

        WalkState::Continue
    }

    fn handle_error(&self, err: anyhow::Error) -> WalkState {
        self.err_count.fetch_add(1, Ordering::Relaxed);
        println!("{}", err);
        WalkState::Quit
    }
}

fn main() -> anyhow::Result<()> {
    Converter::new(Args::parse()).run()
}
