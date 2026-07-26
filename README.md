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
# Create a note (the title is derived from the first line)
cargo run -- add "Note content"

# Create a note with an explicit title
cargo run -- add "Note content" --title "Title"

# `add` is implied when no subcommand is given
cargo run -- "Note content"

# List all notes
cargo run -- list

# Show a note
cargo run -- show 1

# Search notes
cargo run -- search Docker

# Delete a note
cargo run -- delete 1
```

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
