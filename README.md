# archiver

A TUI tool that simplifies creating archives. It allows to pick files from the
root directory which need to be excluded/included/included and compressed.
User choice will be stored in a state file and loaded on the next launches.

## Usage

Create an archive from the current directory:

```text
archiver archive.tar
```

Create an archive from another directory:

```text
archiver archive.tar path/to/directory
```

Pick files to include or compress in the TUI. The default action is compress.

The same utility can inspect and extract archives:

```text
archiver --list archive.tar
archiver --list archive.tar path/to/file
archiver --extract archive.tar
archiver --extract --directory destination archive.tar path/to/file
```

With `--extract`, omitting paths extracts every file; supplying paths extracts
only matching files and descendants. `--list` prints matching paths without
extracting them.

## Limitations (TODO)

- There is no progress bar or any other indication of the progress during the archiving.
- Output to stdout is not supported. This is inconvenient for situations when further actions with the archive are required (gpg encryption, streaming through the network).
