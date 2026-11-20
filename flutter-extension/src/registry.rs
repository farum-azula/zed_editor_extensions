//! A registry of the extension's parts of one kind (debug adapters, language servers, debug
//! locators), looked up by the name Zed knows them by: the key of their section in
//! `extension.toml`. A new part is one type implementing the kind's trait and one registration.

/// A part of the extension that Zed asks for by name (`Send + Sync`, as the extension is).
pub trait Named: Send + Sync {
    /// The name in `extension.toml`: `Flutter` of `[debug_adapters.Flutter]`.
    fn name(&self) -> &'static str;
}

/// The parts of one kind, in the order they were registered.
pub struct Registry<T: ?Sized + Named> {
    /// What the parts are, for the error of an unknown name: `debug adapter`.
    kind: &'static str,
    entries: Vec<Box<T>>,
}

impl<T: ?Sized + Named> Registry<T> {
    pub fn new(kind: &'static str) -> Self {
        Self {
            kind,
            entries: Vec::new(),
        }
    }

    /// Adds [entry]; a second part of the same name is a programming error.
    pub fn with(mut self, entry: Box<T>) -> Self {
        assert!(
            self.find(entry.name()).is_none(),
            "two {}s named {}",
            self.kind,
            entry.name()
        );
        self.entries.push(entry);
        self
    }

    /// The part named [name], if there is one.
    pub fn find(&self, name: &str) -> Option<&T> {
        self.entries
            .iter()
            .map(Box::as_ref)
            .find(|entry| entry.name() == name)
    }

    /// The part named [name], or an error naming what is registered.
    pub fn get(&self, name: &str) -> Result<&T, String> {
        self.find(name).ok_or_else(|| {
            format!(
                "Flutter Tools has no {} named {name:?} (it has {})",
                self.kind,
                self.names().join(", ")
            )
        })
    }

    /// The names of the parts, in registration order.
    pub fn names(&self) -> Vec<&'static str> {
        self.entries.iter().map(|entry| entry.name()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    trait Part: Named {}

    struct One;
    impl Named for One {
        fn name(&self) -> &'static str {
            "one"
        }
    }
    impl Part for One {}

    struct Two;
    impl Named for Two {
        fn name(&self) -> &'static str {
            "two"
        }
    }
    impl Part for Two {}

    fn registry() -> Registry<dyn Part> {
        Registry::<dyn Part>::new("part")
            .with(Box::new(One))
            .with(Box::new(Two))
    }

    #[test]
    fn finds_a_part_by_name() {
        assert_eq!(registry().find("two").map(|part| part.name()), Some("two"));
        assert!(registry().find("three").is_none());
    }

    #[test]
    fn an_unknown_name_is_an_error_listing_the_known_ones() {
        let error = registry().get("three").err().unwrap();
        assert_eq!(
            error,
            "Flutter Tools has no part named \"three\" (it has one, two)"
        );
    }

    #[test]
    #[should_panic(expected = "two parts named one")]
    fn a_name_registers_once() {
        let _ = registry().with(Box::new(One));
    }
}
