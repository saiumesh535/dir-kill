use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

/// Custom directory list widget
pub struct DirectoryList {
    items: Vec<String>,
    selected: usize,
}

impl DirectoryList {
    pub fn new(items: Vec<String>) -> Self {
        Self {
            items,
            selected: 0,
        }
    }

    pub fn set_selected(&mut self, selected: usize) {
        if !self.items.is_empty() {
            self.selected = selected.min(self.items.len() - 1);
        }
    }

    pub fn get_selected(&self) -> usize {
        self.selected
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Convert to ratatui List widget
    pub fn to_list_widget(&self) -> List {
        let list_items: Vec<ListItem> = self.items.iter().enumerate().map(|(index, dir)| {
            let is_selected = index == self.selected;
            let style = if is_selected {
                Style::default().fg(Color::Black).bg(Color::White)
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(vec![Line::from(vec![
                Span::styled("📁 ", Style::default().fg(Color::Yellow)),
                Span::styled(dir, style),
            ])])
        }).collect();

        List::new(list_items)
            .block(Block::default().borders(Borders::ALL).title("Directories"))
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Black).bg(Color::White))
    }


}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_directory_list_creation() {
        let items = vec!["dir1".to_string(), "dir2".to_string()];
        let list = DirectoryList::new(items.clone());
        assert_eq!(list.items, items);
        assert_eq!(list.selected, 0);
    }

    #[test]
    fn test_set_selected() {
        let items = vec!["dir1".to_string(), "dir2".to_string(), "dir3".to_string()];
        let mut list = DirectoryList::new(items);
        
        list.set_selected(1);
        assert_eq!(list.selected, 1);
        
        list.set_selected(5); // Out of bounds
        assert_eq!(list.selected, 2); // Should clamp to max index
    }

    #[test]
    fn test_set_selected_empty_list() {
        let mut list = DirectoryList::new(vec![]);
        list.set_selected(5);
        assert_eq!(list.selected, 0);
    }

    #[test]
    fn test_get_selected() {
        let items = vec!["dir1".to_string(), "dir2".to_string()];
        let mut list = DirectoryList::new(items);
        
        assert_eq!(list.get_selected(), 0);
        
        list.set_selected(1);
        assert_eq!(list.get_selected(), 1);
    }

    #[test]
    fn test_item_count() {
        let list = DirectoryList::new(vec!["dir1".to_string(), "dir2".to_string()]);
        assert_eq!(list.item_count(), 2);
    }

    #[test]
    fn test_is_empty() {
        let list = DirectoryList::new(vec![]);
        assert!(list.is_empty());
        
        let list = DirectoryList::new(vec!["dir1".to_string()]);
        assert!(!list.is_empty());
    }


} 