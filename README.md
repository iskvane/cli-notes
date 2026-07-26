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

# Delete a note
cn delete 1
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
