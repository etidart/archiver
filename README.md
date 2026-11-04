# archiver

A TUI tool that simplifies creating archives. It allows to pick files from the
root directory which need to be excluded/included/included and compressed.
User choice will be stored in a state file and loaded on the next launches.

## Usage

1. Launch the program `archiver <path to archive file> [dir to archive]`.
2. Pick files to exclude/include/include and compress. Default is include and compress. Use the instructions on the screen.
3. Wait till the archive is created.

## Limitations (TODO)

- There is no progress bar or any other indication of the progress during the archiving.
- Output to stdout is not supported. This is inconvenient for situations when further actions with the archive are required (gpg encryption, streaming through the network).
