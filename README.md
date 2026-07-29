# Notes CLI

A small command-line application for local notes, built with Rust.  
Notes are stored in a SQLite database.

## Requirements

- Rust (latest stable version)

## Run

```bash
cargo run -- --help
```

Once installed, the application is called as `cn`.

## Commands

```bash
# Create a note (the title is derived from the first line)
cn add "Note content"

# Create a note with an explicit title
cn add "Note content" --title "Title"

# `add` is implied when no subcommand is given
cn "Note content"

# List all notes
cn list

# Show a note
cn show 1

# Search notes
cn search Docker

# Replace the content of a note
cn edit 1 "New note content"

# Edit a note in your editor (saved when the editor is closed)
cn edit 1

# Change the title of a note
cn edit-title 1 "New title"

# Delete a note
cn delete 1
```

## Piping

`show`, `list` and `search` detect whether stdout is a terminal. In a terminal
they print the usual decorated output; in a pipe they switch to raw output, so
the result can be fed straight into other tools:

```bash
# Only the note body, no header lines
cn show 1 | grep TODO

# Tab-separated columns (id, created_at, title)
cn list | cut -f3
cn search Docker | awk -F'\t' '{ print $1 }'
```

Both flags override the detection:

```bash
# Force raw output, even in a terminal
cn show 1 --raw

# Keep headers and alignment, even in a pipe
cn show 1 --no-raw | less
```

## Editor

`edit <id>` without a message opens the note in the editor from `$VISUAL`,
falling back to `$EDITOR`. If neither is set, `notepad` is used on Windows and
`vi` elsewhere. The note is saved when the editor exits; a non-zero exit status
leaves the note untouched.

## Storage

The SQLite database is stored locally at:

```text
~/.notes/notes.db
```

On Windows, it is stored in the user directory, for example:

```text
C:\Users\<Name>\.notes\notes.db
```

## Stack

- Rust
- SQLite
- clap
- rusqlite
