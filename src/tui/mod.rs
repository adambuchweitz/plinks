pub mod editor;
pub mod events;
pub mod state;
pub mod view;

use std::fs;
use std::io;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::clipboard::LinkClipboard;
use crate::config::{Config, load_existing, write_config};
use crate::open_link::LinkOpener;
use crate::project_root::ResolvedConfigPath;

use self::events::{EventResult, handle_key};
use self::state::{App, Mode};

pub fn run(
    resolved: ResolvedConfigPath,
    opener: &dyn LinkOpener,
    clipboard: &dyn LinkClipboard,
) -> Result<()> {
    let loaded = load_or_default(&resolved)?;
    let mut app = App::new(loaded.config);
    let mut snapshot = loaded.raw;

    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(
        &mut terminal,
        &resolved,
        opener,
        clipboard,
        &mut app,
        &mut snapshot,
    );

    disable_raw_mode().ok();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    resolved: &ResolvedConfigPath,
    opener: &dyn LinkOpener,
    clipboard: &dyn LinkClipboard,
    app: &mut App,
    snapshot: &mut Option<String>,
) -> Result<()> {
    loop {
        terminal.draw(|frame| view::render(frame, app, resolved))?;

        if !event::poll(Duration::from_millis(200))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };

        match handle_key(app, key)? {
            EventResult::None => {}
            EventResult::Quit => return Ok(()),
            EventResult::Reload => {
                reload_from_disk(app, resolved, snapshot)?;
            }
            EventResult::OpenSelected => {
                if let Err(err) = open_selected(app, opener) {
                    app.set_error(err.to_string());
                }
            }
            EventResult::YankSelected => {
                if let Err(err) = yank_selected(app, clipboard) {
                    app.set_error(err.to_string());
                }
            }
            EventResult::SaveEditor(editor) => match editor.build_candidate() {
                Ok(candidate) => {
                    let original = editor.original_primary.as_deref();
                    if let Err(err) = persist_edit(app, resolved, snapshot, original, candidate) {
                        app.set_error(err.to_string());
                        if let Mode::Editor(state) = &mut app.mode {
                            state.error = Some(err.to_string());
                        }
                    } else {
                        app.mode = Mode::Normal;
                    }
                }
                Err(err) => {
                    if let Mode::Editor(state) = &mut app.mode {
                        state.error = Some(err.to_string());
                    }
                }
            },
            EventResult::ConfirmDelete => {
                let Some(primary) = app.selected_primary() else {
                    app.set_error("No link selected");
                    continue;
                };
                if let Err(err) = persist_delete(app, resolved, snapshot, &primary) {
                    app.set_error(err.to_string());
                } else {
                    app.mode = Mode::Normal;
                }
            }
        }
    }
}

fn open_selected(app: &mut App, opener: &dyn LinkOpener) -> Result<()> {
    let primary = app.selected_primary().context("No link selected")?;
    let entry = app
        .config
        .links
        .get(&primary)
        .with_context(|| format!("selected link '{primary}' no longer exists"))?;
    opener.open(&entry.url)?;
    app.set_info(format!("Opened '{primary}'"));
    Ok(())
}

fn yank_selected(app: &mut App, clipboard: &dyn LinkClipboard) -> Result<()> {
    let primary = app.selected_primary().context("No link selected")?;
    let entry = app
        .config
        .links
        .get(&primary)
        .with_context(|| format!("selected link '{primary}' no longer exists"))?;
    clipboard.copy_text(&entry.url)?;
    app.set_info(format!("Copied URL for '{primary}'"));
    Ok(())
}

fn load_or_default(resolved: &ResolvedConfigPath) -> Result<LoadedSnapshot> {
    match load_existing(&resolved.config_path)? {
        Some(document) => Ok(LoadedSnapshot {
            config: document.config,
            raw: Some(document.raw),
        }),
        None => Ok(LoadedSnapshot {
            config: Config::default(),
            raw: None,
        }),
    }
}

fn reload_from_disk(
    app: &mut App,
    resolved: &ResolvedConfigPath,
    snapshot: &mut Option<String>,
) -> Result<()> {
    let loaded = load_or_default(resolved)?;
    app.config = loaded.config;
    app.ensure_selection();
    app.mode = Mode::Normal;
    app.set_info("Reloaded from disk");
    *snapshot = loaded.raw;
    Ok(())
}

fn persist_edit(
    app: &mut App,
    resolved: &ResolvedConfigPath,
    snapshot: &mut Option<String>,
    original_primary: Option<&str>,
    candidate: crate::config::CandidateLink,
) -> Result<()> {
    let current = current_disk_state(&resolved.config_path)?;
    if current != *snapshot {
        reload_from_disk(app, resolved, snapshot)?;
        anyhow::bail!("config changed on disk; reloaded current contents");
    }

    let mut next = app.config.clone();
    next.save_link(original_primary, candidate)?;
    let written = write_config(&resolved.config_path, &next)?;
    app.config = next;
    app.ensure_selection();
    app.set_info("Saved changes");
    *snapshot = Some(written);
    Ok(())
}

