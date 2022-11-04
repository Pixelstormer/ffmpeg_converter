# ffmpeg_converter
An extremely simple wrapper application over ffmpeg for extremely simple mass file conversions.

## Runtime dependencies
You must have the [`ffmpeg`](https://ffmpeg.org/) binary in your PATH for this app to function.

## Usage
```
ffmpeg_converter 2.0.0
Recursively searches a given directory and its subdirectories for files with a given extension, and uses ffmpeg to convert those files to a different extension.

Effectively functions as a shorthand for the following shell commands:

`fd -e mp3 -x ffmpeg -i {} {.}.opus && fd -e mp3 -x rm`.

Usage: cv [OPTIONS] [INPUTS]... [-- <FFMPEG_ARGS>...]

Arguments:
  [INPUTS]...
          The file extensions to convert from

          [default: mp3]

  [FFMPEG_ARGS]...
          Extra arguments to be passed to ffmpeg during execution

Options:
  -d, --dry-run
          If set, prints information about actions that would be taken, instead of actually doing anything

  -v, --verbose
          If set, prints more messages about what is being done

  -q, --quiet
          If set, prints less messages about what is being done. Overrides the `--verbose` option

  -o, --output <OUTPUT>
          The output file extension to which files will be converted

          [default: opus]

  -t, --target-dir <TARGET_DIR>
          The directory to search in

          [default: ./]

  -m, --max-depth <MAX_DEPTH>
          The maximum search depth. If unset, is infinite

  -f, --follow-links
          If set, follows symbolic links

  -s, --same-fs
          If set, avoids crossing file system boundries when searching

  -n, --num-threads <NUM_THREADS>
          The number of threads to use for searching and processing files. If unset, defaults to the number of CPU cores

  -p, --preserve-files
          If set, does not delete files after successfully converting them

  -h, --help
          Print help information (use `-h` for a summary)

  -V, --version
          Print version information
```

## Examples
To recursively convert all `*.mp3` files in the current directory and any subdirectories to `*.opus` files:
```
cv
```

To recursively convert all `*.wav` and `*.aiff` files in the `Music/` directory and any subdirectories to `*.flac` files:
```
cv -o flac -t Music/ wav aiff
```

To convert all `*.mp4` files in the current directory only to HEVC-encoded `*.mkv` files:
```
cv -o mkv -m 1 mp4 -- -c:v libx265
```
