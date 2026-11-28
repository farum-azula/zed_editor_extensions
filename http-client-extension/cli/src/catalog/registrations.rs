//! Where client classes are registered under a name, away from the class itself:
//! `@ImportHttpServices(group: 'catalog', types: [BookService])` on the application puts
//! `BookService` in the `catalog` group, whose base URL is `spring.http.serviceclient.catalog.base-url`.
//! The registration is looked up in the class's file, then in the other files of its project.
//! A group named by a constant (`group: EdgeFunctions.group`) is the string the constant holds:
//! declared in the project's files, else in the framework's own table of its packages' constants.

use super::compile::compile_patterns;
use super::model::RegistrationEntry;
use crate::pattern::Matcher;
use crate::server;
use regex::Regex;
use std::cell::OnceCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// The compiled `registrations` of a framework.
pub(super) struct Registrations {
    matcher: Matcher,
    /// Per pattern, the name it registers under when it captures no `{name}`.
    names: Vec<Option<String>>,
    /// The values of the framework's own constants (`EdgeFunctions.group`).
    constants: BTreeMap<String, String>,
}

/// The name classes are registered under: written out, or the constant holding it.
enum Name {
    Literal(String),
    Constant(String),
}

/// A name, and the text listing the classes registered under it (`[BookService, AuthorService]`).
struct Registered {
    name: Name,
    classes: String,
}

impl Registrations {
    pub fn compile(
        entries: Vec<RegistrationEntry>,
        constants: BTreeMap<String, String>,
        context: &str,
    ) -> Result<Option<Registrations>, String> {
        let (patterns, names): (Vec<String>, Vec<Option<String>>) =
            entries.into_iter().map(RegistrationEntry::into_parts).unzip();
        Ok(compile_patterns(&patterns, context)?.map(|matcher| Registrations { matcher, names, constants }))
    }

    /// The registrations in `text`: a `{name}`, a `{ref}` to the constant holding it (or the
    /// pattern's own name), and the `{keys}` list of classes, or a single `{type}`.
    fn registered<'a>(&'a self, text: &'a str) -> impl Iterator<Item = Registered> + 'a {
        self.matcher.hits(text).filter_map(|hit| {
            let name = match (hit.get("name"), hit.get("ref")) {
                (Some(name), _) => Name::Literal(name.to_string()),
                (None, Some(reference)) => Name::Constant(reference.to_string()),
                (None, None) => Name::Literal(self.names[hit.alt].clone()?),
            };
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
    project_texts: OnceCell<Vec<String>>,
    project: OnceCell<Vec<Registered>>,
}

impl<'a> RegistrationLookup<'a> {
    pub fn new(registrations: &'a Registrations, text: &'a str, source: &'a Path, extensions: &'a [String]) -> Self {
        RegistrationLookup {
            registrations,
            text,
            source,
            extensions,
            project_texts: OnceCell::new(),
            project: OnceCell::new(),
        }
    }

    /// The name the class `class` is registered under.
    pub fn name_of(&self, class: &str) -> Option<String> {
        let in_file = self.registrations.registered(self.text).find(|registered| lists(&registered.classes, class));
        let name = match in_file {
            Some(registered) => registered.name,
            None => {
                let in_project = self.project().iter().find(|registered| lists(&registered.classes, class))?;
                match &in_project.name {
                    Name::Literal(name) => Name::Literal(name.clone()),
                    Name::Constant(reference) => Name::Constant(reference.clone()),
                }
            }
        };
        match name {
            Name::Literal(name) => Some(name),
            Name::Constant(reference) => self.value_of(&reference),
        }
    }

    /// The string the constant `reference` holds (`Groups.catalog`, `CATALOG`): the framework's
    /// table first, then a declaration in the file, then in the rest of the project.
    fn value_of(&self, reference: &str) -> Option<String> {
        if let Some(value) = self.registrations.constants.get(reference) {
            return Some(value.clone());
        }
        let texts = std::iter::once(self.text).chain(self.project_texts().iter().map(String::as_str));
        texts.into_iter().find_map(|text| declared_string(text, reference))
    }

    /// The project's other source files of the same language, read once.
    fn project_texts(&self) -> &[String] {
        self.project_texts.get_or_init(|| {
            let files = server::project_files(self.source, self.extensions);
            files
                .iter()
                .filter(|file| file.as_path() != self.source)
                .filter_map(|file| fs::read_to_string(file).ok())
                .collect()
        })
    }

    /// The registrations in the project's other source files of the same language.
    fn project(&self) -> &[Registered] {
        self.project.get_or_init(|| {
            let texts = self.project_texts();
            texts.iter().flat_map(|text| self.registrations.registered(text).collect::<Vec<_>>()).collect()
        })
    }
}

/// The string literal assigned to the constant `reference` in `text`: for `Owner.name`, the
/// first `name = '...'` after the declaration of `Owner`; for a bare `name`, the first anywhere.
fn declared_string(text: &str, reference: &str) -> Option<String> {
    let (owner, name) = match reference.rsplit_once('.') {
        Some((owner, name)) => (Some(owner.rsplit('.').next().unwrap_or(owner)), name),
        None => (None, reference),
    };
    let from = match owner {
        Some(owner) => {
            let declaration =
                Regex::new(&format!(r"\b(?:class|interface|enum|object|record|mixin)\s+{}\b", regex::escape(owner)))
                    .ok()?;
            declaration.find(text)?.end()
        }
        None => 0,
    };
    let assignment = Regex::new(&format!(r#"\b{}\s*=\s*r?["']([^"'\n]*)["']"#, regex::escape(name))).ok()?;
    Some(assignment.captures(&text[from..])?[1].to_string())
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
