//! Running one named story either on a test screen or a real terminal.

use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use file_types::{CommitHash, File, RepoPath, Revs};
use loom::crokey::KeyCombination;
use loom::ratatui::buffer::Buffer;
use loom::ratatui::layout::Rect;
use loom::testing::Harness;
use loom::{Flow, Tree, deliver_input};
use ui::Theme;
use ui::components::Context as UiContext;
use ui::services::{DiffService, SyntaxService, VcsService};

#[cfg(test)]
use super::catalog;
use super::definition::{LiveRepo, StoryDefinition, StoryFixture};
use super::preview::{PreviewAction, StoryPreview, StoryPreviewProps};

enum StoryEvent {
    Terminal(ui::crossterm::event::Event),
    #[cfg(unix)]
    Signal(i32),
    Redraw,
}

pub(super) struct StoryServices {
    pub vcs: Rc<VcsService>,
    pub diff: Rc<DiffService>,
    pub syntax: Rc<SyntaxService>,
}

impl StoryServices {
    pub fn is_busy(&self) -> bool {
        self.vcs.is_busy() || self.diff.is_busy() || self.syntax.is_busy()
    }
}

/// Services that read the story's fixture instead of Git, and the props
/// that show it. Code stories open their files at once.
pub(super) fn story_props(
    definition: &'static StoryDefinition,
    fixture: StoryFixture,
    redraw: impl Fn() + Send + Sync + 'static,
    navigate: Option<Rc<dyn Fn(PreviewAction)>>,
) -> (StoryServices, StoryPreviewProps, Option<LiveRepo>) {
    let redraw: Arc<dyn Fn() + Send + Sync> = Arc::new(redraw);
    if let StoryFixture::Repository {
        repo,
        revision_range,
    } = fixture
    {
        let vcs_redraw = Arc::clone(&redraw);
        let vcs = Rc::new(VcsService::start(move || vcs_redraw()));
        let diff_redraw = Arc::clone(&redraw);
        let diff = Rc::new(DiffService::start(
            move || diff_redraw(),
            diff::Settings::default(),
        ));
        let syntax_redraw = Arc::clone(&redraw);
        let syntax = Rc::new(SyntaxService::start(move || syntax_redraw()));
        let props = StoryPreviewProps {
            definition,
            base_context: UiContext {
                theme: Rc::new(Theme::DARK),
                repo: Rc::from(repo.path()),
                revision_range,
                vcs: Some(Rc::clone(&vcs)),
                diff: Some(Rc::clone(&diff)),
                syntax: Some(Rc::clone(&syntax)),
                ..UiContext::default()
            },
            files: Rc::new(Vec::new()),
            navigate,
        };
        let services = StoryServices { vcs, diff, syntax };
        return (services, props, Some(repo));
    }

    let (listed, contents, commits, picking, branch_drawing, open) = match fixture {
        StoryFixture::Welcome => (None, Vec::new(), Vec::new(), None, false, false),
        StoryFixture::Explorer(files) => (Some(files), Vec::new(), Vec::new(), None, false, false),
        StoryFixture::ExplorerCards { files, diffs } => {
            (Some(files), diffs, Vec::new(), None, false, false)
        }
        StoryFixture::Repository { .. } => unreachable!("handled above"),
        StoryFixture::RevisionPicker {
            commits,
            worktree_files,
            branch_drawing,
        } => {
            let files: Vec<File> = (1..=worktree_files)
                .map(|i| {
                    File::unchanged_path(
                        RepoPath::new(format!("src/file_{i}.rs"), Path::new("/codediff")),
                        Revs::worktree_against(CommitHash::new("base")),
                    )
                })
                .collect();
            (
                Some(files),
                Vec::new(),
                commits,
                None,
                branch_drawing,
                false,
            )
        }
        StoryFixture::RevisionHeader { commits, active } => {
            (None, Vec::new(), commits, active, false, false)
        }
        StoryFixture::SideBySide(content)
        | StoryFixture::Inline(content)
        | StoryFixture::SingleFile(content)
        | StoryFixture::Card(content) => (
            None,
            vec![(*content).clone()],
            Vec::new(),
            None,
            false,
            true,
        ),
        StoryFixture::Cards(contents) => (
            None,
            contents.iter().map(|content| (**content).clone()).collect(),
            Vec::new(),
            None,
            false,
            true,
        ),
    };
    let opened: Vec<_> = if open {
        contents
            .iter()
            .map(|content| content.file().clone())
            .collect()
    } else {
        Vec::new()
    };
    let vcs_redraw = Arc::clone(&redraw);
    let vcs = Rc::new(VcsService::fake(
        move || vcs_redraw(),
        move |_, _| Ok(listed.clone().unwrap_or_default()),
        move |_, limit| Ok(commits.iter().take(limit).cloned().collect()),
    ));
    let diff_redraw = Arc::clone(&redraw);
    let diff = Rc::new(DiffService::fake(
        move || diff_redraw(),
        move |file| {
            contents
                .iter()
                .find(|content| content.file() == file)
                .cloned()
                .ok_or_else(|| format!("the story has no diff for {}", file.path()))
        },
    ));
    let syntax_redraw = Arc::clone(&redraw);
    let syntax = Rc::new(SyntaxService::start(move || syntax_redraw()));
    let files = opened.into_iter().map(|file| diff.load(file)).collect();
    let props = StoryPreviewProps {
        definition,
        base_context: UiContext {
            theme: Rc::new(Theme::DARK),
            branch_drawing,
            repo: Rc::from(Path::new("/codediff")),
            picking,
            vcs: Some(Rc::clone(&vcs)),
            diff: Some(Rc::clone(&diff)),
            syntax: Some(Rc::clone(&syntax)),
            ..UiContext::default()
        },
        files: Rc::new(files),
        navigate,
    };
    let services = StoryServices { vcs, diff, syntax };
    (services, props, None)
}

