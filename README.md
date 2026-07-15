# archiver

A TUI tool that simplifies creating archives. It allows to pick files from the root directory which need to be excluded/included/included and compressed.
User choise will be stored in a state file and loaded on the next launches.
It also has a feature of optimizing images (jpg, jpeg, png) by compressing them by similarities using x265 codec. Note that this feature requires ffmpeg and exiftool utilities available.

## Usage

1. Launch the program `archiver <dir to archive> <path to archive file>`.
2. Pick files to exclude/include/include and compress. Default is include and compress. Use the instructions on the screen.
3. If there are optimizible files in the archive, answer the prompted question, to optimize them or not.
4. Wait till the archive is created.

## Limitations (TODO)

- There is no ability to choose what image files user wants to optimize. Currently all included image files will be optimized.
- There is no progress bar or any other indication of the progress during the optimization and archiving.
- Currently all symlinks are followed. This is unsafe because it may cause an infinite loop. Also this may not be a user preference.
- Output to stdout is not supported. This is inconvenient for situations when further actions with the archive are required (gpg encryption, streaming through the network).
