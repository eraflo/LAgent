use thiserror::Error;

#[derive(Debug, Error)]
pub enum HeapError {
    #[error("context overflow: requested {requested} tokens but only {available} available")]
    Overflow { requested: usize, available: usize },
    #[error("invalid segment handle: {0}")]
    InvalidHandle(u32),
}

/// A segment of the token context heap.
#[derive(Debug, Clone)]
pub struct CtxSegment {
    pub id: u32,
    pub capacity: usize,
    pub content: String,
}

/// Token Heap — analogous to a memory heap, but for LLM context tokens.
#[derive(Debug, Default)]
pub struct TokenHeap {
    segments: Vec<CtxSegment>,
    next_id: u32,
    total_capacity: usize,
    used: usize,
}

impl TokenHeap {
    pub fn new(total_capacity: usize) -> Self {
        Self {
            segments: Vec::new(),
            next_id: 0,
            total_capacity,
            used: 0,
        }
    }

    /// Allocate a new context segment (analogous to malloc).
    pub fn alloc(&mut self, tokens: usize) -> Result<u32, HeapError> {
        let available = self.total_capacity.saturating_sub(self.used);
        if tokens > available {
            return Err(HeapError::Overflow { requested: tokens, available });
        }
        let id = self.next_id;
        self.next_id += 1;
        self.used += tokens;
        self.segments.push(CtxSegment { id, capacity: tokens, content: String::new() });
        Ok(id)
    }

    /// Append text to a context segment.
    pub fn append(&mut self, id: u32, text: &str) -> Result<(), HeapError> {
        let seg = self.get_mut(id)?;
        seg.content.push_str(text);
        Ok(())
    }

    /// Free a context segment (analogous to free).
    pub fn free(&mut self, id: u32) -> Result<(), HeapError> {
        let pos = self.segments.iter().position(|s| s.id == id)
            .ok_or(HeapError::InvalidHandle(id))?;
        let seg = self.segments.remove(pos);
        self.used = self.used.saturating_sub(seg.capacity);
        Ok(())
    }

    fn get_mut(&mut self, id: u32) -> Result<&mut CtxSegment, HeapError> {
        self.segments.iter_mut().find(|s| s.id == id)
            .ok_or(HeapError::InvalidHandle(id))
    }

    pub fn get(&self, id: u32) -> Result<&CtxSegment, HeapError> {
        self.segments.iter().find(|s| s.id == id)
            .ok_or(HeapError::InvalidHandle(id))
    }
}
