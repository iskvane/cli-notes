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
# Create a note
cn add "Title" --body "Note content"

# List all notes
cn list

# Show a note
cn show 1

# Search notes
cn search Docker

# Delete a note
cn delete 1
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