struct StoryHost {
    definition: &'static StoryDefinition,
    events_tx: Sender<StoryEvent>,
    events_rx: Receiver<StoryEvent>,
    services: StoryServices,
    _repo: Option<LiveRepo>,
}

impl StoryHost {
    fn new(definition: &'static StoryDefinition) -> Result<(Self, StoryPreviewProps)> {
        let fixture = definition.create_fixture()?;
        let (events_tx, events_rx) = mpsc::channel();
        let redraw = events_tx.clone();
        let (services, props, repo) = story_props(
            definition,
            fixture,
            move || {
                let _ = redraw.send(StoryEvent::Redraw);
            },
            None,
        );
        let host = Self {
            definition,
            events_tx,
            events_rx,
            services,
            _repo: repo,
        };
        Ok((host, props))
    }

    /// Draws, presses the story's keys, and draws until nothing is loading.
    fn prepare(&self, mut draw: impl FnMut(Option<KeyCombination>)) -> Result<()> {
        self.settle(&mut draw)?;
        for &key in self.definition.initial_keys {
            draw(Some(key));
        }
        self.settle(&mut draw)
    }

    fn settle(&self, draw: &mut impl FnMut(Option<KeyCombination>)) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let idle = !self.services.is_busy();
            while self.events_rx.try_recv().is_ok() {}
            draw(None);
            // A job asks for a redraw before it stops counting as busy, so
            // work that finished during the drawing has left a request behind.
            if idle && !self.services.is_busy() && self.events_rx.try_recv().is_err() {
                return Ok(());
            }
            if Instant::now() > deadline {
                bail!("{} never finished loading", self.definition.id);
            }
            let _ = self.events_rx.recv_timeout(Duration::from_millis(20));
        }
    }
}

pub fn snapshot(
    definition: &'static StoryDefinition,
    width: u16,
    height: u16,
) -> Result<Vec<String>> {
    let mut harness = story_harness(definition, width, height)?;
    Ok(harness.screen())
}

fn story_harness(definition: &'static StoryDefinition, width: u16, height: u16) -> Result<Harness> {
    validate_dimensions(width, height)?;
    let (host, props) = StoryHost::new(definition)?;
    let mut harness = Harness::new::<StoryPreview>(props, width, height);
    host.prepare(|key| {
        if let Some(key) = key {
            harness.press(key);
        }
        harness.redraw_all();
        settle_harness(&mut harness);
    })?;
    Ok(harness)
}

