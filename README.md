# Notes CLI

A small command-line application for local notes, built with Rust.  
Notes are stored in a SQLite database.

## Requirements

- Rust (latest stable version)

## Run

```bash
cargo run -- --help
```

## Commands

```bash
# Create a note
cargo run -- add "Title" --body "Note content"

# List all notes
cargo run -- list

# Show a note
cargo run -- show 1

# Search notes
cargo run -- search Docker

# Replace the content of a note
cargo run -- edit 1 "New note content"

# Edit a note in your editor (saved when the editor is closed)
cargo run -- edit 1

# Delete a note
cargo run -- delete 1
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
