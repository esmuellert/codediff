//! Names one live component, and the token it holds while it runs.

/// Names one live component. The generation is bumped when a slab entry is
/// reused, so a stale handle fails a check instead of reading a stranger.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ScopeId {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

/// The token a component holds while it runs.
///
/// Two integers. Everything a hook touches lives in the runtime and is reached
/// by a short borrow, so a hook may walk the tree while the component's own
/// function is on the stack.
pub struct Scope {
    pub(crate) id: ScopeId,
}

impl Scope {
    pub fn id(&self) -> ScopeId {
        self.id
    }
    pub fn name(&self) -> &'static str {
        crate::current::with(|rt| rt.name_of(self.id)).unwrap_or("?")
    }
}

/// What the runtime keeps for one live component.
pub(crate) struct Mounted {
    pub name: &'static str,
    pub type_id: std::any::TypeId,
    pub key: Option<crate::node::Key>,
    pub parent: Option<ScopeId>,
    pub children: Vec<ScopeId>,
    /// The props this scope last rendered with.
    pub props: std::rc::Rc<dyn std::any::Any>,
    pub render: fn(&dyn std::any::Any, &mut Scope) -> crate::node::Node,
    pub props_equal: Option<fn(&dyn std::any::Any, &dyn std::any::Any) -> bool>,
    /// The hosts this scope produced last frame, handed back when it is clean.
    pub produced: Vec<crate::reconcile::Fiber>,
    /// Set when something asked for this scope to run again.
    pub dirty: bool,
    /// How many times this scope's function has run.
    pub renders: usize,
    /// Which contexts this scope read last render, and at what version.
    pub reads: Vec<(std::any::TypeId, u64)>,
}

/// One entry in the scope slab.
pub(crate) enum Cell {
    Live(Box<Mounted>),
    Free { next: Option<u32> },
}

/// The slab of live scopes. The generation beside each cell is what makes a
/// stale `ScopeId` fail a check instead of reading whoever moved in.
pub(crate) struct Scopes {
    cells: Vec<Cell>,
    generations: Vec<u32>,
    free: Option<u32>,
}

impl Scopes {
    pub fn new() -> Self {
        Self { cells: Vec::new(), generations: Vec::new(), free: None }
    }

    pub fn insert(&mut self, mounted: Mounted) -> ScopeId {
        match self.free {
            Some(index) => {
                let Cell::Free { next } = self.cells[index as usize] else {
                    unreachable!("the free list holds only free cells")
                };
                self.free = next;
                self.cells[index as usize] = Cell::Live(Box::new(mounted));
                ScopeId { index, generation: self.generations[index as usize] }
            }
            None => {
                let index = self.cells.len() as u32;
                self.cells.push(Cell::Live(Box::new(mounted)));
                self.generations.push(0);
                ScopeId { index, generation: 0 }
            }
        }
    }

    pub fn remove(&mut self, id: ScopeId) -> Option<Box<Mounted>> {
        if !self.is_alive(id) {
            return None;
        }
        let cell =
            std::mem::replace(&mut self.cells[id.index as usize], Cell::Free { next: self.free });
        // The generation moves on, so every handle to the old occupant now
        // fails `is_alive`.
        self.generations[id.index as usize] = id.generation.wrapping_add(1);
        self.free = Some(id.index);
        match cell {
            Cell::Live(mounted) => Some(mounted),
            Cell::Free { .. } => None,
        }
    }

    pub fn is_alive(&self, id: ScopeId) -> bool {
        matches!(self.cells.get(id.index as usize), Some(Cell::Live(_)))
            && self.generations.get(id.index as usize) == Some(&id.generation)
    }

    pub fn get(&self, id: ScopeId) -> Option<&Mounted> {
        if !self.is_alive(id) {
            return None;
        }
        match &self.cells[id.index as usize] {
            Cell::Live(mounted) => Some(mounted),
            Cell::Free { .. } => None,
        }
    }

    pub fn get_mut(&mut self, id: ScopeId) -> Option<&mut Mounted> {
        if !self.is_alive(id) {
            return None;
        }
        match &mut self.cells[id.index as usize] {
            Cell::Live(mounted) => Some(mounted),
            Cell::Free { .. } => None,
        }
    }
}
