//! Widget inspection utilities for testing GTK widgets
//!
//! This module provides functions to find, traverse, and inspect GTK widget hierarchies
//! during testing. These utilities make it easier to write deep assertions about widget
//! state and structure.

use gtk::prelude::*;
use relm4::gtk;

/// Recursively searches for a widget with the specified CSS class in the widget tree.
///
/// Returns the first widget found with the given CSS class, or `None` if no match is found.
///
/// # Example
///
/// ```ignore
/// let label = find_descendant_by_css_class(&root_widget, "article-title");
/// assert!(label.is_some());
/// ```
pub fn find_descendant_by_css_class(
    widget: &impl IsA<gtk::Widget>,
    css_class: &str,
) -> Option<gtk::Widget> {
    let widget = widget.as_ref();

    // Check if this widget has the CSS class
    if widget.has_css_class(css_class) {
        return Some(widget.clone());
    }

    // Recursively check children
    let mut child = widget.first_child();
    while let Some(c) = child {
        if let Some(found) = find_descendant_by_css_class(&c, css_class) {
            return Some(found);
        }
        child = c.next_sibling();
    }

    None
}

/// Recursively finds all widgets with the specified CSS class in the widget tree.
///
/// Returns a vector of all widgets found with the given CSS class.
///
/// # Example
///
/// ```ignore
/// let labels = find_all_descendants_by_css_class(&root_widget, "article-text");
/// assert_eq!(labels.len(), 3);
/// ```
pub fn find_all_descendants_by_css_class(
    widget: &impl IsA<gtk::Widget>,
    css_class: &str,
) -> Vec<gtk::Widget> {
    let mut results = Vec::new();
    let widget = widget.as_ref();

    // Check if this widget has the CSS class
    if widget.has_css_class(css_class) {
        results.push(widget.clone());
    }

    // Recursively check children
    let mut child = widget.first_child();
    while let Some(c) = child {
        results.extend(find_all_descendants_by_css_class(&c, css_class));
        child = c.next_sibling();
    }

    results
}

/// Recursively searches for a widget of the specified type in the widget tree.
///
/// Returns the first widget found of the given type, or `None` if no match is found.
///
/// # Example
///
/// ```ignore
/// let label: Option<gtk::Label> = find_descendant_by_type(&root_widget);
/// assert!(label.is_some());
/// ```
pub fn find_descendant_by_type<W: IsA<gtk::Widget>>(widget: &impl IsA<gtk::Widget>) -> Option<W> {
    let widget = widget.as_ref();

    // Try to downcast this widget
    if let Some(typed_widget) = widget.clone().dynamic_cast::<W>().ok() {
        return Some(typed_widget);
    }

    // Recursively check children
    let mut child = widget.first_child();
    while let Some(c) = child {
        if let Some(found) = find_descendant_by_type::<W>(&c) {
            return Some(found);
        }
        child = c.next_sibling();
    }

    None
}

/// Recursively finds all widgets of the specified type in the widget tree.
///
/// Returns a vector of all widgets found of the given type.
///
/// # Example
///
/// ```ignore
/// let labels: Vec<gtk::Label> = find_all_descendants_by_type(&root_widget);
/// assert_eq!(labels.len(), 5);
/// ```
pub fn find_all_descendants_by_type<W: IsA<gtk::Widget>>(widget: &impl IsA<gtk::Widget>) -> Vec<W> {
    let mut results = Vec::new();
    let widget = widget.as_ref();

    // Try to downcast this widget
    if let Ok(typed_widget) = widget.clone().dynamic_cast::<W>() {
        results.push(typed_widget);
    }

    // Recursively check children
    let mut child = widget.first_child();
    while let Some(c) = child {
        results.extend(find_all_descendants_by_type::<W>(&c));
        child = c.next_sibling();
    }

    results
}

/// Finds a label widget with the specified text content.
///
/// Returns the first label found with matching text, or `None` if no match is found.
///
/// # Example
///
/// ```ignore
/// let label = find_label_with_text(&root_widget, "Test Article");
/// assert!(label.is_some());
/// ```
pub fn find_label_with_text(widget: &impl IsA<gtk::Widget>, text: &str) -> Option<gtk::Label> {
    let labels: Vec<gtk::Label> = find_all_descendants_by_type(widget);
    labels.into_iter().find(|label| label.text() == text)
}

