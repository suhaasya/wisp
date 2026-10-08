//! Fixed-size ring of loaded pages around the viewport.

use wisp_drivers::Page;

use super::strategy::KeysetCursor;

#[derive(Debug)]
pub struct CachedPage {
    pub page_index: u64,
    pub offset: u64,
    pub page: Page,
    pub bytes: usize,
    pub end_cursor: Option<KeysetCursor>,
}

#[derive(Debug)]
pub struct PageRing {
    capacity: usize,
    slots: Vec<Option<CachedPage>>,
}

impl PageRing {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            slots: (0..capacity.max(1)).map(|_| None).collect(),
        }
    }

    pub fn clear(&mut self) {
        for slot in &mut self.slots {
            *slot = None;
        }
    }

    pub fn resident_bytes(&self) -> usize {
        self.slots
            .iter()
            .filter_map(|s| s.as_ref())
            .map(|p| p.bytes)
            .sum()
    }

    pub fn contains(&self, page_index: u64) -> bool {
        self.slots
            .iter()
            .any(|s| s.as_ref().is_some_and(|p| p.page_index == page_index))
    }

    pub fn get(&self, page_index: u64) -> Option<&Page> {
        self.slots
            .iter()
            .find_map(|s| s.as_ref().filter(|p| p.page_index == page_index))
            .map(|p| &p.page)
    }

    pub fn end_cursor(&self, page_index: u64) -> Option<KeysetCursor> {
        self.slots
            .iter()
            .find_map(|s| s.as_ref().filter(|p| p.page_index == page_index))
            .and_then(|p| p.end_cursor.clone())
    }

    pub fn would_evict_bytes(
        &self,
        anchor_page: u64,
        protect_from: u64,
        protect_through: u64,
    ) -> usize {
        if self.slots.iter().any(|s| s.is_none()) {
            return 0;
        }
        self.pick_victim(anchor_page, protect_from, protect_through)
            .and_then(|i| self.slots[i].as_ref().map(|p| p.bytes))
            .unwrap_or(0)
    }

    fn pick_victim(
        &self,
        anchor_page: u64,
        protect_from: u64,
        protect_through: u64,
    ) -> Option<usize> {
        let mut candidates: Vec<(usize, u64)> = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().map(|p| (i, p.page_index)))
            .filter(|(_, idx)| *idx < protect_from || *idx > protect_through)
            .collect();
        if candidates.is_empty() {
            candidates = self
                .slots
                .iter()
                .enumerate()
                .filter_map(|(i, s)| s.as_ref().map(|p| (i, p.page_index)))
                .collect();
        }
        candidates
            .into_iter()
            .max_by_key(|(_, idx)| idx.abs_diff(anchor_page))
            .map(|(i, _)| i)
    }

    pub fn insert(
        &mut self,
        entry: CachedPage,
        anchor_page: u64,
        protect_from: u64,
        protect_through: u64,
    ) {
        if self.contains(entry.page_index) {
            return;
        }
        if let Some(empty) = self.slots.iter().position(|s| s.is_none()) {
            self.slots[empty] = Some(entry);
            return;
        }
        let victim = self
            .pick_victim(anchor_page, protect_from, protect_through)
            .expect("ring has slots");
        self.slots[victim] = Some(entry);
    }
}
