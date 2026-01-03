//! Testing utilities for Relm4 components
//!
//! This module provides helper types to facilitate testing of Relm4 components,
//! both regular components and factory components.
//!
//! ## Widget Introspection
//!
//! Both `ComponentTester` and `FactoryComponentTester` support deep widget introspection,
//! allowing you to:
//! - Find widgets by CSS class or type
//! - Search for labels by text content
//! - Count and collect child widgets
//! - Verify widget visibility and state
//!
//! ### Regular Components
//!
//! For regular components, introspection works on the component's root widget:
//!
//! ```ignore
//! let tester = ComponentTester::<MyComponent>::launch(init);
//! tester.send_input(MyInput::SetTitle("Test".to_string()));
//! tester.process_events();
//!
//! // Find and assert on widgets
//! let label = tester.find_label_by_css_class("title").unwrap();
//! assert_eq!(label.text().as_str(), "Test");
//! assert!(label.is_visible());
//! ```
//!
//! ### Factory Components
//!
//! For factory components, introspection works on the parent widget that contains
//! all rendered factory items. This is especially useful for testing lists:
//!
//! ```ignore
//! let mut tester = FactoryComponentTester::<Article>::new(gtk::ListBox::default());
//!
//! // Add multiple items
//! tester.init(ArticleInit { title: "Article 1".to_string(), ... });
//! tester.init(ArticleInit { title: "Article 2".to_string(), ... });
//! tester.init(ArticleInit { title: "Article 3".to_string(), ... });
//! tester.process_events();
//!
//! // Find all titles across all factory items
//! let all_titles = tester.find_all_labels_by_css_class("article-title");
//! assert_eq!(all_titles.len(), 3);
//!
//! // Find a specific article by text
//! let article2 = tester.find_label_with_text("Article 2");
//! assert!(article2.is_some());
//!
//! // Count the number of rendered items
//! assert_eq!(tester.count_factory_children(), 3);
//! ```

use flume::Receiver;
use gtk::prelude::*;
use relm4::factory::{DynamicIndex, FactoryComponent, FactoryVecDeque};
use relm4::gtk;
use relm4::{Component, ComponentController};
use std::time::Duration;

pub mod widget_inspection;

/// A test helper for testing factory components in isolation.
///
/// `FactoryComponentTester` provides a convenient API for:
/// - Initializing factory components
/// - Sending input messages to components
/// - Receiving and asserting on output messages
/// - Accessing component state
///
/// # Example
///
/// ```ignore
/// #[gtk::test]
/// fn test_article_component() {
///     let mut tester = FactoryComponentTester::<Article>::new(gtk::ListBox::default());
///
///     // Initialize a component
///     let index = tester.init(ArticleInit {
///         title: "Test".to_string(),
///         // ...
///     });
///
///     // Send an input message
///     tester.send_input(index, ArticleInput::ArticleSelected);
///
///     // Assert on output
///     let output = tester.try_recv_output();
///     assert!(matches!(output, Some(ArticleOutput::ArticleSelected(..))));
/// }
/// ```
pub struct FactoryComponentTester<C>
where
    C: FactoryComponent<Index = DynamicIndex>,
    C::Output: Send,
{
    factory: FactoryVecDeque<C>,
    output_receiver: Receiver<C::Output>,
    parent_widget: C::ParentWidget,
}

