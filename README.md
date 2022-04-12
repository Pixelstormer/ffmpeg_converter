# ffmpeg_converter
An extremely simple wrapper application over ffmpeg for extremely simple mass file conversions.

## Runtime dependencies
You must have the [`ffmpeg`](https://ffmpeg.org/) binary in your PATH for this app to function.

## Usage
```
ffmpeg_converter 1.0.0
Recursively searches a given directory and its subdirectories for files with a given extension, and
uses ffmpeg to convert those files to a different extension.

Effectively functions as a shorthand for the following shell commands:

`fd -e mp3 -x ffmpeg -i {} {.}.opus && fd -e mp3 -x rm`.

USAGE:
    ffmpeg_converter [OPTIONS] [FROM] [TO] [-- <TARGET_DIR>]

ARGS:
    <FROM>
            The file extension to convert from

            [default: mp3]

    <TO>
            The file extension to convert to

            [default: opus]

    <TARGET_DIR>
            The directory to search in

            [default: ./]

OPTIONS:
    -d, --dry-run
            If true, prints information about actions that would be taken, instead of actually doing
            anything

    -h, --help
            Print help information

    -V, --version
            Print version information
```

## Examples
To recursively convert all `*.mp3` files in the current directory and any subdirectories to `*.opus` files:
```
cv
```

To recursively convert all `*.wav` files in the `Music/` directory and any subdirectories to `*.flac` files:
```
cv wav flac -- Music/
```
