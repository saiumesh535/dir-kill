use crate::fs::DirectoryInfo;

/// Application state for TUI
pub struct App {
    pub directories: Vec<DirectoryInfo>,
    pub selected: usize,
    pub pattern: String,
    pub path: String,
    pub current_page: usize,
    pub selection_mode: bool,
}

impl App {
    pub fn new(directories: Vec<DirectoryInfo>, pattern: String, path: String) -> Self {
        Self {
            directories,
            selected: 0,
            pattern,
            path,
            current_page: 0,
            selection_mode: false,
        }
    }

    pub fn next(&mut self, items_per_page: usize) {
        if !self.directories.is_empty() {
            self.selected = (self.selected + 1) % self.directories.len();
            self.update_selection_for_pagination(items_per_page);
        }
    }

    pub fn previous(&mut self, items_per_page: usize) {
        if !self.directories.is_empty() {
            self.selected = if self.selected == 0 {
                self.directories.len() - 1
            } else {
                self.selected - 1
            };
            self.update_selection_for_pagination(items_per_page);
        }
    }

    pub fn select_first(&mut self) {
        if !self.directories.is_empty() {
            self.selected = 0;
        }
    }

    pub fn select_last(&mut self) {
        if !self.directories.is_empty() {
            self.selected = self.directories.len() - 1;
        }
    }

    pub fn get_selected_directory(&self) -> Option<&DirectoryInfo> {
        self.directories.get(self.selected)
    }

    pub fn directory_count(&self) -> usize {
        self.directories.len()
    }

    pub fn is_empty(&self) -> bool {
        self.directories.is_empty()
    }

    // Pagination methods
    pub fn total_pages(&self, items_per_page: usize) -> usize {
        if self.directories.is_empty() || items_per_page == 0 {
            0
        } else {
            (self.directories.len() - 1) / items_per_page + 1
        }
    }

    pub fn visible_items(&self, items_per_page: usize) -> Vec<&DirectoryInfo> {
        let start = self.current_page * items_per_page;
        let end = std::cmp::min(start + items_per_page, self.directories.len());
        self.directories.get(start..end).unwrap_or(&[]).iter().collect()
    }

    pub fn visible_selected_index(&self, items_per_page: usize) -> usize {
        self.selected % items_per_page
    }

    pub fn next_page(&mut self, items_per_page: usize) {
        if self.current_page < self.total_pages(items_per_page).saturating_sub(1) {
            self.current_page += 1;
            // Adjust selected to stay within visible range
            self.selected = self.current_page * items_per_page;
        }
    }

    pub fn previous_page(&mut self, items_per_page: usize) {
        if self.current_page > 0 {
            self.current_page -= 1;
            // Adjust selected to stay within visible range
            self.selected = self.current_page * items_per_page;
        }
    }

    pub fn go_to_page(&mut self, page: usize, items_per_page: usize) {
        if page < self.total_pages(items_per_page) {
            self.current_page = page;
            self.selected = page * items_per_page;
        }
    }

    pub fn update_selection_for_pagination(&mut self, items_per_page: usize) {
        // Ensure selected index stays within bounds
        if self.selected >= self.directories.len() {
            self.selected = self.directories.len().saturating_sub(1);
        }
        
        // Update current page based on selection
        self.current_page = self.selected / items_per_page;
    }

    // Selection methods
    pub fn toggle_selection_mode(&mut self) {
        self.selection_mode = !self.selection_mode;
    }

    pub fn toggle_current_selection(&mut self) {
        if !self.directories.is_empty() && self.selected < self.directories.len() {
            self.directories[self.selected].selected = !self.directories[self.selected].selected;
        }
    }

    pub fn select_all(&mut self) {
        for dir in &mut self.directories {
            dir.selected = true;
        }
    }

    pub fn deselect_all(&mut self) {
        for dir in &mut self.directories {
            dir.selected = false;
        }
    }

    pub fn select_current(&mut self) {
        if !self.directories.is_empty() && self.selected < self.directories.len() {
            self.directories[self.selected].selected = true;
        }
    }

    pub fn deselect_current(&mut self) {
        if !self.directories.is_empty() && self.selected < self.directories.len() {
            self.directories[self.selected].selected = false;
        }
    }

    pub fn get_selected_count(&self) -> usize {
        self.directories.iter().filter(|dir| dir.selected).count()
    }

    pub fn get_selected_directories(&self) -> Vec<&DirectoryInfo> {
        self.directories.iter().filter(|dir| dir.selected).collect()
    }