impl<C> FactoryComponentTester<C>
where
    C: FactoryComponent<Index = DynamicIndex>,
    C::Output: Send,
    C::ParentWidget: Clone,
{
    /// Creates a new `FactoryComponentTester` with the given parent widget.
    ///
    /// # Arguments
    ///
    /// * `parent_widget` - The parent widget for the factory (e.g., `gtk::ListBox`)
    pub fn new(parent_widget: C::ParentWidget) -> Self {
        let (output_sender, output_receiver) = flume::unbounded();
        let output_sender_relm: relm4::Sender<C::Output> = output_sender.into();

        let parent_clone = parent_widget.clone();
        let factory = FactoryVecDeque::builder()
            .launch(parent_widget)
            .forward(&output_sender_relm, |msg| msg);

        Self {
            factory,
            output_receiver,
            parent_widget: parent_clone,
        }
    }

    /// Initializes a new component with the given init data.
    ///
    /// Returns the index of the newly created component.
    pub fn init(&mut self, init: C::Init) -> usize {
        let mut guard = self.factory.guard();
        guard.push_back(init);
        guard.len() - 1
    }

    /// Sends an input message to the component at the specified index.
    pub fn send_input(&self, index: usize, input: C::Input) {
        self.factory.send(index, input);
    }

    /// Processes pending GTK/GLib events to ensure messages are handled.
    ///
    /// Call this after sending input messages to ensure outputs are processed.
    pub fn process_events(&self) {
        while gtk::glib::MainContext::default().iteration(false) {}
    }

    /// Attempts to receive an output message without blocking.
    ///
    /// Returns `Some(output)` if a message is available, or `None` if the channel is empty.
    /// Note: Call `process_events()` first to ensure pending messages are processed.
    pub fn try_recv_output(&self) -> Option<C::Output> {
        self.output_receiver.try_recv().ok()
    }

    /// Receives an output message, blocking for up to the specified duration.
    ///
    /// Returns `Some(output)` if a message is received within the timeout, or `None` if the timeout expires.
    pub fn recv_output_timeout(&self, timeout: Duration) -> Option<C::Output> {
        self.output_receiver.recv_timeout(timeout).ok()
    }

    /// Receives an output message, blocking until one is available.
    ///
    /// Returns the output message or an error if the channel is disconnected.
    pub fn recv_output(&self) -> Result<C::Output, flume::RecvError> {
        self.output_receiver.recv()
    }

    /// Provides read-only access to the component at the specified index.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let index = tester.init(MyInit { value: 42 });
    /// tester.get(index, |component| {
    ///     assert_eq!(component.value, 42);
    /// });
    /// ```
    pub fn get<F, R>(&self, index: usize, f: F) -> R
    where
        F: FnOnce(&C) -> R,
    {
        let component = self.factory.get(index).expect("Index out of bounds");
        f(component)
    }

    /// Provides mutable access to the component at the specified index.
    ///
    /// Note: Direct mutation of components should generally be avoided in tests.
    /// Prefer sending input messages via `send_input` instead.
    pub fn get_mut<F, R>(&mut self, index: usize, f: F) -> R
    where
        F: FnOnce(&mut C) -> R,
    {
        let mut guard = self.factory.guard();
        let component = guard.get_mut(index).expect("Index out of bounds");
        f(component)
    }

    /// Returns the number of components in the factory.
    pub fn len(&mut self) -> usize {
        self.factory.guard().len()
    }

    /// Returns true if the factory contains no components.
    pub fn is_empty(&mut self) -> bool {
        self.factory.guard().is_empty()
    }

    /// Provides access to the underlying factory for advanced operations.
    pub fn factory(&self) -> &FactoryVecDeque<C> {
        &self.factory
    }

    /// Returns a reference to the parent widget that contains all factory components.
    ///
    /// This is useful for inspecting the rendered factory components since they
    /// are appended as children to this parent widget.
    pub fn parent_widget(&self) -> &C::ParentWidget {
        &self.parent_widget
    }
}

