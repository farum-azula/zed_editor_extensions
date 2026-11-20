//! How a language's source is laid out, as far as the engine needs it: lines, classes,
//! decorators, definitions, and the lines that belong to a route.

use super::compile::Language;
use crate::pattern::Matcher;
use std::collections::BTreeSet;

/// How many lines after a route (or handler definition) belong to it.
const WINDOW: usize = 30;

/// Byte offsets where the lines of a text start, to turn an offset into a line.
pub(super) struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    pub fn new(text: &str) -> LineIndex {
        LineIndex { starts: std::iter::once(0).chain(text.match_indices('\n').map(|(at, _)| at + 1)).collect() }
    }

    /// The 0-based line holding byte `offset`.
    pub fn line_of(&self, offset: usize) -> usize {
        self.starts.partition_point(|&start| start <= offset).saturating_sub(1)
    }

    /// The byte offset `line` starts at, or `None` past the last line.
    pub fn start_of(&self, line: usize) -> Option<usize> {
        self.starts.get(line).copied()
    }
}

impl Language {
    /// Does the decorator on `line` belong to a class? It does if the first line after the
    /// decorator block (other decorators, blank lines) declares a class.
    pub(super) fn is_class_level(&self, text: &str, lines: &LineIndex, line: usize) -> bool {
        self.class_after(text, lines, line).is_some()
    }

    /// The line of the class declaration that the decorator on `line` belongs to.
    pub(super) fn class_after(&self, text: &str, lines: &LineIndex, line: usize) -> Option<usize> {
        let class_keywords = self.class_keywords.as_ref()?;
        let from = lines.start_of(line).unwrap_or(text.len());
        let (offset, found) = text[from..]
            .lines()
            .enumerate()
            .skip(1)
            .find(|(_, candidate)| !candidate.trim().is_empty() && !self.starts_decorated(candidate))?;
        class_keywords.is_match(found).then_some(line + offset)
    }

    /// The line of the last class declaration above `line` (the class a route belongs to).
    pub(super) fn enclosing_class(&self, text: &str, line: usize) -> Option<usize> {
        let class_keywords = self.class_keywords.as_ref()?;
        let lines: Vec<&str> = text.lines().collect();
        (0..line.min(lines.len()))
            .rev()
            .find(|&candidate| !self.starts_decorated(lines[candidate]) && class_keywords.is_match(lines[candidate]))
    }

    pub(super) fn starts_decorated(&self, line: &str) -> bool {
        let trimmed = line.trim_start();
        self.decorators.iter().any(|decorator| trimmed.starts_with(decorator.as_str()))
    }

    fn is_definition(&self, line: &str) -> bool {
        self.definitions.as_ref().is_some_and(|definitions| definitions.is_match(line))
    }

    /// The line where the function `handler` is defined.
    pub(super) fn find_definition(&self, lines: &[&str], handler: &str) -> Option<usize> {
        let naming_the_handler: Vec<String> =
            self.definition_patterns.iter().filter(|pattern| pattern.contains("{handler}")).cloned().collect();
        let matcher = Matcher::new(&naming_the_handler, &[("handler", handler)]).ok()??;
        lines.iter().position(|line| matcher.is_match(line))
    }

    /// The handler a route's line refers to (`router.get('/x', listUsers)`), by the language's
    /// handler references.
    pub(super) fn handler_on(&self, line: &str) -> Option<String> {
        self.handler_refs.as_ref()?.first(line)?.get("handler").map(str::to_string)
    }

    /// Lines belonging to the route / handler at `start`: up to 3 lines before it (a signature
    /// above a client call, stacked decorators), then forward until the next route or definition.
    pub(super) fn window(&self, lines: &[&str], start: usize, route_lines: &BTreeSet<usize>) -> String {
        let start = start.min(lines.len());
        let mut from = start;
        while from > 0 && start - from < 3 {
            let previous = lines[from - 1].trim_end();
            let decorated = self.starts_decorated(previous);
            if previous.trim().is_empty()
                || route_lines.contains(&(from - 1))
                || (!decorated && previous.ends_with(['}', ';', ')']))
            {
                break;
            }
            from -= 1;
        }
        // After a decorator (`@app.post`, `#[get]`) the first definition is the handler itself.
        let mut seen_definition = !lines.get(start).is_some_and(|line| self.starts_decorated(line));
        let last = lines.len().min(start + WINDOW);
        let end = (start + 1..last)
            .find(|&index| {
                if route_lines.contains(&index) {
                    return true;
                }
                let definition = self.is_definition(lines[index]);
                let stop = definition && seen_definition;
                seen_definition |= definition;
                stop
            })
            .unwrap_or(last);
        lines[from..end].join("\n")
    }

    /// The declared type of the variable `variable` in `text`.
    pub(super) fn declared_type(&self, text: &str, variable: &str) -> Option<String> {
        let matcher = Matcher::new(&self.variables, &[("var", variable)]).ok()??;
        let found = matcher.hits(text).find_map(|hit| hit.get("type").map(str::to_string));
        found
    }
}
