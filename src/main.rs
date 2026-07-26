use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use rusqlite::{params, Connection, OptionalExtension};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser)]
#[command(name = "notes", version, about = "Eine kleine lokale Notes-CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Neue Notiz erstellen
    Add {
        title: String,

        #[arg(short, long)]
        body: String,
    },

    /// Alle Notizen auflisten
    List,

    /// Einzelne Notiz anzeigen
    Show {
        id: i64,
    },

    /// In Titel und Inhalt suchen
    Search {
        query: String,
    },

    /// Inhalt einer Notiz ersetzen; ohne Nachricht öffnet der konfigurierte Editor
    Edit {
        id: i64,

        /// Neuer Inhalt der Notiz
        #[arg(num_args = 1..)]
        message: Vec<String>,
    },

    /// Notiz löschen
    Delete {
        id: i64,
    },
}

/// Liefert den konfigurierten Editor aus $VISUAL bzw. $EDITOR.
fn configured_editor() -> String {
    for key in ["VISUAL", "EDITOR"] {
        if let Ok(editor) = std::env::var(key) {
            if !editor.trim().is_empty() {
                return editor;
            }
        }
    }

    if cfg!(windows) {
        "notepad".to_string()
    } else {
        "vi".to_string()
    }
}

/// Öffnet den Text im konfigurierten Editor und gibt den gespeicherten Stand zurück.
fn edit_in_editor(initial: &str) -> Result<String> {
    let editor = configured_editor();

    let mut parts = editor.split_whitespace();
    let program = parts
        .next()
        .context("Der konfigurierte Editor ist leer.")?;

    let path = std::env::temp_dir().join(format!("notes-{}.txt", std::process::id()));
    fs::write(&path, initial)?;

    let status = Command::new(program)
        .args(parts)
        .arg(&path)
        .status()
        .with_context(|| format!("Editor \"{editor}\" konnte nicht gestartet werden."))?;

    if !status.success() {
        let _ = fs::remove_file(&path);
        bail!("Editor \"{editor}\" wurde mit einem Fehler beendet. Notiz bleibt unverändert.");
    }

    let edited = fs::read_to_string(&path)?;
    let _ = fs::remove_file(&path);

    // Editoren hängen beim Speichern gerne einen Zeilenumbruch an.
    Ok(edited.trim_end_matches(['\r', '\n']).to_string())
}

fn database_path() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))?;

    let data_dir = PathBuf::from(home).join(".notes");
    fs::create_dir_all(&data_dir)?;

    Ok(data_dir.join("notes.db"))
}

fn init_db(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS notes (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            title       TEXT NOT NULL,
            body        TEXT NOT NULL,
            created_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TRIGGER IF NOT EXISTS notes_update_timestamp
        AFTER UPDATE ON notes
        FOR EACH ROW
        BEGIN
            UPDATE notes
            SET updated_at = CURRENT_TIMESTAMP
            WHERE id = OLD.id;
        END;
        "#,
    )?;

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let db_path = database_path()?;
    let conn = Connection::open(db_path)?;
    init_db(&conn)?;

    match cli.command {
        Commands::Add { title, body } => {
            conn.execute(
                "INSERT INTO notes (title, body) VALUES (?1, ?2)",
                params![title, body],
            )?;

            println!("Notiz gespeichert (ID: {}).", conn.last_insert_rowid());
        }

        Commands::List => {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, title, created_at
                FROM notes
                ORDER BY updated_at DESC
                "#,
            )?;

            let notes = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?;

            for note in notes {
                let (id, title, created_at) = note?;
                println!("{id:>4}  {created_at}  {title}");
            }
        }

        Commands::Show { id } => {
            let mut stmt = conn.prepare(
                "SELECT id, title, body, created_at, updated_at FROM notes WHERE id = ?1",
            )?;

            let note = stmt.query_row(params![id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            });

            match note {
                Ok((id, title, body, created_at, updated_at)) => {
                    println!("#{id}: {title}");
                    println!("Erstellt: {created_at}");
                    println!("Geändert: {updated_at}");
                    println!("\n{body}");
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    eprintln!("Keine Notiz mit ID {id} gefunden.");
                }
                Err(error) => return Err(error.into()),
            }
        }

        Commands::Search { query } => {
            let pattern = format!("%{query}%");

            let mut stmt = conn.prepare(
                r#"
                SELECT id, title, created_at
                FROM notes
                WHERE title LIKE ?1 OR body LIKE ?1
                ORDER BY updated_at DESC
                "#,
            )?;

            let notes = stmt.query_map(params![pattern], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?;

            for note in notes {
                let (id, title, created_at) = note?;
                println!("{id:>4}  {created_at}  {title}");
            }
        }

        Commands::Edit { id, message } => {
            let body = if message.is_empty() {
                let current: Option<String> = conn
                    .query_row("SELECT body FROM notes WHERE id = ?1", params![id], |row| {
                        row.get(0)
                    })
                    .optional()?;

                let Some(current) = current else {
                    eprintln!("Keine Notiz mit ID {id} gefunden.");
                    return Ok(());
                };

                let edited = edit_in_editor(&current)?;

                if edited == current {
                    println!("Notiz {id} unverändert.");
                    return Ok(());
                }

                edited
            } else {
                message.join(" ")
            };

            let count = conn.execute(
                "UPDATE notes SET body = ?2 WHERE id = ?1",
                params![id, body],
            )?;

            if count == 0 {
                eprintln!("Keine Notiz mit ID {id} gefunden.");
            } else {
                println!("Notiz {id} aktualisiert.");
            }
        }

        Commands::Delete { id } => {
            let count = conn.execute("DELETE FROM notes WHERE id = ?1", params![id])?;

            if count == 0 {
                eprintln!("Keine Notiz mit ID {id} gefunden.");
            } else {
                println!("Notiz {id} gelöscht.");
            }
        }
    }

    Ok(())
}
