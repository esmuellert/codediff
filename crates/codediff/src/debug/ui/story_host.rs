//! Running one named story either on a test screen or a real terminal.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use loom::ratatui::buffer::Buffer;
use loom::ratatui::layout::Rect;
use loom::testing::Harness;
use loom::{Flow, Tree, deliver_input};
use ui::Theme;
use ui::components::Context as UiContext;
use ui::services::files::FilesService;
use ui::services::syntax::SyntaxService;

#[cfg(test)]
use super::catalog;
use super::definition::{StoryDefinition, StoryFixture};
use super::preview::{StoryPreview, StoryPreviewProps};

enum StoryEvent {
    Terminal(ui::crossterm::event::Event),
    #[cfg(unix)]
    Signal(i32),
    FilesReady(pipeline::files::Response),
    SyntaxReady(syntax::SyntaxResponse),
}

struct StoryHost {
    definition: &'static StoryDefinition,
    events_tx: Sender<StoryEvent>,
    events_rx: Receiver<StoryEvent>,
    files_service: Option<Rc<FilesService>>,
    syntax_service: Option<Rc<SyntaxService>>,
    initial_syntax_response_count: usize,
}

impl StoryHost {
    fn new(definition: &'static StoryDefinition) -> Result<(Self, StoryPreviewProps)> {
        let fixture = definition.create_fixture()?;
        let syntax = fixture.needs_syntax();
        let initial_syntax_response_count = fixture.initial_syntax_response_count();
        let (files, content) = match fixture {
            StoryFixture::Welcome => (None, None),
            StoryFixture::Explorer(files) => (Some(files), None),
            StoryFixture::SideBySide(content) | StoryFixture::SingleFile(content) => {
                (None, Some(content))
            }
        };

        let (events_tx, events_rx) = mpsc::channel();
        let files_service = files.map(|files| {
            let worker = pipeline::files::FilesWorker::mock(
                vec![files],
                channel::Emitter::new(events_tx.clone(), StoryEvent::FilesReady),
            );
            Rc::new(FilesService::new(Rc::new(RefCell::new(worker)), Vec::new()))
        });
        let syntax_service = syntax.then(|| {
            let worker = syntax::Syntax::start(channel::Emitter::new(
                events_tx.clone(),
                StoryEvent::SyntaxReady,
            ));
            Rc::new(SyntaxService::new(Rc::new(RefCell::new(worker))))
        });
        let props = StoryPreviewProps {
            definition,
            base_context: story_context(&files_service, &syntax_service),
            content,
            navigate: None,
        };
        Ok((
            Self {
                definition,
                events_tx,
                events_rx,
                files_service,
                syntax_service,
                initial_syntax_response_count,
            },
            props,
        ))
    }

    fn wait_for_files(&self) -> Result<pipeline::files::Response> {
        match self.events_rx.recv_timeout(Duration::from_secs(2)) {
            Ok(StoryEvent::FilesReady(response)) => Ok(response),
            Ok(_) => bail!("{} received the wrong setup event", self.definition.id),
            Err(error) => Err(error).context("waiting for the story's file list"),
        }
    }

    fn wait_for_syntax(&self) -> Result<syntax::SyntaxResponse> {
        match self.events_rx.recv_timeout(Duration::from_secs(2)) {
            Ok(StoryEvent::SyntaxReady(response)) => Ok(response),
            Ok(_) => bail!("{} received the wrong setup event", self.definition.id),
            Err(error) => Err(error).context("waiting for the story's syntax colours"),
        }
    }

    fn prepare_harness(&self, harness: &mut Harness) -> Result<()> {
        settle_harness(harness);
        if let Some(service) = &self.files_service {
            service.deliver(self.wait_for_files()?);
            settle_harness(harness);
        }
        for &key in self.definition.initial_keys {
            harness.press(key);
            settle_harness(harness);
        }
        if let Some(service) = &self.syntax_service {
            for _ in 0..self.initial_syntax_response_count {
                service.deliver(self.wait_for_syntax()?);
                settle_harness(harness);
            }
        }
        Ok(())
    }

    fn prepare_tree(&self, tree: &mut Tree) -> Result<()> {
        settle_story_tree(tree);
        if let Some(service) = &self.files_service {
            service.deliver(self.wait_for_files()?);
            settle_story_tree(tree);
        }
        for &key in self.definition.initial_keys {
            tree.press(key);
            settle_story_tree(tree);
        }
        if let Some(service) = &self.syntax_service {
            for _ in 0..self.initial_syntax_response_count {
                service.deliver(self.wait_for_syntax()?);
                settle_story_tree(tree);
            }
        }
        tree.redraw_all();
        Ok(())
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
    host.prepare_harness(&mut harness)?;
    Ok(harness)
}

pub fn run(definition: &'static StoryDefinition) -> Result<()> {
    let (host, props) = StoryHost::new(definition)?;
    let mut tree = Tree::new::<StoryPreview>(props);
    host.prepare_tree(&mut tree)?;

    #[cfg(unix)]
    ui::spawn_signals(host.events_tx.clone(), StoryEvent::Signal);

    let files_service = host.files_service;
    let syntax_service = host.syntax_service;
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
                StoryEvent::FilesReady(response) => {
                    if let Some(service) = &files_service {
                        service.deliver(response);
                    }
                }
                StoryEvent::SyntaxReady(response) => {
                    if let Some(service) = &syntax_service {
                        service.deliver(response);
                    }
                }
            }
            Flow::Continue
        },
    )?;
    Ok(())
}

pub(super) fn story_context(
    files_service: &Option<Rc<FilesService>>,
    syntax_service: &Option<Rc<SyntaxService>>,
) -> UiContext {
    UiContext {
        theme: Rc::new(Theme::DARK),
        repo: Rc::from(Path::new("/codediff-story")),
        files_service: files_service.as_ref().map(Rc::clone),
        syntax_service: syntax_service.as_ref().map(Rc::clone),
        ..UiContext::default()
    }
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
        bail!("a UI story needs a non-zero width and at least two rows");
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
        assert!(harness.screen_row(5).contains("button.rs"));
    }

    #[test]
    fn every_nonempty_code_story_waits_for_syntax_colours() {
        use super::super::definition::StoryComponent;

        for story in catalog::stories().filter(|story| {
            matches!(
                story.component,
                StoryComponent::SideBySide | StoryComponent::SingleFile
            ) && story.id != "single-file/empty"
        }) {
            let mut harness = story_harness(story, 100, 24).unwrap();
            let normal = Theme::DARK.normal.fg;
            let cells = harness.cells();
            let coloured = |start: u16, end: u16| {
                (start..end).any(|x| {
                    cells.cell((x, 2)).is_some_and(|cell| {
                        cell.symbol().chars().any(char::is_alphabetic) && cell.style().fg != normal
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
                StoryComponent::SingleFile => {
                    assert!(coloured(0, 100), "{} had no syntax-coloured text", story.id);
                }
                StoryComponent::Welcome | StoryComponent::Explorer => unreachable!(),
            }
        }
    }

    #[test]
    fn syntax_story_waits_for_colours() {
        let story = catalog::by_id("single-file/rust-syntax").unwrap();
        let mut harness = story_harness(story, 100, 24).unwrap();

        assert_ne!(harness.style_at(4, 2).fg, Theme::DARK.normal.fg);
        assert!(harness.screen_row(2).contains("fn highlighted"));
    }
}
