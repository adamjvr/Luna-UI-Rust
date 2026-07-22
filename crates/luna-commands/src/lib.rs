// SPDX-License-Identifier: MPL-2.0

//! Typed command registration, key binding, and dispatch requests.
//!
//! Luna commands describe reusable intent without embedding application behavior in widgets or
//! native hosts. Moth Text may register product commands, while Luna widgets may emit requests
//! against those IDs. The registry remains deterministic and contains no callbacks, which keeps
//! command resolution testable and avoids hidden ownership or thread-affinity requirements.

use luna_core::NodeId;
use luna_input::{Key, KeyboardEvent, Modifiers};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Stable validated identifier for a command.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CommandId(String);

impl CommandId {
    /// Creates a command ID.
    ///
    /// IDs must be non-empty and cannot contain whitespace. Dotted names such as
    /// `luna.window.close` are encouraged because they remain readable in traces and fixtures.
    pub fn new(value: impl Into<String>) -> Result<Self, CommandIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(CommandIdError::Empty);
        }
        if value.chars().any(char::is_whitespace) {
            return Err(CommandIdError::ContainsWhitespace(value));
        }
        Ok(Self(value))
    }

    /// Returns the ID as a borrowed string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for CommandId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Validation failure returned while constructing a [`CommandId`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandIdError {
    /// The identifier was empty.
    Empty,
    /// The identifier contained whitespace.
    ContainsWhitespace(String),
}

impl Display for CommandIdError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("a Luna command identifier cannot be empty"),
            Self::ContainsWhitespace(value) => {
                write!(
                    formatter,
                    "Luna command identifier contains whitespace: {value:?}"
                )
            }
        }
    }
}

impl Error for CommandIdError {}

/// Static user-facing command metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandDefinition {
    /// Stable command identity.
    pub id: CommandId,
    /// Short display label suitable for menus and command palettes.
    pub title: String,
    /// Optional longer explanation.
    pub description: Option<String>,
}

impl CommandDefinition {
    /// Creates a command definition with no long description.
    #[must_use]
    pub fn new(id: CommandId, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            description: None,
        }
    }

    /// Adds a longer description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// One normalized keyboard chord.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct KeyChord {
    /// Logical key after keyboard-layout processing.
    pub key: Key,
    /// Required modifier set.
    pub modifiers: Modifiers,
}

impl KeyChord {
    /// Creates a keyboard chord.
    #[must_use]
    pub fn new(key: Key, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }
}

/// One keyboard binding from a chord to a command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyBinding {
    /// Keyboard chord that activates the binding.
    pub chord: KeyChord,
    /// Command requested by the chord.
    pub command: CommandId,
    /// Whether operating-system key-repeat events may activate the command.
    pub allow_repeat: bool,
}

impl KeyBinding {
    /// Creates a non-repeating binding.
    #[must_use]
    pub fn new(chord: KeyChord, command: CommandId) -> Self {
        Self {
            chord,
            command,
            allow_repeat: false,
        }
    }

    /// Enables or disables key-repeat activation.
    #[must_use]
    pub fn with_repeat(mut self, allow_repeat: bool) -> Self {
        self.allow_repeat = allow_repeat;
        self
    }
}

/// Origin of a command dispatch request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandSource {
    /// A registered keyboard binding resolved the command.
    KeyBinding(KeyChord),
    /// An accessibility action requested the command.
    Accessibility,
    /// Application code requested the command directly.
    Application,
    /// A widget requested the command from a pointer or semantic action.
    Widget(NodeId),
}

/// Product-neutral request to dispatch one command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandRequest {
    /// Command to invoke.
    pub command: CommandId,
    /// Request origin.
    pub source: CommandSource,
    /// Optional semantic target node.
    pub target: Option<NodeId>,
}

impl CommandRequest {
    /// Creates a request without a target node.
    #[must_use]
    pub fn new(command: CommandId, source: CommandSource) -> Self {
        Self {
            command,
            source,
            target: None,
        }
    }

    /// Adds a semantic target node.
    #[must_use]
    pub fn with_target(mut self, target: NodeId) -> Self {
        self.target = Some(target);
        self
    }
}

/// Deterministic registry of command metadata and keyboard bindings.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CommandRegistry {
    definitions: BTreeMap<CommandId, CommandDefinition>,
    bindings: BTreeMap<KeyChord, KeyBinding>,
}

