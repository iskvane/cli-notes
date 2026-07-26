use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use rusqlite::{params, Connection};
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "notes",
    version,
    about = "Eine kleine lokale Notes-CLI",
    args_conflicts_with_subcommands = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Inhalt einer neuen Notiz (implizites "add")
    #[arg(trailing_var_arg = true)]
    message: Vec<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Neue Notiz erstellen
    Add {
        /// Inhalt der Notiz
        #[arg(required = true, num_args = 1..)]
        message: Vec<String>,

        /// Titel der Notiz (Standard: erste Zeile des Inhalts)
        #[arg(short, long)]
        title: Option<String>,
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

    /// Notiz löschen
    Delete {
        id: i64,
    },
}

/// Maximale Länge eines automatisch abgeleiteten Titels.
const TITLE_MAX_LEN: usize = 50;

/// Leitet einen Titel aus der ersten Zeile des Inhalts ab.
fn derive_title(body: &str) -> String {
    let first_line = body.lines().next().unwrap_or("").trim();

    if first_line.chars().count() <= TITLE_MAX_LEN {
        return first_line.to_string();
    }

    let truncated: String = first_line.chars().take(TITLE_MAX_LEN - 1).collect();

    format!("{}…", truncated.trim_end())
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

    // Ohne Subcommand zählt ein freier Text als "add", alles andere zeigt die Hilfe.
    let command = match cli.command {
        Some(command) => command,
        None if !cli.message.is_empty() => Commands::Add {
            message: cli.message,
            title: None,
        },
        None => {
            Cli::command().print_help()?;
            return Ok(());
        }
    };

    let db_path = database_path()?;
    let conn = Connection::open(db_path)?;
    init_db(&conn)?;

    match command {
        Commands::Add { message, title } => {
            let body = message.join(" ");
            let title = title.unwrap_or_else(|| derive_title(&body));

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_title_uses_the_first_line() {
        assert_eq!(derive_title("Einkaufen\nMilch\nEier"), "Einkaufen");
    }

    #[test]
    fn derive_title_trims_whitespace() {
        assert_eq!(derive_title("  Einkaufen  \nMilch"), "Einkaufen");
    }

    #[test]
    fn derive_title_truncates_long_lines() {
        let title = derive_title(&"a".repeat(TITLE_MAX_LEN + 10));

        assert_eq!(title.chars().count(), TITLE_MAX_LEN);
        assert!(title.ends_with('…'));
    }

    #[test]
    fn derive_title_keeps_lines_at_the_limit_intact() {
        let body = "a".repeat(TITLE_MAX_LEN);

        assert_eq!(derive_title(&body), body);
    }

    #[test]
    fn derive_title_handles_an_empty_body() {
        assert_eq!(derive_title(""), "");
    }
}
