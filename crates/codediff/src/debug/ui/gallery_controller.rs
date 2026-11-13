//! One terminal session that switches between the catalog and story previews.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{self, Sender};

use anyhow::Result;
use diff_service::DiffService;
use loom::{Flow, Tree, deliver_input};

use super::catalog;
use super::catalog_view::{CatalogRoot, CatalogRootProps};
use super::definition::StoryDefinition;
use super::preview::{PreviewAction, StoryPreview};
use super::story_host::{settle_story_tree, story_props};

enum GalleryEvent {
    Terminal(ui::crossterm::event::Event),
    #[cfg(unix)]
    Signal(i32),
    OpenStory(usize),
    PreviewAction(PreviewAction),
    Redraw,
}

struct ActiveStory {
    definition: &'static StoryDefinition,
    pipeline: Rc<DiffService>,
    initial_keys_applied: bool,
}

struct GalleryController {
    events_tx: Sender<GalleryEvent>,
    selected_story_index: usize,
    active_story: Option<ActiveStory>,
}

impl GalleryController {
    fn new(events_tx: Sender<GalleryEvent>) -> Self {
        Self {
            events_tx,
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
        let redraw = self.events_tx.clone();
        let navigation_tx = self.events_tx.clone();
        let (pipeline, props) = story_props(
            definition,
            fixture,
            move || {
                let _ = redraw.send(GalleryEvent::Redraw);
            },
            Some(Rc::new(move |action| {
                let _ = navigation_tx.send(GalleryEvent::PreviewAction(action));
            })),
        );
        *tree = Tree::new::<StoryPreview>(props);
        settle_story_tree(tree);
        self.selected_story_index = index;
        self.active_story = Some(ActiveStory {
            definition,
            pipeline,
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

    /// Draws what the background threads changed. A story's keys are pressed
    /// once its files have loaded.
    fn redraw(&mut self, tree: &mut Tree) {
        tree.redraw_all();
        let Some(story) = self.active_story.as_mut() else {
            return;
        };
        if story.initial_keys_applied || story.pipeline.is_busy() {
            return;
        }
        story.initial_keys_applied = true;
        settle_story_tree(tree);
        for &key in story.definition.initial_keys {
            tree.press(key);
            settle_story_tree(tree);
        }
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
                GalleryEvent::Redraw => {
                    controller.redraw(tree);
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
