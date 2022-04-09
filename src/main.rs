use anyhow::anyhow;
use clap::Parser;
use ignore::{types::TypesBuilder, WalkBuilder, WalkState};
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU16, Ordering},
};

#[derive(Parser, Debug)]
#[clap(version = clap::crate_version!())]
struct Args {
    #[clap(short, long)]
    dry_run: bool,
    #[clap(default_value = "mp3")]
    from: String,
    #[clap(default_value = "opus")]
    to: String,
    #[clap(last = true, default_value = "./")]
    target_dir: PathBuf,
}

fn main() -> Result<(), anyhow::Error> {
    let args = Args::parse();

    let mut types = TypesBuilder::new();
    types.add("from", &format!("*.{}", args.from))?;
    types.add("to", &format!("*.{}", args.to))?;
    types.select("from").select("to");

    if args.dry_run {
        println!("Dry-run enabled");
    }

    println!("Converting files from '{}' to '{}'", args.from, args.to);

    let error_count = AtomicU16::new(0);
    let converted_count = AtomicU16::new(0);

    WalkBuilder::new(args.target_dir)
        .standard_filters(false)
        .types(types.build()?)
        .build_parallel()
        .run(|| {
            Box::new(|path| match path {
                Ok(dir) if dir.file_type().map_or(false, |f| f.is_file()) => {
                    let path = dir.path();
                    println!("Converting '{}'", path.display());

                    if let Err(e @ anyhow::Error { .. }) = (|| {
                        let from_path = path.to_str().ok_or_else(|| {
                            anyhow!("Input path '{}' is not valid unicode", path.display())
                        })?;

                        let path = path.with_extension(&args.to);
                        let to_path = path.to_str().ok_or_else(|| {
                            anyhow!("Output path '{}' is not valid unicode", path.display())
                        })?;

                        let mut command = Command::new("ffmpeg");
                        command.args(["-i", from_path, to_path]);
                        if args.dry_run {
                            println!("Dry_run: Running '{:?}'", command);
                            println!("Dry_run: Removing file '{}'", from_path);
                        } else {
                            command.output()?;
                            std::fs::remove_file(from_path)?;
                        }

                        converted_count.fetch_add(1, Ordering::Relaxed);
                        Ok(())
                    })() {
                        error_count.fetch_add(1, Ordering::Relaxed);
                        println!("{}", e);
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
