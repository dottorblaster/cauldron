//! Testing utilities for Relm4 components
//!
//! This module provides helper types to facilitate testing of Relm4 components,
//! both regular components and factory components.

use flume::Receiver;
use relm4::factory::{DynamicIndex, FactoryComponent, FactoryVecDeque};
use relm4::gtk;
use relm4::{Component, ComponentController};
use std::time::Duration;

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
}

impl<C> FactoryComponentTester<C>
where
    C: FactoryComponent<Index = DynamicIndex>,
    C::Output: Send,
{
    /// Creates a new `FactoryComponentTester` with the given parent widget.
    ///
    /// # Arguments
    ///
    /// * `parent_widget` - The parent widget for the factory (e.g., `gtk::ListBox`)
    pub fn new(parent_widget: C::ParentWidget) -> Self {
        let (output_sender, output_receiver) = flume::unbounded();
        let output_sender_relm: relm4::Sender<C::Output> = output_sender.into();

        let factory = FactoryVecDeque::builder()
            .launch(parent_widget)
            .forward(&output_sender_relm, |msg| msg);

        Self {
            factory,
            output_receiver,
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
}