fn persist_delete(
    app: &mut App,
    resolved: &ResolvedConfigPath,
    snapshot: &mut Option<String>,
    primary: &str,
) -> Result<()> {
    let current = current_disk_state(&resolved.config_path)?;
    if current != *snapshot {
        reload_from_disk(app, resolved, snapshot)?;
        anyhow::bail!("config changed on disk; reloaded current contents");
    }

    let mut next = app.config.clone();
    next.links
        .remove(primary)
        .with_context(|| format!("primary alias '{primary}' no longer exists"))?;
    let written = write_config(&resolved.config_path, &next)?;
    app.config = next;
    app.ensure_selection();
    app.set_info(format!("Deleted '{primary}'"));
    *snapshot = Some(written);
    Ok(())
}

fn current_disk_state(path: &std::path::Path) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(fs::read_to_string(path).with_context(|| {
        format!("failed to read {}", path.display())
    })?))
}

struct LoadedSnapshot {
    config: Config,
    raw: Option<String>,
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::clipboard::LinkClipboard;
    use crate::config::CandidateLink;
    use crate::open_link::LinkOpener;

    #[derive(Default)]
    struct RecordingClipboard {
        copied: RefCell<Vec<String>>,
    }

    impl LinkClipboard for RecordingClipboard {
        fn copy_text(&self, text: &str) -> Result<()> {
            self.copied.borrow_mut().push(text.to_string());
            Ok(())
        }
    }

    impl RecordingClipboard {
        fn copied(&self) -> Vec<String> {
            self.copied.borrow().clone()
        }
    }

    #[derive(Default)]
    struct RecordingOpener {
        opened: RefCell<Vec<String>>,
    }

    impl LinkOpener for RecordingOpener {
        fn open(&self, url: &str) -> Result<()> {
            self.opened.borrow_mut().push(url.to_string());
            Ok(())
        }
    }

    impl RecordingOpener {
        fn opened(&self) -> Vec<String> {
            self.opened.borrow().clone()
        }
    }

    #[test]
    fn save_conflict_reloads_on_disk_state() {
        let temp = TempDir::new().unwrap();
        let config_path = temp.path().join("project-links.toml");
        let resolved = ResolvedConfigPath {
            project_dir: temp.path().to_path_buf(),
            config_path: config_path.clone(),
            git_root: None,
        };

        let mut initial = Config::default();
        initial
            .save_link(
                None,
                CandidateLink::new(
                    "docs".into(),
                    "https://docs.rs".into(),
                    vec![],
                    vec![],
                    None,
                )
                .unwrap(),
            )
            .unwrap();
        let initial_text = write_config(&config_path, &initial).unwrap();

        let mut app = App::new(initial);
        let mut snapshot = Some(initial_text);

        let mut external = Config::default();
        external
            .save_link(
                None,
                CandidateLink::new(
                    "jira".into(),
                    "https://jira.example".into(),
                    vec![],
                    vec![],
                    None,
                )
                .unwrap(),
            )
            .unwrap();
        let external_text = write_config(&config_path, &external).unwrap();

        let err = persist_edit(
            &mut app,
            &resolved,
            &mut snapshot,
            None,
            CandidateLink::new(
                "db".into(),
                "postgres://localhost:5432/app".into(),
                vec![],
                vec![],
                None,
            )
            .unwrap(),
        )
        .unwrap_err();

        assert!(err.to_string().contains("changed on disk"));
        assert!(app.config.links.contains_key("jira"));
        assert_eq!(snapshot, Some(external_text));
        assert_eq!(
            fs::read_to_string(&config_path).unwrap(),
            snapshot.clone().unwrap()
        );
    }

    #[test]
    fn yank_selected_copies_selected_url() {
        let mut config = Config::default();
        config
            .save_link(
                None,
                CandidateLink::new(
                    "docs".into(),
                    "https://docs.rs".into(),
                    vec![],
                    vec![],
                    None,
                )
                .unwrap(),
            )
            .unwrap();

        let mut app = App::new(config);
        let clipboard = RecordingClipboard::default();

        yank_selected(&mut app, &clipboard).unwrap();

        assert_eq!(clipboard.copied(), vec!["https://docs.rs".to_string()]);
        assert_eq!(
            app.status.as_ref().map(|status| status.text.as_str()),
            Some("Copied URL for 'docs'")
        );
    }

    #[test]
    fn open_selected_opens_selected_url() {
        let mut config = Config::default();
        config
            .save_link(
                None,
                CandidateLink::new(
                    "docs".into(),
                    "https://docs.rs".into(),
                    vec![],
                    vec![],
                    None,
                )
                .unwrap(),
            )
            .unwrap();

        let mut app = App::new(config);
        let opener = RecordingOpener::default();

        open_selected(&mut app, &opener).unwrap();

        assert_eq!(opener.opened(), vec!["https://docs.rs".to_string()]);
        assert_eq!(
            app.status.as_ref().map(|status| status.text.as_str()),
            Some("Opened 'docs'")
        );
    }
}
