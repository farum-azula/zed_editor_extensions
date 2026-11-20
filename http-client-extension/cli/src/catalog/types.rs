//! Source types to JSON shapes: the type names of `frameworks.json`, wrapper types, and the
//! fields of the models (classes, structs, records…) declared in a file.

use super::compile::{Language, Models};
use super::model::{Block, TypesDefinition};
use crate::infer::{Field, FieldType};
use crate::pattern::Matcher;
use regex::Regex;
use std::sync::LazyLock;

/// How deep models nested in models are followed.
const MAX_MODEL_DEPTH: usize = 3;

/// Lines of a model body that start like this declare something else than a field.
const NOT_A_FIELD: [&str; 7] = ["def ", "fn ", "func ", "function ", "return", "class ", "static "];

/// An annotation or attribute in front of a field: `@JsonProperty("x")`, `[Required]`.
static ANNOTATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:@[\w.]+(?:\([^)]*\))?|\[[^\]]*\])\s*").expect("the annotation pattern is a valid regex")
});

/// The type names of one JSON shape.
struct TypeClass {
    names: Vec<String>,
    kind: FieldType,
}

/// The compiled `types` section: type names per JSON shape, and wrapper types.
pub(super) struct TypeTable {
    /// Tried in order; the first class naming a type decides its shape.
    classes: Vec<TypeClass>,
    wrappers: Vec<String>,
}

impl TypeTable {
    pub fn new(definition: TypesDefinition) -> TypeTable {
        let class = |names: Vec<String>, kind: FieldType| TypeClass { names, kind };
        TypeTable {
            classes: vec![
                class(definition.list, FieldType::List),
                class(definition.int, FieldType::Integer),
                class(definition.float, FieldType::Float),
                class(definition.bool, FieldType::Boolean),
                class(definition.object, FieldType::Object(vec![])),
                class(definition.string, FieldType::String),
            ],
            wrappers: definition.wrappers,
        }
    }

    /// Strips wrapper types (`Optional<T>`, `Json<T>`…) down to the type they carry.
    pub fn unwrap<'a>(&self, type_name: &'a str) -> &'a str {
        let mut type_name = type_name.trim().trim_start_matches(['&', '*']).trim_end_matches('?');
        type_name = type_name.strip_suffix(" | None").unwrap_or(type_name).trim();
        loop {
            let base = base_name(type_name);
            let is_wrapper = self.wrappers.iter().any(|wrapper| wrapper.eq_ignore_ascii_case(base));
            match (is_wrapper, type_name.find(['<', '[']), type_name.rfind(['>', ']'])) {
                (true, Some(open), Some(close)) if open < close => {
                    type_name = type_name[open + 1..close].split(',').next().unwrap_or("").trim();
                }
                _ => return type_name,
            }
        }
    }

    /// The shape of a type named in the catalog (by its short name, any case).
    fn known(&self, short_name: &str) -> Option<FieldType> {
        let named = |class: &&TypeClass| class.names.iter().any(|name| name.eq_ignore_ascii_case(short_name));
        self.classes.iter().find(named).map(|class| class.kind.clone())
    }
}

/// Resolves the types named in one file of one language.
pub(super) struct TypeResolver<'a> {
    pub table: &'a TypeTable,
    pub language: &'a Language,
    pub text: &'a str,
}

impl TypeResolver<'_> {
    /// The JSON shape of the source type `type_name`; models declared in the file are followed
    /// `MAX_MODEL_DEPTH` levels deep.
    pub fn field_type(&self, type_name: &str, depth: usize) -> FieldType {
        let type_name = self.table.unwrap(type_name);
        if type_name.starts_with("[]") || type_name.ends_with("[]") {
            return FieldType::List;
        }
        let base = base_name(type_name);
        if let Some(kind) = self.table.known(&short_name(base).to_lowercase()) {
            return kind;
        }
        if depth < MAX_MODEL_DEPTH && starts_uppercase(base) {
            let fields = self.model_fields(base, depth);
            if !fields.is_empty() {
                return FieldType::Object(fields);
            }
        }
        FieldType::String
    }

    /// Fields of the model `name` declared in the file.
    pub fn model_fields(&self, name: &str, depth: usize) -> Vec<Field> {
        let name = short_name(base_name(self.table.unwrap(name)));
        let Some(models) = &self.language.models else { return vec![] };
        if !starts_uppercase(name) {
            return vec![];
        }
        let Some(rest) = self.declaration_of(models, name) else { return vec![] };
        let Some(block) = FIELD_BLOCKS.iter().find(|block| block.follows(models.block, rest)) else { return vec![] };
        let Some(body) = block.read(rest) else { return vec![] };
        let mut fields: Vec<Field> = Vec::new();
        for raw in body.lines() {
            let Some(line) = field_line(raw, models, block.allows_parentheses()) else { continue };
            let Some(hit) = models.fields.first(line).filter(|hit| hit.start == 0) else { continue };
            let (Some(field_name), Some(type_name)) = (hit.get("name"), hit.get("type")) else { continue };
            if !fields.iter().any(|field| field.name == field_name) {
                fields.push(Field::new(field_name, self.field_type(type_name, depth + 1)));
            }
        }
        fields
    }

    /// The text after the declaration of the model `name` (`class Name`), not of a longer name.
    fn declaration_of(&self, models: &Models, name: &str) -> Option<&str> {
        let declaration = Matcher::new(&models.declarations, &[("type", name)]).ok()??;
        let text = self.text;
        let hit = declaration
            .hits(text)
            .find(|hit| text[hit.end..].chars().next().is_none_or(|next| !next.is_alphanumeric() && next != '_'))?;
        Some(&text[hit.end..])
    }
}