pub fn run(definition: &'static StoryDefinition) -> Result<()> {
    let (host, props) = StoryHost::new(definition)?;
    let mut tree = Tree::new::<StoryPreview>(props);
    host.prepare(|key| {
        if let Some(key) = key {
            tree.press(key);
        }
        tree.redraw_all();
        settle_story_tree(&mut tree);
    })?;

    #[cfg(unix)]
    ui::spawn_signals(host.events_tx.clone(), StoryEvent::Signal);

    loom::run(
        &mut tree,
        host.events_rx,
        host.events_tx,
        StoryEvent::Terminal,
        move |event, tree| {
            match event {
                StoryEvent::Terminal(event) => deliver_input(tree, &event),
                #[cfg(unix)]
                StoryEvent::Signal(signal) => {
                    loom::restore();
                    std::process::exit(128 + signal);
                }
                StoryEvent::Redraw => tree.redraw_all(),
            }
            Flow::Continue
        },
    )?;
    Ok(())
}

pub(super) fn settle_story_tree(tree: &mut Tree) {
    let area = Rect::new(0, 0, 100, 24);
    for _ in 0..4 {
        let mut cells = Buffer::empty(area);
        tree.draw(&mut cells, area);
    }
}

fn settle_harness(harness: &mut Harness) {
    for _ in 0..4 {
        harness.force_draw();
    }
}

fn validate_dimensions(width: u16, height: u16) -> Result<()> {
    if width == 0 || height < 2 {
        bail!("a UI story needs a non-zero width and at least two lines");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_story_preselects_a_file() {
        let story = catalog::by_id("explorer/selected").unwrap();
        let mut harness = story_harness(story, 100, 24).unwrap();
        let selected = Theme::DARK.normal.patch(Theme::DARK.cursor_line).bg;

        assert_eq!(harness.style_at(0, 5).bg, selected);
        assert!(harness.screen_line(5).contains("button.rs"));
    }

    #[test]
    fn revision_picker_search_story_filters_tags_via_initial_keys() {
        let story = catalog::by_id("revision-picker/search").unwrap();
        let mut harness = story_harness(story, 110, 24).unwrap();
        let selected = Theme::DARK.normal.patch(Theme::DARK.cursor_line).bg;

        assert!(harness.screen_line(3).contains("> v0.28"));
        assert_eq!(harness.style_at(0, 4).bg, selected);
        assert!(harness.screen_line(4).contains("tag: v0.28.4"));
    }

    #[test]
    fn every_nonempty_code_story_waits_for_syntax_colours() {
        use super::super::definition::StoryComponent;

        for story in catalog::stories().filter(|story| {
            matches!(
                story.component,
                StoryComponent::SideBySide
                    | StoryComponent::Inline
                    | StoryComponent::SingleFile
                    | StoryComponent::Card
                    | StoryComponent::Cards
                    | StoryComponent::ExplorerCards
            ) && story.id != "single-file/empty"
        }) {
            let mut harness = story_harness(story, 100, 24).unwrap();
            let normal = Theme::DARK.normal.fg;
            let cells = harness.cells();
            let coloured = |start: u16, end: u16| {
                (2..24).any(|y| {
                    (start..end).any(|x| {
                        cells.cell((x, y)).is_some_and(|cell| {
                            cell.symbol().chars().any(char::is_alphabetic)
                                && cell.style().fg != normal
                        })
                    })
                })
            };
            match story.component {
                StoryComponent::SideBySide => {
                    let divider = (0..100)
                        .find(|&x| cells.cell((x, 2)).is_some_and(|cell| cell.symbol() == "│"))
                        .expect("side-by-side divider");
                    assert!(coloured(0, divider), "{} left side has no syntax", story.id);
                    assert!(
                        coloured(divider + 1, 100),
                        "{} right side has no syntax",
                        story.id
                    );
                }
                StoryComponent::Inline
                | StoryComponent::SingleFile
                | StoryComponent::Card
                | StoryComponent::Cards
                | StoryComponent::ExplorerCards => {
                    assert!(coloured(0, 100), "{} had no syntax-coloured text", story.id);
                }
                StoryComponent::Welcome
                | StoryComponent::Explorer
                | StoryComponent::RevisionPicker
                | StoryComponent::RevisionHeader => unreachable!(),
            }
        }
    }

    #[test]
    fn syntax_story_waits_for_colours() {
        let story = catalog::by_id("single-file/rust-syntax").unwrap();
        let mut harness = story_harness(story, 100, 24).unwrap();

        assert_ne!(harness.style_at(4, 2).fg, Theme::DARK.normal.fg);
        assert!(harness.screen_line(2).contains("fn highlighted"));
    }
}
