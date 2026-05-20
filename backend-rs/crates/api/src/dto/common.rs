//! Shared envelope types.

use serde::{Deserialize, Serialize};

use flightradar_domain::ports::repositories::Page;

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct PageInfo {
    pub page: u32,
    pub page_size: u32,
    pub total: u64,
    pub total_pages: u32,
}

impl PageInfo {
    pub fn from_page<T>(page: &Page<T>) -> Self {
        let page_size = page.page_size.max(1);
        // Ceiling division without floats: (total + ps - 1) / ps.
        let total_pages = page
            .total
            .saturating_add(u64::from(page_size).saturating_sub(1))
            .checked_div(u64::from(page_size))
            .unwrap_or(0);
        let total_pages = u32::try_from(total_pages).unwrap_or(u32::MAX).max(1);
        Self {
            page: page.page,
            page_size,
            total: page.total,
            total_pages,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PagedResponse<T> {
    pub items: Vec<T>,
    #[serde(flatten)]
    pub page: PageInfo,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page<T>(items: Vec<T>, total: u64, page: u32, page_size: u32) -> Page<T> {
        Page {
            items,
            total,
            page,
            page_size,
        }
    }

    #[test]
    fn page_info_calculates_total_pages() {
        let p = page::<i32>(vec![], 25, 1, 10);
        let info = PageInfo::from_page(&p);
        assert_eq!(info.total_pages, 3);
        assert_eq!(info.total, 25);
        assert_eq!(info.page_size, 10);
    }

    #[test]
    fn page_info_total_pages_never_below_one() {
        let p = page::<i32>(vec![], 0, 1, 10);
        let info = PageInfo::from_page(&p);
        assert_eq!(info.total_pages, 1);
    }

    #[test]
    fn page_info_handles_exact_multiple() {
        let p = page::<i32>(vec![], 20, 2, 10);
        assert_eq!(PageInfo::from_page(&p).total_pages, 2);
    }

    #[test]
    fn page_info_clamps_zero_page_size() {
        let p = page::<i32>(vec![], 10, 1, 0);
        assert_eq!(PageInfo::from_page(&p).page_size, 1);
    }
}
