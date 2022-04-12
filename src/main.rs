use anyhow::bail;
use clap::Parser;
use ignore::{types::TypesBuilder, WalkBuilder, WalkState};
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
    /// If true, prints information about actions that would be taken, instead of actually doing anything.
    #[clap(short, long)]
    dry_run: bool,
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

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if args.dry_run {
        println!("Dry-run enabled");
    }

    println!("Converting files from '{}' to '{}'", args.from, args.to);

    let error_count = AtomicU16::new(0);
    let converted_count = AtomicU16::new(0);

    let mut types = TypesBuilder::new();
    types.add("from", &format!("*.{}", args.from))?;
    types.select("from");

    let current_dir = std::env::current_dir().ok();

    WalkBuilder::new(args.target_dir)
        .standard_filters(false)
        .types(types.build()?)
        .threads(num_cpus::get())
        .build_parallel()
        .run(|| {
            Box::new(|path| match path {
                Ok(dir) if dir.file_type().map(|f| f.is_file()).unwrap_or_default() => {
                    let input_path = dir.into_path();
                    let output_path = input_path.with_extension(&args.to);

                    println!(
                        "Converting '{}'",
                        current_dir
                            .as_ref()
                            .and_then(|p| pathdiff::diff_paths(&input_path, p))
                            .as_ref()
                            .unwrap_or(&input_path)
                            .display()
                    );

                    let mut command = Command::new("ffmpeg");
                    command.arg("-i").arg(&input_path).arg(&output_path);

                    if args.dry_run {
                        println!("Dry_run: Running '{:?}'", command);
                        println!("Dry_run: Removing file '{}'", input_path.display());
                    } else if let Err(e) =
                        command.output().map_err(anyhow::Error::new).and_then(|o| {
                            o.status
                                .success()
                                .then(|| std::fs::remove_file(input_path))
                                .map_or_else(
                                    || bail!(String::from_utf8_lossy(&o.stderr).into_owned()),
                                    |e| e.map_err(anyhow::Error::new),
                                )
                        })
                    {
                        error_count.fetch_add(1, Ordering::Relaxed);
                        println!("{}", e);
                    } else {
                        converted_count.fetch_add(1, Ordering::Relaxed);

                        println!(
                            "Finished converting '{}'",
                            current_dir
                                .as_ref()
                                .and_then(|p| pathdiff::diff_paths(&output_path, p))
                                .as_ref()
                                .unwrap_or(&output_path)
                                .display()
                        );
                    }

                    WalkState::Continue
                }
                Ok(_) => WalkState::Continue,
                Err(e) => {
                    error_count.fetch_add(1, Ordering::Relaxed);
                    println!("{}", e);
                    WalkState::Quit
                }
            })
        });

    println!(
        "Converted {} files.",
        converted_count.load(Ordering::Relaxed)
    );
    println!(
        "Finished with {} errors.",
        error_count.load(Ordering::Relaxed)
    );

    Ok(())
}
