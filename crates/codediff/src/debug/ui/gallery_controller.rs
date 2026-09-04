//! One terminal session that switches between the catalog and story previews.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{self, Sender};

use anyhow::Result;
use loom::{Flow, Tree, deliver_input};
use ui::services::files::FilesService;
use ui::services::syntax::SyntaxService;

use super::catalog;
use super::catalog_view::{CatalogRoot, CatalogRootProps};
use super::definition::{StoryDefinition, StoryFixture};
use super::preview::{PreviewAction, StoryPreview, StoryPreviewProps};
use super::story_host::{settle_story_tree, story_context};

enum GalleryEvent {
    Terminal(ui::crossterm::event::Event),
    #[cfg(unix)]
    Signal(i32),
    OpenStory(usize),
    PreviewAction(PreviewAction),
    FilesReady {
        generation: u64,
        response: pipeline::files::Response,
    },
    SyntaxReady {
        generation: u64,
        response: syntax::SyntaxResponse,
    },
}

struct ActiveStory {
    generation: u64,
    definition: &'static StoryDefinition,
    files_service: Option<Rc<FilesService>>,
    syntax_service: Option<Rc<SyntaxService>>,
    initial_keys_applied: bool,
}

struct GalleryController {
    events_tx: Sender<GalleryEvent>,
    generation: u64,
    selected_story_index: usize,
    active_story: Option<ActiveStory>,
}

impl GalleryController {
    fn new(events_tx: Sender<GalleryEvent>) -> Self {
        Self {
            events_tx,
            generation: 0,
            selected_story_index: 0,
            active_story: None,
        }
    }

    fn catalog_tree(&mut self) -> Tree {
        self.active_story = None;
        let events_tx = self.events_tx.clone();
        Tree::new::<CatalogRoot>(CatalogRootProps {
            initial_story_index: self.selected_story_index,
            open_story: Rc::new(move |index| {
                let _ = events_tx.send(GalleryEvent::OpenStory(index));
            }),
        })
    }

    fn open_story(&mut self, index: usize, tree: &mut Tree) -> Result<()> {
        let definition = catalog::by_index(index)
            .ok_or_else(|| anyhow::anyhow!("story index {index} is outside the catalog"))?;
        let fixture = definition.create_fixture()?;
        let syntax = fixture.needs_syntax();
        let (files, content) = match fixture {
            StoryFixture::Welcome => (None, None),
            StoryFixture::Explorer(files) => (Some(files), None),
            StoryFixture::SideBySide(content) | StoryFixture::SingleFile(content) => {
                (None, Some(content))
            }
        };

        self.generation += 1;
        let generation = self.generation;
        let files_service = files.map(|files| {
            let worker = pipeline::files::FilesWorker::mock(
                vec![files],
                channel::Emitter::new(self.events_tx.clone(), move |response| {
                    GalleryEvent::FilesReady {
                        generation,
                        response,
                    }
                }),
            );
            Rc::new(FilesService::new(Rc::new(RefCell::new(worker)), Vec::new()))
        });
        let syntax_service = syntax.then(|| {
            let worker = syntax::Syntax::start(channel::Emitter::new(
                self.events_tx.clone(),
                move |response| GalleryEvent::SyntaxReady {
                    generation,
                    response,
                },
            ));
            Rc::new(SyntaxService::new(Rc::new(RefCell::new(worker))))
        });
        let navigation_tx = self.events_tx.clone();
        let props = StoryPreviewProps {
            definition,
            base_context: story_context(&files_service, &syntax_service),
            content,
            navigate: Some(Rc::new(move |action| {
                let _ = navigation_tx.send(GalleryEvent::PreviewAction(action));
            })),
        };
        *tree = Tree::new::<StoryPreview>(props);
        settle_story_tree(tree);
        self.selected_story_index = index;
        self.active_story = Some(ActiveStory {
            generation,
            definition,
            files_service,
            syntax_service,
            initial_keys_applied: definition.initial_keys.is_empty(),
        });
        Ok(())
    }

    fn apply_preview_action(&mut self, action: PreviewAction, tree: &mut Tree) -> Result<()> {
        match action {
            PreviewAction::Catalog => {
                *tree = self.catalog_tree();
                Ok(())
            }
            PreviewAction::Previous => {
                let index = self
                    .selected_story_index
                    .checked_sub(1)
                    .unwrap_or(catalog::story_count() - 1);
                self.open_story(index, tree)
            }
            PreviewAction::Next => self.open_story(
                (self.selected_story_index + 1) % catalog::story_count(),
                tree,
            ),
            PreviewAction::Reset => self.open_story(self.selected_story_index, tree),
        }
    }

    fn deliver_files_if_current(
        &mut self,
        generation: u64,
        response: pipeline::files::Response,
        tree: &mut Tree,
    ) {
        let Some(active_story) = self
            .active_story
            .as_mut()
            .filter(|story| story.generation == generation)
        else {
            return;
        };
        let Some(service) = active_story.files_service.as_ref().map(Rc::clone) else {
            return;
        };
        let initial_keys = active_story.definition.initial_keys;
        let apply_initial_keys = !active_story.initial_keys_applied;
        active_story.initial_keys_applied = true;

        service.deliver(response);
        settle_story_tree(tree);
        if apply_initial_keys {
            for &key in initial_keys {
                tree.press(key);
                settle_story_tree(tree);
            }
        }
    }

    fn deliver_syntax_if_current(
        &self,
        generation: u64,
        response: syntax::SyntaxResponse,
        tree: &mut Tree,
    ) {
        let Some(service) = self
            .active_story
            .as_ref()
            .filter(|story| story.generation == generation)
            .and_then(|story| story.syntax_service.as_ref())
        else {
            return;
        };
        service.deliver(response);
        settle_story_tree(tree);
    }
}

pub fn run() -> Result<()> {
    let (events_tx, events_rx) = mpsc::channel();
    let mut controller = GalleryController::new(events_tx.clone());
    let mut tree = controller.catalog_tree();
    let failure = Rc::new(RefCell::new(None));
    let failure_in_loop = Rc::clone(&failure);

    #[cfg(unix)]
    ui::spawn_signals(events_tx.clone(), GalleryEvent::Signal);

    loom::run(
        &mut tree,
        events_rx,
        events_tx,
        GalleryEvent::Terminal,
        move |event, tree| {
            let result = match event {
                GalleryEvent::Terminal(event) => {
                    deliver_input(tree, &event);
                    Ok(())
                }
                #[cfg(unix)]
                GalleryEvent::Signal(signal) => {
                    loom::restore();
                    std::process::exit(128 + signal);
                }
                GalleryEvent::OpenStory(index) => controller.open_story(index, tree),
                GalleryEvent::PreviewAction(action) => {
                    controller.apply_preview_action(action, tree)
                }
                GalleryEvent::FilesReady {
                    generation,
                    response,
                } => {
                    controller.deliver_files_if_current(generation, response, tree);
                    Ok(())
                }
                GalleryEvent::SyntaxReady {
                    generation,
                    response,
                } => {
                    controller.deliver_syntax_if_current(generation, response, tree);
                    Ok(())
                }
            };
            if let Err(error) = result {
                *failure_in_loop.borrow_mut() = Some(error);
                Flow::Quit
            } else {
                Flow::Continue
            }
        },
    )?;

    if let Some(error) = failure.borrow_mut().take() {
        return Err(error);
    }
    Ok(())
}
