use anyhow::{Context, Result, bail};
use clap::{Args, CommandFactory, Parser, Subcommand};
use rusqlite::{Connection, OptionalExtension, params};
use std::fs;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser)]
#[command(
    name = "cn",
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
    List {
        #[command(flatten)]
        output: OutputArgs,
    },

    /// Einzelne Notiz anzeigen
    Show {
        id: i64,

        #[command(flatten)]
        output: OutputArgs,
    },

    /// In Titel und Inhalt suchen
    Search {
        query: String,

        #[command(flatten)]
        output: OutputArgs,
    },

    /// Inhalt einer Notiz ersetzen; ohne Nachricht öffnet der konfigurierte Editor
    Edit {
        id: i64,

        /// Neuer Inhalt der Notiz
        #[arg(num_args = 1..)]
        message: Vec<String>,
    },

    /// Titel einer Notiz ändern
    EditTitle {
        id: i64,

        /// Neuer Titel der Notiz
        #[arg(required = true, num_args = 1..)]
        title: Vec<String>,
    },

    /// Notiz löschen
    Delete { id: i64 },
}

/// Steuert, ob die Ausgabe für Menschen oder für Pipes gedacht ist.
#[derive(Args)]
struct OutputArgs {
    /// Nur die Nutzdaten ausgeben, ohne Kopfzeilen und Ausrichtung
    #[arg(long)]
    raw: bool,

    /// Kopfzeilen erzwingen, auch in einer Pipe
    #[arg(long, conflicts_with = "raw")]
    no_raw: bool,
}

impl OutputArgs {
    /// Ohne Flag entscheidet das Ziel der Ausgabe: Terminal schmückt, Pipe nicht.
    fn raw(&self) -> bool {
        resolve_raw(self.raw, self.no_raw, std::io::stdout().is_terminal())
    }
}

/// Löst die beiden Flags gegen die Terminal-Erkennung auf.
fn resolve_raw(raw: bool, no_raw: bool, stdout_is_terminal: bool) -> bool {
    if raw {
        true
    } else if no_raw {
        false
    } else {
        !stdout_is_terminal
    }
}

/// Gibt eine Zeile der Notizliste aus; roh mit Tabs, sonst ausgerichtet.
fn print_note_row(id: i64, created_at: &str, title: &str, raw: bool) {
    if raw {
        println!("{id}\t{created_at}\t{title}");
    } else {
        println!("{id:>4}  {created_at}  {title}");
    }
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
    let program = parts.next().context("Der konfigurierte Editor ist leer.")?;

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
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE"))?;

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

        Commands::List { output } => {
            let raw = output.raw();

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
                print_note_row(id, &created_at, &title, raw);
            }
        }

        Commands::Show { id, output } => {
            let raw = output.raw();

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
                    if !raw {
                        println!("#{id}: {title}");
                        println!("Erstellt: {created_at}");
                        println!("Geändert: {updated_at}");
                        println!();
                    }

                    println!("{body}");
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    eprintln!("Keine Notiz mit ID {id} gefunden.");
                }
                Err(error) => return Err(error.into()),
            }
        }

        Commands::Search { query, output } => {
            let raw = output.raw();
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
                print_note_row(id, &created_at, &title, raw);
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

        Commands::EditTitle { id, title } => {
            let title = title.join(" ");

            let count = conn.execute(
                "UPDATE notes SET title = ?2 WHERE id = ?1",
                params![id, title],
            )?;

            if count == 0 {
                eprintln!("Keine Notiz mit ID {id} gefunden.");
            } else {
                println!("Titel von Notiz {id} aktualisiert.");
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

    #[test]
    fn raw_defaults_to_the_output_target() {
        assert!(resolve_raw(false, false, false));
        assert!(!resolve_raw(false, false, true));
    }

    #[test]
    fn raw_flags_beat_the_terminal_detection() {
        assert!(resolve_raw(true, false, true));
        assert!(!resolve_raw(false, true, false));
    }

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
