/// The event system for both client and server
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};

/// EventQueue has an Arc to the internal structure, so cloning it clones the Arc and is cheap
#[derive(Clone)]
pub struct EventQueue {
    arc: Arc<RwLock<VecDeque<Event>>>,
}

impl EventQueue {
    pub fn new() -> Self {
        EventQueue {
            arc: Arc::new(RwLock::new(VecDeque::new())),
        }
    }

    pub fn push(&self, event: Event) {
        self.arc.write().unwrap().push_back(event);
    }

    /// Consumes all events
    pub fn poll(&self, mut f: impl FnMut(Event)) {
        let mut deque = self.arc.write().unwrap();
        while let Some(event) = deque.pop_front() {
            f(event);
        }
    }
}

#[derive(Debug)]
pub enum Event {
    /// A press of a mouse button with this id
    Button(u32),
    /// A key press with this scan code
    KeyPressed(u32),
    KeyReleased(u32),
    /// A change in mouse position
    Mouse(f64, f64),
    /// A window resize, with new width and height
    Resize(f64, f64),
    /// The application needs to close, so do any destruction necessary
    Quit,
}