/// Finds a label widget containing the specified text.
///
/// Returns the first label found containing the text, or `None` if no match is found.
///
/// # Example
///
/// ```ignore
/// let label = find_label_containing_text(&root_widget, "Article");
/// assert!(label.is_some());
/// ```
pub fn find_label_containing_text(
    widget: &impl IsA<gtk::Widget>,
    text: &str,
) -> Option<gtk::Label> {
    let labels: Vec<gtk::Label> = find_all_descendants_by_type(widget);
    labels
        .into_iter()
        .find(|label| label.text().as_str().contains(text))
}

/// Collects all child widgets (non-recursively) of the given widget.
///
/// Returns a vector of direct children only.
///
/// # Example
///
/// ```ignore
/// let children = collect_direct_children(&container);
/// assert_eq!(children.len(), 3);
/// ```
pub fn collect_direct_children(widget: &impl IsA<gtk::Widget>) -> Vec<gtk::Widget> {
    let mut children = Vec::new();
    let widget = widget.as_ref();

    let mut child = widget.first_child();
    while let Some(c) = child {
        children.push(c.clone());
        child = c.next_sibling();
    }

    children
}

/// Recursively collects all descendant widgets of the given widget.
///
/// Returns a vector of all descendants in depth-first order.
///
/// # Example
///
/// ```ignore
/// let all_widgets = collect_all_descendants(&root_widget);
/// assert!(all_widgets.len() > 10);
/// ```
pub fn collect_all_descendants(widget: &impl IsA<gtk::Widget>) -> Vec<gtk::Widget> {
    let mut descendants = Vec::new();
    let widget = widget.as_ref();

    let mut child = widget.first_child();
    while let Some(c) = child {
        descendants.push(c.clone());
        descendants.extend(collect_all_descendants(&c));
        child = c.next_sibling();
    }

    descendants
}

/// Counts the number of direct children of a widget.
///
/// # Example
///
/// ```ignore
/// let count = count_direct_children(&container);
/// assert_eq!(count, 5);
/// ```
pub fn count_direct_children(widget: &impl IsA<gtk::Widget>) -> usize {
    let widget = widget.as_ref();
    let mut count = 0;
    let mut child = widget.first_child();
    while let Some(c) = child {
        count += 1;
        child = c.next_sibling();
    }
    count
}

/// Checks if a widget or any of its descendants has the specified CSS class.
///
/// # Example
///
/// ```ignore
/// assert!(has_descendant_with_css_class(&root_widget, "article-title"));
/// ```
pub fn has_descendant_with_css_class(widget: &impl IsA<gtk::Widget>, css_class: &str) -> bool {
    find_descendant_by_css_class(widget, css_class).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gtk::test]
    fn test_find_descendant_by_css_class() {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let label = gtk::Label::new(Some("Test"));
        label.add_css_class("test-class");
        container.append(&label);

        let found = find_descendant_by_css_class(&container, "test-class");
        assert!(found.is_some());
    }

    #[gtk::test]
    fn test_find_all_descendants_by_css_class() {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let label1 = gtk::Label::new(Some("Test 1"));
        label1.add_css_class("test-class");
        let label2 = gtk::Label::new(Some("Test 2"));
        label2.add_css_class("test-class");
        container.append(&label1);
        container.append(&label2);

        let found = find_all_descendants_by_css_class(&container, "test-class");
        assert_eq!(found.len(), 2);
    }

    #[gtk::test]
    fn test_find_descendant_by_type() {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let label = gtk::Label::new(Some("Test"));
        container.append(&label);

        let found: Option<gtk::Label> = find_descendant_by_type(&container);
        assert!(found.is_some());
        assert_eq!(found.unwrap().text().as_str(), "Test");
    }

    #[gtk::test]
    fn test_find_label_with_text() {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let label = gtk::Label::new(Some("Specific Text"));
        container.append(&label);

        let found = find_label_with_text(&container, "Specific Text");
        assert!(found.is_some());
    }

    #[gtk::test]
    fn test_count_direct_children() {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        container.append(&gtk::Label::new(Some("1")));
        container.append(&gtk::Label::new(Some("2")));
        container.append(&gtk::Label::new(Some("3")));

        let count = count_direct_children(&container);
        assert_eq!(count, 3);
    }

    #[gtk::test]
    fn test_nested_search() {
        let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let inner = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let label = gtk::Label::new(Some("Nested"));
        label.add_css_class("nested-class");

        inner.append(&label);
        outer.append(&inner);

        let found = find_descendant_by_css_class(&outer, "nested-class");
        assert!(found.is_some());
    }
}