impl CommandRegistry {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            definitions: BTreeMap::new(),
            bindings: BTreeMap::new(),
        }
    }

    /// Registers static command metadata.
    pub fn register_command(
        &mut self,
        definition: CommandDefinition,
    ) -> Result<(), CommandRegistryError> {
        if self.definitions.contains_key(&definition.id) {
            return Err(CommandRegistryError::DuplicateCommand(definition.id));
        }
        self.definitions.insert(definition.id.clone(), definition);
        Ok(())
    }

    /// Registers one keyboard binding.
    pub fn register_binding(&mut self, binding: KeyBinding) -> Result<(), CommandRegistryError> {
        if !self.definitions.contains_key(&binding.command) {
            return Err(CommandRegistryError::UnknownCommand(binding.command));
        }
        if self.bindings.contains_key(&binding.chord) {
            return Err(CommandRegistryError::DuplicateBinding(binding.chord));
        }
        self.bindings.insert(binding.chord.clone(), binding);
        Ok(())
    }

    /// Looks up command metadata by ID.
    #[must_use]
    pub fn command(&self, id: &CommandId) -> Option<&CommandDefinition> {
        self.definitions.get(id)
    }

    /// Iterates command metadata in stable identifier order.
    pub fn commands(&self) -> impl Iterator<Item = &CommandDefinition> {
        self.definitions.values()
    }

    /// Resolves a normalized keyboard event into a dispatch request.
    ///
    /// Releases are ignored. Repeat events are ignored unless the matching binding explicitly
    /// opts in. The registry does not execute behavior; it only produces immutable intent.
    #[must_use]
    pub fn resolve_keyboard(&self, event: &KeyboardEvent) -> Option<CommandRequest> {
        if !event.is_pressed {
            return None;
        }
        let chord = KeyChord::new(event.key.clone(), event.modifiers);
        let binding = self.bindings.get(&chord)?;
        if event.is_repeat && !binding.allow_repeat {
            return None;
        }
        Some(CommandRequest::new(
            binding.command.clone(),
            CommandSource::KeyBinding(chord),
        ))
    }
}

/// Registration failure for [`CommandRegistry`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandRegistryError {
    /// A command ID was already registered.
    DuplicateCommand(CommandId),
    /// A binding referenced a command not present in the registry.
    UnknownCommand(CommandId),
    /// A chord was already bound.
    DuplicateBinding(KeyChord),
}

impl Display for CommandRegistryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateCommand(id) => write!(formatter, "duplicate Luna command: {id}"),
            Self::UnknownCommand(id) => {
                write!(
                    formatter,
                    "key binding references unknown Luna command: {id}"
                )
            }
            Self::DuplicateBinding(chord) => {
                write!(formatter, "duplicate Luna key binding: {chord:?}")
            }
        }
    }
}

impl Error for CommandRegistryError {}

#[cfg(test)]
mod tests {
    use super::{CommandDefinition, CommandId, CommandRegistry, KeyBinding, KeyChord};
    use luna_input::{Key, KeyboardEvent, Modifiers};
    use std::error::Error;

    #[test]
    fn registered_chord_resolves_to_typed_request() -> Result<(), Box<dyn Error>> {
        let command = CommandId::new("luna.demo.toggle")?;
        let chord = KeyChord::new(Key::Character("p".to_owned()), Modifiers::CONTROL);
        let mut registry = CommandRegistry::new();
        registry.register_command(CommandDefinition::new(command.clone(), "Toggle panel"))?;
        registry.register_binding(KeyBinding::new(chord, command.clone()))?;

        let request = registry
            .resolve_keyboard(&KeyboardEvent {
                key: Key::Character("p".to_owned()),
                is_pressed: true,
                is_repeat: false,
                modifiers: Modifiers::CONTROL,
                timestamp_micros: 10,
            })
            .ok_or_else(|| std::io::Error::other("binding should resolve"))?;

        assert_eq!(request.command, command);
        Ok(())
    }

    #[test]
    fn repeats_are_rejected_by_default() -> Result<(), Box<dyn Error>> {
        let command = CommandId::new("luna.demo.once")?;
        let chord = KeyChord::new(Key::Character("x".to_owned()), Modifiers::NONE);
        let mut registry = CommandRegistry::new();
        registry.register_command(CommandDefinition::new(command.clone(), "Run once"))?;
        registry.register_binding(KeyBinding::new(chord, command))?;

        assert!(
            registry
                .resolve_keyboard(&KeyboardEvent {
                    key: Key::Character("x".to_owned()),
                    is_pressed: true,
                    is_repeat: true,
                    modifiers: Modifiers::NONE,
                    timestamp_micros: 20,
                })
                .is_none()
        );
        Ok(())
    }
}