// Widget inspection methods for factory components
// These work on the parent widget, which contains all factory component children
impl<C> FactoryComponentTester<C>
where
    C: FactoryComponent<Index = DynamicIndex>,
    C::Output: Send,
    C::ParentWidget: Clone + IsA<gtk::Widget>,
{
    /// Finds the first widget with the specified CSS class in the parent widget tree.
    ///
    /// This searches through all factory components rendered in the parent widget.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let tester = FactoryComponentTester::<Article>::new(gtk::ListBox::default());
    /// tester.init(ArticleInit { title: "Test".to_string(), ... });
    /// tester.process_events();
    ///
    /// let label = tester.find_widget_by_css_class("article-title").unwrap();
    /// assert!(label.is_visible());
    /// ```
    pub fn find_widget_by_css_class(&self, css_class: &str) -> Option<gtk::Widget> {
        widget_inspection::find_descendant_by_css_class(&self.parent_widget, css_class)
    }

    /// Finds all widgets with the specified CSS class in the parent widget tree.
    ///
    /// This searches through all factory components rendered in the parent widget.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let widgets = tester.find_all_widgets_by_css_class("article-title");
    /// assert_eq!(widgets.len(), 3); // If you initialized 3 articles
    /// ```
    pub fn find_all_widgets_by_css_class(&self, css_class: &str) -> Vec<gtk::Widget> {
        widget_inspection::find_all_descendants_by_css_class(&self.parent_widget, css_class)
    }

    /// Finds the first label widget with the specified CSS class.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let label = tester.find_label_by_css_class("article-title").unwrap();
    /// assert_eq!(label.text().as_str(), "Test Article");
    /// ```
    pub fn find_label_by_css_class(&self, css_class: &str) -> Option<gtk::Label> {
        self.find_widget_by_css_class(css_class)
            .and_then(|w| w.dynamic_cast::<gtk::Label>().ok())
    }

    /// Finds all label widgets with the specified CSS class.
    ///
    /// Useful when you have multiple factory components with the same CSS class.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let labels = tester.find_all_labels_by_css_class("article-title");
    /// assert_eq!(labels.len(), 3);
    /// assert_eq!(labels[0].text().as_str(), "Article 1");
    /// assert_eq!(labels[1].text().as_str(), "Article 2");
    /// ```
    pub fn find_all_labels_by_css_class(&self, css_class: &str) -> Vec<gtk::Label> {
        self.find_all_widgets_by_css_class(css_class)
            .into_iter()
            .filter_map(|w| w.dynamic_cast::<gtk::Label>().ok())
            .collect()
    }

    /// Finds the first widget of the specified type in the parent widget tree.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let button: Option<gtk::Button> = tester.find_widget_by_type();
    /// assert!(button.is_some());
    /// ```
    pub fn find_widget_by_type<W: IsA<gtk::Widget>>(&self) -> Option<W> {
        widget_inspection::find_descendant_by_type(&self.parent_widget)
    }

    /// Finds all widgets of the specified type in the parent widget tree.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let labels: Vec<gtk::Label> = tester.find_all_widgets_by_type();
    /// assert_eq!(labels.len(), 6); // 2 labels per article, 3 articles
    /// ```
    pub fn find_all_widgets_by_type<W: IsA<gtk::Widget>>(&self) -> Vec<W> {
        widget_inspection::find_all_descendants_by_type(&self.parent_widget)
    }

    /// Finds a label widget with the specified text content.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let label = tester.find_label_with_text("Test Article").unwrap();
    /// assert!(label.is_visible());
    /// ```
    pub fn find_label_with_text(&self, text: &str) -> Option<gtk::Label> {
        widget_inspection::find_label_with_text(&self.parent_widget, text)
    }

    /// Finds a label widget containing the specified text.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let label = tester.find_label_containing_text("Article").unwrap();
    /// assert!(label.is_visible());
    /// ```
    pub fn find_label_containing_text(&self, text: &str) -> Option<gtk::Label> {
        widget_inspection::find_label_containing_text(&self.parent_widget, text)
    }

    /// Counts the number of direct children in the parent widget.
    ///
    /// This typically corresponds to the number of factory components rendered.
    ///
    /// # Example
    ///
    /// ```ignore
    /// assert_eq!(tester.count_factory_children(), 3);
    /// ```
    pub fn count_factory_children(&self) -> usize {
        widget_inspection::count_direct_children(&self.parent_widget)
    }

    /// Collects all direct children of the parent widget.
    ///
    /// Each child typically represents one factory component's root widget.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let children = tester.collect_factory_children();
    /// assert_eq!(children.len(), 3);
    /// ```
    pub fn collect_factory_children(&self) -> Vec<gtk::Widget> {
        widget_inspection::collect_direct_children(&self.parent_widget)
    }

    /// Collects all descendant widgets in the parent widget tree.
    ///
    /// This includes all widgets from all factory components.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let all_widgets = tester.collect_all_widgets();
    /// assert!(all_widgets.len() > 10);
    /// ```
    pub fn collect_all_widgets(&self) -> Vec<gtk::Widget> {
        widget_inspection::collect_all_descendants(&self.parent_widget)
    }

    /// Checks if any factory component has a descendant with the specified CSS class.
    ///
    /// # Example
    ///
    /// ```ignore
    /// assert!(tester.has_widget_with_css_class("article-title"));
    /// ```
    pub fn has_widget_with_css_class(&self, css_class: &str) -> bool {
        widget_inspection::has_descendant_with_css_class(&self.parent_widget, css_class)
    }
}

