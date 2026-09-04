//! One terminal session that switches between the catalog and story previews.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{self, Sender};

use anyhow::Result;
use loom::{Flow, Tree, deliver_input};
use ui::services::files::FilesService;
use ui::services::syntax::SyntaxService;

use super::browser::{BrowserApp, BrowserAppProps};
use super::catalog;
use super::component::{Gallery, GalleryProps, Navigation};
use super::definition::{StoryDefinition, StoryFixture};
use super::session::{context, settle_tree};

enum Event {
    Terminal(ui::crossterm::event::Event),
    #[cfg(unix)]
    Signal(i32),
    Open(usize),
    Navigate(Navigation),
    FilesReady {
        generation: u64,
        response: pipeline::files::Response,
    },
    SyntaxReady {
        generation: u64,
        response: syntax::SyntaxResponse,
    },
}

struct Active {
    generation: u64,
    definition: &'static StoryDefinition,
    files_service: Option<Rc<FilesService>>,
    syntax_service: Option<Rc<SyntaxService>>,
    setup_done: bool,
}

struct Controller {
    events: Sender<Event>,
    generation: u64,
    selected: usize,
    active: Option<Active>,
}

impl Controller {
    fn new(events: Sender<Event>) -> Self {
        Self {
            events,
            generation: 0,
            selected: 0,
            active: None,
        }
    }

    fn browser_tree(&mut self) -> Tree {
        self.active = None;
        let events = self.events.clone();
        Tree::new::<BrowserApp>(BrowserAppProps {
            initial_story: self.selected,
            open: Rc::new(move |index| {
                let _ = events.send(Event::Open(index));
            }),
        })
    }

    fn open(&mut self, index: usize, tree: &mut Tree) -> Result<()> {
        let definition = catalog::at(index)
            .ok_or_else(|| anyhow::anyhow!("story index {index} is outside the catalog"))?;
        let fixture = definition.build()?;
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
                channel::Emitter::new(self.events.clone(), move |response| Event::FilesReady {
                    generation,
                    response,
                }),
            );
            Rc::new(FilesService::new(Rc::new(RefCell::new(worker)), Vec::new()))
        });
        let syntax_service = syntax.then(|| {
            let worker = syntax::Syntax::start(channel::Emitter::new(
                self.events.clone(),
                move |response| Event::SyntaxReady {
                    generation,
                    response,
                },
            ));
            Rc::new(SyntaxService::new(Rc::new(RefCell::new(worker))))
        });
        let navigation_events = self.events.clone();
        let props = GalleryProps {
            definition,
            base_context: context(&files_service, &syntax_service),
            content,
            navigate: Some(Rc::new(move |navigation| {
                let _ = navigation_events.send(Event::Navigate(navigation));
            })),
        };
        *tree = Tree::new::<Gallery>(props);
        settle_tree(tree);
        self.selected = index;
        self.active = Some(Active {
            generation,
            definition,
            files_service,
            syntax_service,
            setup_done: definition.setup.is_empty(),
        });
        Ok(())
    }

    fn navigate(&mut self, navigation: Navigation, tree: &mut Tree) -> Result<()> {
        match navigation {
            Navigation::Catalog => {
                *tree = self.browser_tree();
                Ok(())
            }
            Navigation::Previous => {
                let index = self.selected.checked_sub(1).unwrap_or(catalog::len() - 1);
                self.open(index, tree)
            }
            Navigation::Next => self.open((self.selected + 1) % catalog::len(), tree),
            Navigation::Reset => self.open(self.selected, tree),
        }
    }

    fn deliver_files(
        &mut self,
        generation: u64,
        response: pipeline::files::Response,
        tree: &mut Tree,
    ) {
        let Some(active) = self
            .active
            .as_mut()
            .filter(|active| active.generation == generation)
        else {
            return;
        };
        let Some(service) = active.files_service.as_ref().map(Rc::clone) else {
            return;
        };
        let setup = active.definition.setup;
        let apply_setup = !active.setup_done;
        active.setup_done = true;

        service.deliver(response);
        settle_tree(tree);
        if apply_setup {
            for &key in setup {
                tree.press(key);
                settle_tree(tree);
            }
        }
    }

    fn deliver_syntax(&self, generation: u64, response: syntax::SyntaxResponse, tree: &mut Tree) {
        let Some(service) = self
            .active
            .as_ref()
            .filter(|active| active.generation == generation)
            .and_then(|active| active.syntax_service.as_ref())
        else {
            return;
        };
        service.deliver(response);
        settle_tree(tree);
    }
}

pub fn run() -> Result<()> {
    let (events_tx, events_rx) = mpsc::channel();
    let mut controller = Controller::new(events_tx.clone());
    let mut tree = controller.browser_tree();
    let failure = Rc::new(RefCell::new(None));
    let failure_in_loop = Rc::clone(&failure);

    #[cfg(unix)]
    ui::spawn_signals(events_tx.clone(), Event::Signal);

    loom::run(
        &mut tree,
        events_rx,
        events_tx,
        Event::Terminal,
        move |event, tree| {
            let result = match event {
                Event::Terminal(event) => {
                    deliver_input(tree, &event);
                    Ok(())
                }
                #[cfg(unix)]
                Event::Signal(signal) => {
                    loom::restore();
                    std::process::exit(128 + signal);
                }
                Event::Open(index) => controller.open(index, tree),
                Event::Navigate(navigation) => controller.navigate(navigation, tree),
                Event::FilesReady {
                    generation,
                    response,
                } => {
                    controller.deliver_files(generation, response, tree);
                    Ok(())
                }
                Event::SyntaxReady {
                    generation,
                    response,
                } => {
                    controller.deliver_syntax(generation, response, tree);
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
