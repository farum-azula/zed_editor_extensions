//! Where client classes are registered under a name, away from the class itself:
//! `@ImportHttpServices(group: 'catalog', types: [BookService])` on the application puts
//! `BookService` in the `catalog` group, whose base URL is `spring.http.serviceclient.catalog.base-url`.
//! The registration is looked up in the class's file, then in the other files of its project.

use super::compile::compile_patterns;
use super::model::RegistrationEntry;
use crate::pattern::Matcher;
use crate::server;
use std::cell::OnceCell;
use std::fs;
use std::path::Path;

/// The compiled `registrations` of a framework.
pub(super) struct Registrations {
    matcher: Matcher,
    /// Per pattern, the name it registers under when it captures no `{name}`.
    names: Vec<Option<String>>,
}

/// A name, and the text listing the classes registered under it (`[BookService, AuthorService]`).
struct Registered {
    name: String,
    classes: String,
}

impl Registrations {
    pub fn compile(entries: Vec<RegistrationEntry>, context: &str) -> Result<Option<Registrations>, String> {
        let (patterns, names): (Vec<String>, Vec<Option<String>>) =
            entries.into_iter().map(RegistrationEntry::into_parts).unzip();
        Ok(compile_patterns(&patterns, context)?.map(|matcher| Registrations { matcher, names }))
    }

    /// The registrations in `text`: a `{name}` (or the pattern's own name) and the `{keys}` list
    /// of classes, or a single `{type}`.
    fn registered<'a>(&'a self, text: &'a str) -> impl Iterator<Item = Registered> + 'a {
        self.matcher.hits(text).filter_map(|hit| {
            let name = hit.get("name").map(str::to_string).or_else(|| self.names[hit.alt].clone())?;
            let classes = hit.get("keys").or(hit.get("type"))?.to_string();
            Some(Registered { name, classes })
        })
    }
}

/// The registrations a file's routes are looked up in: the file's own, then its project's,
/// read once and only if the file has none for a class.
pub(super) struct RegistrationLookup<'a> {
    registrations: &'a Registrations,
    text: &'a str,
    source: &'a Path,
    extensions: &'a [String],
    project: OnceCell<Vec<Registered>>,
}

impl<'a> RegistrationLookup<'a> {
    pub fn new(registrations: &'a Registrations, text: &'a str, source: &'a Path, extensions: &'a [String]) -> Self {
        RegistrationLookup { registrations, text, source, extensions, project: OnceCell::new() }
    }

    /// The name the class `class` is registered under.
    pub fn name_of(&self, class: &str) -> Option<String> {
        let in_file = self.registrations.registered(self.text).find(|registered| lists(&registered.classes, class));
        let found = in_file.map(|registered| registered.name);
        found.or_else(|| {
            let in_project = self.project().iter().find(|registered| lists(&registered.classes, class))?;
            Some(in_project.name.clone())
        })
    }

    /// The registrations in the project's other source files of the same language.
    fn project(&self) -> &[Registered] {
        self.project.get_or_init(|| {
            let files = server::project_files(self.source, self.extensions);
            let texts = files
                .iter()
                .filter(|file| file.as_path() != self.source)
                .filter_map(|file| fs::read_to_string(file).ok());
            texts.flat_map(|text| self.registrations.registered(&text).collect::<Vec<_>>()).collect()
        })
    }
}

/// Does the list `classes` (`[BookService, AuthorService]`, `{BookService.class}`) name `class`,
/// as a whole word?
fn lists(classes: &str, class: &str) -> bool {
    let is_word = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    classes.match_indices(class).any(|(at, _)| {
        let before = classes[..at].chars().next_back();
        let after = classes[at + class.len()..].chars().next();
        !before.is_some_and(is_word) && !after.is_some_and(is_word)
    })
}