/// A test helper for testing regular Relm4 components in isolation.
///
/// `ComponentTester` provides a convenient API for:
/// - Initializing components
/// - Sending input messages
/// - Receiving and asserting on output messages
/// - Accessing the root widget
///
/// # Example
///
/// ```ignore
/// #[gtk::test]
/// fn test_my_component() {
///     let tester = ComponentTester::launch(MyInit {
///         title: "Test".to_string(),
///     });
///
///     // Send an input message
///     tester.send_input(MyInput::ButtonClicked);
///
///     // Assert on output
///     let output = tester.try_recv_output();
///     assert!(matches!(output, Some(MyOutput::ActionPerformed)));
/// }
/// ```
pub struct ComponentTester<C>
where
    C: Component,
    C::Output: Clone,
{
    controller: relm4::Controller<C>,
    output_receiver: Receiver<C::Output>,
}

impl<C> ComponentTester<C>
where
    C: Component,
    C::Output: Clone,
    C::Root: IsA<gtk::Widget>,
{
    /// Launches a new component with the given init data.
    pub fn launch(init: C::Init) -> Self {
        let (sender, receiver) = flume::unbounded();
        let sender_relm: relm4::Sender<C::Output> = sender.into();

        let controller = C::builder()
            .launch(init)
            .forward(&sender_relm, |msg| msg.clone());

        Self {
            controller,
            output_receiver: receiver,
        }
    }

    /// Sends an input message to the component.
    pub fn send_input(&self, input: C::Input) {
        self.controller.sender().send(input).unwrap();
    }

    /// Processes pending GTK/GLib events to ensure messages are handled.
    ///
    /// Call this after sending input messages to ensure outputs are processed.
    pub fn process_events(&self) {
        while gtk::glib::MainContext::default().iteration(false) {}
    }

    /// Attempts to receive an output message without blocking.
    ///
    /// Returns `Some(output)` if a message is available, or `None` if the channel is empty.
    /// Note: Call `process_events()` first to ensure pending messages are processed.
    pub fn try_recv_output(&self) -> Option<C::Output> {
        self.output_receiver.try_recv().ok()
    }

    /// Receives an output message, blocking for up to the specified duration.
    ///
    /// Returns `Some(output)` if a message is received within the timeout, or `None` if the timeout expires.
    pub fn recv_output_timeout(&self, timeout: Duration) -> Option<C::Output> {
        self.output_receiver.recv_timeout(timeout).ok()
    }

    /// Receives an output message, blocking until one is available.
    ///
    /// Returns the output message or an error if the channel is disconnected.
    pub fn recv_output(&self) -> Result<C::Output, flume::RecvError> {
        self.output_receiver.recv()
    }

    /// Returns a reference to the component's root widget.
    pub fn widget(&self) -> &C::Root {
        self.controller.widget()
    }

    /// Returns a reference to the underlying controller for advanced operations.
    pub fn controller(&self) -> &relm4::Controller<C> {
        &self.controller
    }

    /// Returns a reference to the component's model.
    ///
    /// This method provides read-only access to the component's state.
    /// Note that it returns a `Ref` guard which dereferences automatically.
    pub fn model(&self) -> std::cell::Ref<'_, C> {
        self.controller.model()
    }

    // Widget inspection methods

    /// Finds the first widget with the specified CSS class in the component's widget tree.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let label = tester.find_widget_by_css_class("article-title").unwrap();
    /// assert!(label.is_visible());
    /// ```
    pub fn find_widget_by_css_class(&self, css_class: &str) -> Option<gtk::Widget> {
        widget_inspection::find_descendant_by_css_class(self.widget(), css_class)
    }

    /// Finds all widgets with the specified CSS class in the component's widget tree.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let labels = tester.find_all_widgets_by_css_class("article-text");
    /// assert_eq!(labels.len(), 3);
    /// ```
    pub fn find_all_widgets_by_css_class(&self, css_class: &str) -> Vec<gtk::Widget> {
        widget_inspection::find_all_descendants_by_css_class(self.widget(), css_class)
    }

    /// Finds the first label widget with the specified CSS class.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let label = tester.find_label_by_css_class("article-title").unwrap();
    /// assert_eq!(label.text().as_str(), "My Article");
    /// ```
    pub fn find_label_by_css_class(&self, css_class: &str) -> Option<gtk::Label> {
        self.find_widget_by_css_class(css_class)
            .and_then(|w| w.dynamic_cast::<gtk::Label>().ok())
    }

    /// Finds the first widget of the specified type in the component's widget tree.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let button: Option<gtk::Button> = tester.find_widget_by_type();
    /// assert!(button.is_some());
    /// ```
    pub fn find_widget_by_type<W: IsA<gtk::Widget>>(&self) -> Option<W> {
        widget_inspection::find_descendant_by_type(self.widget())
    }

    /// Finds all widgets of the specified type in the component's widget tree.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let labels: Vec<gtk::Label> = tester.find_all_widgets_by_type();
    /// assert!(labels.len() > 0);
    /// ```
    pub fn find_all_widgets_by_type<W: IsA<gtk::Widget>>(&self) -> Vec<W> {
        widget_inspection::find_all_descendants_by_type(self.widget())
    }

    /// Finds a label widget with the specified text content.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let label = tester.find_label_with_text("Test Article").unwrap();
    /// assert!(label.is_visible());
    /// ```
    pub fn find_label_with_text(&self, text: &str) -> Option<gtk::Label> {
        widget_inspection::find_label_with_text(self.widget(), text)
    }

    /// Finds a label widget containing the specified text.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let label = tester.find_label_containing_text("Article").unwrap();
    /// assert!(label.is_visible());
    /// ```
    pub fn find_label_containing_text(&self, text: &str) -> Option<gtk::Label> {
        widget_inspection::find_label_containing_text(self.widget(), text)
    }

    /// Counts the number of direct children in the root widget.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let count = tester.count_root_children();
    /// assert_eq!(count, 3);
    /// ```
    pub fn count_root_children(&self) -> usize {
        widget_inspection::count_direct_children(self.widget())
    }

    /// Collects all direct children of the root widget.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let children = tester.collect_root_children();
    /// assert_eq!(children.len(), 3);
    /// ```
    pub fn collect_root_children(&self) -> Vec<gtk::Widget> {
        widget_inspection::collect_direct_children(self.widget())
    }

    /// Collects all descendant widgets of the root widget.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let all_widgets = tester.collect_all_widgets();
    /// assert!(all_widgets.len() > 10);
    /// ```
    pub fn collect_all_widgets(&self) -> Vec<gtk::Widget> {
        widget_inspection::collect_all_descendants(self.widget())
    }

    /// Checks if the component's widget tree has a descendant with the specified CSS class.
    ///
    /// # Example
    ///
    /// ```ignore
    /// assert!(tester.has_widget_with_css_class("article-title"));
    /// ```
    pub fn has_widget_with_css_class(&self, css_class: &str) -> bool {
        widget_inspection::has_descendant_with_css_class(self.widget(), css_class)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relm4::factory::FactorySender;

    #[derive(Debug)]
    struct TestComponent {
        value: i32,
    }

    #[derive(Debug)]
    struct TestInit {
        value: i32,
    }

    #[derive(Debug)]
    enum TestInput {
        Increment,
        Decrement,
    }

    #[derive(Debug, PartialEq)]
    enum TestOutput {
        ValueChanged(i32),
    }

    #[relm4::factory]
    impl FactoryComponent for TestComponent {
        type Init = TestInit;
        type Input = TestInput;
        type Output = TestOutput;
        type CommandOutput = ();
        type ParentWidget = gtk::ListBox;

        view! {
            #[root]
            gtk::Label {
                set_label: &self.value.to_string(),
            }
        }

        fn init_model(
            init: Self::Init,
            _index: &DynamicIndex,
            _sender: FactorySender<Self>,
        ) -> Self {
            Self { value: init.value }
        }

        fn update(&mut self, msg: Self::Input, sender: FactorySender<Self>) {
            match msg {
                TestInput::Increment => {
                    self.value += 1;
                    sender.output(TestOutput::ValueChanged(self.value)).unwrap();
                }
                TestInput::Decrement => {
                    self.value -= 1;
                    sender.output(TestOutput::ValueChanged(self.value)).unwrap();
                }
            }
        }
    }

    #[gtk::test]
    fn test_factory_component_tester_init() {
        let mut tester = FactoryComponentTester::<TestComponent>::new(gtk::ListBox::default());

        let index = tester.init(TestInit { value: 42 });

        tester.get(index, |component: &TestComponent| {
            assert_eq!(component.value, 42);
        });
    }

    #[gtk::test]
    fn test_factory_component_tester_update() {
        let mut tester = FactoryComponentTester::<TestComponent>::new(gtk::ListBox::default());

        let index = tester.init(TestInit { value: 10 });

        tester.send_input(index, TestInput::Increment);
        tester.process_events();

        // Verify state changed
        tester.get(index, |component: &TestComponent| {
            assert_eq!(component.value, 11);
        });
    }

    #[gtk::test]
    fn test_factory_component_tester_multiple_updates() {
        let mut tester = FactoryComponentTester::<TestComponent>::new(gtk::ListBox::default());

        let index = tester.init(TestInit { value: 0 });

        tester.send_input(index, TestInput::Increment);
        tester.send_input(index, TestInput::Increment);
        tester.send_input(index, TestInput::Decrement);
        tester.process_events();

        // Final state should be 1
        tester.get(index, |component: &TestComponent| {
            assert_eq!(component.value, 1);
        });
    }

    #[gtk::test]
    fn test_factory_component_widget_introspection() {
        let mut tester = FactoryComponentTester::<TestComponent>::new(gtk::ListBox::default());

        // Initialize multiple components
        tester.init(TestInit { value: 10 });
        tester.init(TestInit { value: 20 });
        tester.init(TestInit { value: 30 });
        tester.process_events();

        // Test counting factory children
        assert_eq!(tester.count_factory_children(), 3);

        // Test finding all labels (each factory component has one label)
        let all_labels: Vec<gtk::Label> = tester.find_all_widgets_by_type();
        assert_eq!(all_labels.len(), 3);

        // Verify the labels contain the expected text
        let labels_text: Vec<String> = all_labels.iter().map(|l| l.text().to_string()).collect();
        assert!(labels_text.contains(&"10".to_string()));
        assert!(labels_text.contains(&"20".to_string()));
        assert!(labels_text.contains(&"30".to_string()));
    }

    #[gtk::test]
    fn test_factory_component_find_by_text() {
        let mut tester = FactoryComponentTester::<TestComponent>::new(gtk::ListBox::default());

        tester.init(TestInit { value: 42 });
        tester.init(TestInit { value: 99 });
        tester.process_events();

        // Find label with specific text
        let label = tester.find_label_with_text("42");
        assert!(label.is_some());
        assert_eq!(label.unwrap().text().as_str(), "42");

        // Find another label
        let label2 = tester.find_label_with_text("99");
        assert!(label2.is_some());
    }

    #[gtk::test]
    fn test_factory_component_collect_children() {
        let mut tester = FactoryComponentTester::<TestComponent>::new(gtk::ListBox::default());

        tester.init(TestInit { value: 1 });
        tester.init(TestInit { value: 2 });
        tester.process_events();

        // Collect all factory children (direct children of parent)
        let children = tester.collect_factory_children();
        assert_eq!(children.len(), 2);

        // Collect all widgets (should include the parent and all descendants)
        let all_widgets = tester.collect_all_widgets();
        assert!(all_widgets.len() >= 2); // At least the 2 label widgets
    }
}