    pub fn get_selected_total_size(&self) -> u64 {
        self.directories.iter()
            .filter(|dir| dir.selected)
            .map(|dir| dir.size)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_directory(path: &str, size: u64) -> DirectoryInfo {
        DirectoryInfo {
            path: path.to_string(),
            size,
            formatted_size: format!("{} B", size),
            selected: false,
        }
    }

    #[test]
    fn test_app_creation() {
        let directories = vec![
            create_test_directory("dir1", 100),
            create_test_directory("dir2", 200)
        ];
        let app = App::new(directories.clone(), "test".to_string(), ".".to_string());
        assert_eq!(app.directories.len(), directories.len());
        assert_eq!(app.selected, 0);
        assert_eq!(app.pattern, "test");
        assert_eq!(app.path, ".");
    }

               #[test]
           fn test_navigation_with_items() {
               let directories = vec![
                   create_test_directory("dir1", 100),
                   create_test_directory("dir2", 200),
                   create_test_directory("dir3", 300)
               ];
               let mut app = App::new(directories, "test".to_string(), ".".to_string());
               let items_per_page = 20;
               
               // Test next
               app.next(items_per_page);
               assert_eq!(app.selected, 1);
               
               app.next(items_per_page);
               assert_eq!(app.selected, 2);
               
               // Test wrapping
               app.next(items_per_page);
               assert_eq!(app.selected, 0);
               
               // Test previous
               app.previous(items_per_page);
               assert_eq!(app.selected, 2);
               
               app.previous(items_per_page);
               assert_eq!(app.selected, 1);
           }

               #[test]
           fn test_navigation_empty_list() {
               let mut app = App::new(vec![], "test".to_string(), ".".to_string());
               let items_per_page = 20;
               
               // Navigation should not panic with empty list
               app.next(items_per_page);
               assert_eq!(app.selected, 0);
               
               app.previous(items_per_page);
               assert_eq!(app.selected, 0);
           }

    #[test]
    fn test_get_selected_directory() {
        let directories = vec![
            create_test_directory("dir1", 100),
            create_test_directory("dir2", 200)
        ];
        let mut app = App::new(directories, "test".to_string(), ".".to_string());
        let items_per_page = 20;
        
        assert_eq!(app.get_selected_directory().unwrap().path, "dir1");
        
        app.next(items_per_page);
        assert_eq!(app.get_selected_directory().unwrap().path, "dir2");
    }

    #[test]
    fn test_get_selected_directory_empty() {
        let app = App::new(vec![], "test".to_string(), ".".to_string());
        assert_eq!(app.get_selected_directory(), None);
    }

    #[test]
    fn test_directory_count() {
        let app = App::new(vec![
            create_test_directory("dir1", 100),
            create_test_directory("dir2", 200)
        ], "test".to_string(), ".".to_string());
        assert_eq!(app.directory_count(), 2);
    }

    #[test]
    fn test_is_empty() {
        let app = App::new(vec![], "test".to_string(), ".".to_string());
        assert!(app.is_empty());
        
        let app = App::new(vec![create_test_directory("dir1", 100)], "test".to_string(), ".".to_string());
        assert!(!app.is_empty());
    }

    #[test]
    fn test_select_first() {
        let directories = vec![
            create_test_directory("dir1", 100),
            create_test_directory("dir2", 200),
            create_test_directory("dir3", 300)
        ];
        let mut app = App::new(directories, "test".to_string(), ".".to_string());
        let items_per_page = 20;
        
        // Move to middle
        app.next(items_per_page);
        assert_eq!(app.selected, 1);
        
        // Select first
        app.select_first();
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn test_select_last() {
        let directories = vec![
            create_test_directory("dir1", 100),
            create_test_directory("dir2", 200),
            create_test_directory("dir3", 300)
        ];
        let mut app = App::new(directories, "test".to_string(), ".".to_string());
        
        // Select last
        app.select_last();
        assert_eq!(app.selected, 2);
    }

    #[test]
    fn test_select_first_empty() {
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());
        app.select_first();
        assert_eq!(app.selected, 0);
    }

               #[test]
           fn test_select_last_empty() {
               let mut app = App::new(vec![], "test".to_string(), ".".to_string());
               app.select_last();
               assert_eq!(app.selected, 0);
           }

           #[test]
           fn test_pagination() {
               let directories = (0..25).map(|i| create_test_directory(&format!("dir{}", i), i as u64 * 100)).collect();
               let mut app = App::new(directories, "test".to_string(), ".".to_string());
               let items_per_page = 20;
               
               // Test total pages (25 items, 20 per page = 2 pages)
               assert_eq!(app.total_pages(items_per_page), 2);
               
               // Test visible items on first page
               let visible = app.visible_items(items_per_page);
               assert_eq!(visible.len(), 20);
               assert_eq!(visible[0].path, "dir0");
               assert_eq!(visible[19].path, "dir19");
               
               // Test next page
               app.next_page(items_per_page);
               assert_eq!(app.current_page, 1);
               assert_eq!(app.selected, 20);
               
               let visible = app.visible_items(items_per_page);
               assert_eq!(visible.len(), 5); // Last page has 5 items
               assert_eq!(visible[0].path, "dir20");
               assert_eq!(visible[4].path, "dir24");
               
               // Test previous page
               app.previous_page(items_per_page);
               assert_eq!(app.current_page, 0);
               assert_eq!(app.selected, 0);
               
               // Test go to specific page
               app.go_to_page(1, items_per_page);
               assert_eq!(app.current_page, 1);
               assert_eq!(app.selected, 20);
               
               let visible = app.visible_items(items_per_page);
               assert_eq!(visible.len(), 5); // Last page has 5 items
               assert_eq!(visible[0].path, "dir20");
               assert_eq!(visible[4].path, "dir24");
           }

           #[test]
           fn test_pagination_bounds() {
               let directories = (0..5).map(|i| create_test_directory(&format!("dir{}", i), i as u64 * 100)).collect();
               let mut app = App::new(directories, "test".to_string(), ".".to_string());
               let items_per_page = 20;
               
               // Test that we can't go beyond bounds
               app.previous_page(items_per_page); // Should not change anything
               assert_eq!(app.current_page, 0);
               
               app.next_page(items_per_page); // Should not change anything (only 1 page)
               assert_eq!(app.current_page, 0);
               
               app.go_to_page(5, items_per_page); // Should not change anything
               assert_eq!(app.current_page, 0);
           }

           #[test]
           fn test_visible_selected_index() {
               let directories = (0..25).map(|i| create_test_directory(&format!("dir{}", i), i as u64 * 100)).collect();
               let mut app = App::new(directories, "test".to_string(), ".".to_string());
               let items_per_page = 20;
               
               // First page, first item
               assert_eq!(app.visible_selected_index(items_per_page), 0);
               
               // First page, last item
               app.selected = 19;
               assert_eq!(app.visible_selected_index(items_per_page), 19);
               
               // Second page, first item
               app.selected = 20;
               app.current_page = 1;
               assert_eq!(app.visible_selected_index(items_per_page), 0);
               
               // Second page, last item
               app.selected = 24;
               assert_eq!(app.visible_selected_index(items_per_page), 4);
           }

    #[test]
    fn test_toggle_current_selection() {
        let mut app = App::new(vec![create_test_directory("dir1", 100)], "test".to_string(), ".".to_string());
        assert_eq!(app.directories[0].selected, false);
        app.toggle_current_selection();
        assert_eq!(app.directories[0].selected, true);
        app.toggle_current_selection();
        assert_eq!(app.directories[0].selected, false);
    }

    #[test]
    fn test_select_all_and_deselect_all() {
        let mut app = App::new(
            vec![create_test_directory("dir1", 100), create_test_directory("dir2", 200)],
            "test".to_string(), ".".to_string(),
        );
        app.select_all();
        assert!(app.directories.iter().all(|d| d.selected));
        app.deselect_all();
        assert!(app.directories.iter().all(|d| !d.selected));
    }

    #[test]
    fn test_select_and_deselect_current() {
        let mut app = App::new(
            vec![create_test_directory("dir1", 100), create_test_directory("dir2", 200)],
            "test".to_string(), ".".to_string(),
        );
        app.select_current();
        assert!(app.directories[0].selected);
        app.deselect_current();
        assert!(!app.directories[0].selected);
        app.selected = 1;
        app.select_current();
        assert!(app.directories[1].selected);
    }

    #[test]
    fn test_get_selected_count_and_directories() {
        let mut app = App::new(
            vec![create_test_directory("dir1", 100), create_test_directory("dir2", 200)],
            "test".to_string(), ".".to_string(),
        );
        assert_eq!(app.get_selected_count(), 0);
        app.directories[0].selected = true;
        assert_eq!(app.get_selected_count(), 1);
        let selected_dirs = app.get_selected_directories();
        assert_eq!(selected_dirs.len(), 1);
        assert_eq!(selected_dirs[0].path, "dir1");
    }

    #[test]
    fn test_get_selected_total_size() {
        let mut app = App::new(
            vec![create_test_directory("dir1", 100), create_test_directory("dir2", 200)],
            "test".to_string(), ".".to_string(),
        );
        assert_eq!(app.get_selected_total_size(), 0);
        app.directories[0].selected = true;
        app.directories[1].selected = true;
        assert_eq!(app.get_selected_total_size(), 300);
    }

    #[test]
    fn test_toggle_selection_mode() {
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());
        assert_eq!(app.selection_mode, false);
        app.toggle_selection_mode();
        assert_eq!(app.selection_mode, true);
        app.toggle_selection_mode();
        assert_eq!(app.selection_mode, false);
    }
} 