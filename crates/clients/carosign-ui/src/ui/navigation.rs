use super::page::Page;

#[derive(Clone, Copy, Default)]
pub(crate) struct Navigation {
    page: Page,
    compact: bool,
}

impl Navigation {
    pub(crate) fn page(self) -> Page {
        self.page
    }

    pub(crate) fn compact(self) -> bool {
        self.compact
    }

    pub(crate) fn resize(&mut self, width: f32) {
        const COMPACT_WIDTH: f32 = 700.0;
        self.compact = width < COMPACT_WIDTH;
        if !self.compact && self.page == Page::Sections {
            self.page = Page::Platform;
        }
    }

    pub(crate) fn enter(&mut self, page: Page) -> Page {
        std::mem::replace(&mut self.page, page)
    }

    pub(crate) fn back(&mut self) {
        if self.compact() {
            self.page = Page::Sections;
        }
    }
}

#[cfg(test)]
#[path = "navigation.tests.rs"]
mod tests;
