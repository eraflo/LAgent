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

    /// Return the number of tokens currently in use across all segments.
    pub fn used(&self) -> usize {
        self.used
    }

    /// Return an immutable reference to a segment by id.
    pub fn get(&self, id: u32) -> Result<&CtxSegment, HeapError> {
        self.segments
            .iter()
            .find(|s| s.id == id)
            .ok_or(HeapError::InvalidHandle(id))
    }
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_and_free() {
        let mut heap = TokenHeap::new(1024);
        let id = heap.alloc(256).unwrap();
        heap.free(id).unwrap();
        // After free, used capacity should be back to 0.
        assert_eq!(heap.used(), 0);
    }

    #[test]
    fn overflow_returns_error() {
        let mut heap = TokenHeap::new(100);
        let err = heap.alloc(200).unwrap_err();
        assert!(matches!(
            err,
            HeapError::Overflow { requested: 200, available: 100 }
        ));
    }

    #[test]
    fn append_text_to_segment() {
        let mut heap = TokenHeap::new(1024);
        let id = heap.alloc(512).unwrap();
        heap.append(id, "hello ").unwrap();
        heap.append(id, "world").unwrap();
        assert_eq!(heap.get(id).unwrap().content, "hello world");
    }

    #[test]
    fn free_invalid_handle_returns_error() {
        let mut heap = TokenHeap::new(1024);
        assert!(matches!(
            heap.free(99).unwrap_err(),
            HeapError::InvalidHandle(99)
        ));
    }

    #[test]
    fn multiple_segments_are_independent() {
        let mut heap = TokenHeap::new(2048);
        let id_a = heap.alloc(512).unwrap();
        let id_b = heap.alloc(512).unwrap();
        heap.append(id_a, "alpha").unwrap();
        heap.append(id_b, "beta").unwrap();
        assert_eq!(heap.get(id_a).unwrap().content, "alpha");
        assert_eq!(heap.get(id_b).unwrap().content, "beta");
    }

    #[test]
    fn used_tracks_allocations() {
        let mut heap = TokenHeap::new(1024);
        let id = heap.alloc(300).unwrap();
        assert_eq!(heap.used(), 300);
        heap.free(id).unwrap();
        assert_eq!(heap.used(), 0);
    }
}