/// One way a model's fields are written down after its name.
trait FieldBlock {
    /// Whether this is how the model of a language with `block` bodies, followed by `rest`, is written.
    fn follows(&self, block: Block, rest: &str) -> bool;
    /// The fields, one per line; `None` when the declaration has none here.
    fn read(&self, rest: &str) -> Option<String>;
    /// May a field line hold parentheses? In a parameter list it may; in a body, a line with
    /// parentheses is a method.
    fn allows_parentheses(&self) -> bool {
        false
    }
}

/// `record User(String name, int age)`, `data class User(val name: String)`.
struct ParameterList;
/// Python: the indented lines below `class User(BaseModel):`.
struct IndentedBody;
/// `class User { ... }`, `struct User { ... }`.
struct BracedBody;

/// Tried in order; the braced body is the fallback.
const FIELD_BLOCKS: [&dyn FieldBlock; 3] = [&ParameterList, &IndentedBody, &BracedBody];

impl FieldBlock for ParameterList {
    fn follows(&self, block: Block, rest: &str) -> bool {
        block == Block::Braces
            && rest.trim_start_matches([' ', '\t']).starts_with(['(', '<'])
            && rest.trim_start().trim_start_matches(|c| c != '(' && c != '{').starts_with('(')
    }

    fn read(&self, rest: &str) -> Option<String> {
        Some(enclosed(rest, '(', ')').unwrap_or_default().replace(',', "\n"))
    }

    fn allows_parentheses(&self) -> bool {
        true
    }
}

impl FieldBlock for IndentedBody {
    fn follows(&self, block: Block, _rest: &str) -> bool {
        block == Block::Indent
    }

    fn read(&self, rest: &str) -> Option<String> {
        let indented = rest.lines().skip(1).take_while(|line| line.trim().is_empty() || line.starts_with([' ', '\t']));
        Some(indented.collect::<Vec<_>>().join("\n"))
    }
}

impl FieldBlock for BracedBody {
    fn follows(&self, _block: Block, _rest: &str) -> bool {
        true
    }

    fn read(&self, rest: &str) -> Option<String> {
        let open = rest.find('{')?;
        // A `;` before the body ends a forward declaration.
        if rest[..open].contains(';') {
            return None;
        }
        Some(top_level(&enclosed(&rest[open..], '{', '}').unwrap_or_default()))
    }
}

/// The field declaration on a line of a model body, without annotations and modifiers; `None`
/// for a line that declares nothing or something else.
fn field_line<'a>(raw: &'a str, models: &Models, allows_parentheses: bool) -> Option<&'a str> {
    let mut line = raw.trim();
    while let Some(annotation) = ANNOTATION.find(line) {
        line = line[annotation.end()..].trim_start();
    }
    while let Some(rest) = strip_modifier(line, &models.modifiers) {
        line = rest;
    }
    let declares_something_else = line.is_empty()
        || line.starts_with(['#', '/', '*', '@'])
        || NOT_A_FIELD.iter().any(|keyword| line.starts_with(keyword))
        || (!allows_parentheses && line.contains('('));
    (!declares_something_else).then_some(line)
}

/// `line` without the first of `modifiers` it starts with (`pub`, `final`, `readonly`).
fn strip_modifier<'a>(line: &'a str, modifiers: &[String]) -> Option<&'a str> {
    modifiers.iter().find_map(|modifier| {
        let rest = line.strip_prefix(modifier.as_str())?;
        rest.starts_with(char::is_whitespace).then(|| rest.trim_start())
    })
}

/// `List<Item>` → `List`, `list[str]` → `list`.
fn base_name(type_name: &str) -> &str {
    type_name.split(['<', '[']).next().unwrap_or(type_name)
}

/// `java.util.List` → `List`, `models::User` → `User`.
fn short_name(type_name: &str) -> &str {
    type_name.rsplit(['.', ':']).next().unwrap_or(type_name)
}

fn starts_uppercase(name: &str) -> bool {
    name.chars().next().is_some_and(char::is_uppercase)
}

/// The text between the first `open` and its matching `close`.
fn enclosed(text: &str, open: char, close: char) -> Option<String> {
    let start = text.find(open)?;
    let mut depth = 0;
    for (index, c) in text[start..].char_indices() {
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Some(text[start + 1..start + index].to_string());
            }
        }
    }
    None
}

/// Top-level statements of a block body (nested bodies removed), one per line.
fn top_level(body: &str) -> String {
    let mut out = String::new();
    let mut depth = 0i32;
    for c in body.chars() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
        if c == '\n' && depth > 0 {
            out.push('\n');
        }
    }
    out.replace(';', ";\n")
}
