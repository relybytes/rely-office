// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! `FILTERXML`: the values an `XPath` selects in an XML text. The XML is read with no document
//! type and no entity but the five XML predefines and character references, so it can neither
//! expand nor reach anything outside itself. The `XPath` is the abbreviated subset: absolute and
//! relative paths, `//`, `*`, `@name`, `@*`, `text()`, `node()`, `.`, `..` and positional
//! predicates, `[2]` and `[last()]`.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use super::Evaluator;
use crate::formula::Expr;
use crate::value::{ErrorKind, Value};

type Grid = Vec<Vec<Value>>;

/// The deepest elements nest and the most nodes a text may hold.
const MAX_DEPTH: usize = 256;
const MAX_NODES: usize = 200_000;
/// The longest `XPath` read.
const MAX_PATH_CHARS: usize = 1_024;

#[derive(Debug)]
enum NodeKind {
    Document,
    Element(String),
    Attribute(String, String),
    Text(String),
}

#[derive(Debug)]
struct Node {
    kind: NodeKind,
    parent: Option<usize>,
    children: Vec<usize>,
    attributes: Vec<usize>,
}

/// An XML text as nodes in document order, the document node first.
#[derive(Debug)]
struct Document {
    nodes: Vec<Node>,
}

impl Document {
    fn push(&mut self, kind: NodeKind, parent: Option<usize>) -> Result<usize, ErrorKind> {
        if self.nodes.len() >= MAX_NODES {
            return Err(ErrorKind::Value);
        }
        self.nodes.push(Node {
            kind,
            parent,
            children: Vec::new(),
            attributes: Vec::new(),
        });
        Ok(self.nodes.len() - 1)
    }

    fn push_text(&mut self, parent: usize, text: &str) -> Result<(), ErrorKind> {
        if let Some(&last) = self.nodes[parent].children.last()
            && let NodeKind::Text(previous) = &mut self.nodes[last].kind
        {
            previous.push_str(text);
            return Ok(());
        }
        let node = self.push(NodeKind::Text(text.to_owned()), Some(parent))?;
        self.nodes[parent].children.push(node);
        Ok(())
    }

    fn open(&mut self, start: &BytesStart<'_>, parent: usize) -> Result<usize, ErrorKind> {
        let name = start.name().as_ref().to_owned();
        let element = self.push(NodeKind::Element(name), Some(parent))?;
        self.nodes[parent].children.push(element);
        for attribute in start.attributes() {
            let attribute = attribute.map_err(|_| ErrorKind::Value)?;
            let key = attribute.key.as_ref().to_owned();
            if key == "xmlns" || key.starts_with("xmlns:") {
                continue;
            }
            let value = attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|_| ErrorKind::Value)?
                .into_owned();
            let node = self.push(NodeKind::Attribute(key, value), Some(element))?;
            self.nodes[element].attributes.push(node);
        }
        Ok(element)
    }

    /// The text of a node: an element's is the text of everything inside it.
    fn string_value(&self, node: usize) -> String {
        match &self.nodes[node].kind {
            NodeKind::Attribute(_, value) | NodeKind::Text(value) => value.clone(),
            NodeKind::Document | NodeKind::Element(_) => {
                let mut text = String::new();
                let mut stack = vec![node];
                while let Some(current) = stack.pop() {
                    match &self.nodes[current].kind {
                        NodeKind::Text(value) => text.push_str(value),
                        NodeKind::Document | NodeKind::Element(_) => {
                            stack.extend(self.nodes[current].children.iter().rev());
                        }
                        NodeKind::Attribute(..) => {}
                    }
                }
                text
            }
        }
    }

    /// A node and everything below it, attributes left out, in document order.
    fn descendants_or_self(&self, node: usize, found: &mut Vec<usize>) {
        let mut stack = vec![node];
        while let Some(current) = stack.pop() {
            found.push(current);
            stack.extend(self.nodes[current].children.iter().rev());
        }
    }
}

/// An XML text read into nodes; `#VALUE!` for anything malformed, a document type or an entity
/// other than the predefined ones.
fn parse_document(xml: &str) -> Result<Document, ErrorKind> {
    let mut document = Document { nodes: Vec::new() };
    let root = document.push(NodeKind::Document, None)?;
    let mut reader = Reader::from_str(xml);
    let mut stack = vec![root];
    let mut has_root = false;
    loop {
        let event = reader.read_event().map_err(|_| ErrorKind::Value)?;
        let parent = *stack.last().ok_or(ErrorKind::Value)?;
        match event {
            Event::Start(start) => {
                if stack.len() > MAX_DEPTH || (parent == root && has_root) {
                    return Err(ErrorKind::Value);
                }
                has_root |= parent == root;
                let element = document.open(&start, parent)?;
                stack.push(element);
            }
            Event::Empty(start) => {
                if parent == root && has_root {
                    return Err(ErrorKind::Value);
                }
                has_root |= parent == root;
                document.open(&start, parent)?;
            }
            Event::End(_) => {
                if stack.len() < 2 {
                    return Err(ErrorKind::Value);
                }
                stack.pop();
            }
            Event::Text(text) => {
                let content = text.xml10_content();
                if parent == root {
                    if !content.trim().is_empty() {
                        return Err(ErrorKind::Value);
                    }
                } else {
                    document.push_text(parent, &content)?;
                }
            }
            Event::CData(text) => {
                if parent == root {
                    return Err(ErrorKind::Value);
                }
                let content = text.into_inner().into_owned();
                document.push_text(parent, &content)?;
            }
            Event::GeneralRef(reference) => {
                let resolved = match reference.resolve_char_ref() {
                    Ok(Some(character)) => character.to_string(),
                    Ok(None) => quick_xml::escape::resolve_xml_entity(&reference)
                        .ok_or(ErrorKind::Value)?
                        .to_owned(),
                    Err(_) => return Err(ErrorKind::Value),
                };
                if parent == root {
                    return Err(ErrorKind::Value);
                }
                document.push_text(parent, &resolved)?;
            }
            Event::DocType(_) => return Err(ErrorKind::Value),
            Event::Eof => {
                if stack.len() != 1 || !has_root {
                    return Err(ErrorKind::Value);
                }
                return Ok(document);
            }
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Test {
    /// An element by name, `*` for any.
    Element(Option<String>),
    /// An attribute by name, `@*` for any.
    Attribute(Option<String>),
    Text,
    Node,
    Current,
    Parent,
}

#[derive(Debug, Clone, PartialEq)]
enum Predicate {
    Position(usize),
    Last,
}

#[derive(Debug, Clone, PartialEq)]
struct Step {
    /// Whether the step looks at every node below the context, `//`, rather than its children.
    descendants: bool,
    test: Test,
    predicates: Vec<Predicate>,
}

fn is_name_character(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '_' | '-' | '.' | ':')
}

/// An `XPath` of the abbreviated subset as steps; `#VALUE!` for anything else.
fn parse_path(path: &str) -> Result<Vec<Step>, ErrorKind> {
    let path = path.trim();
    if path.is_empty() || path.chars().count() > MAX_PATH_CHARS {
        return Err(ErrorKind::Value);
    }
    let characters: Vec<char> = path.chars().collect();
    let mut index = 0;
    let mut steps = Vec::new();
    let mut descendants = false;
    if characters.first() == Some(&'/') {
        index = 1;
        if characters.get(1) == Some(&'/') {
            index = 2;
            descendants = true;
        } else if characters.len() == 1 {
            return Err(ErrorKind::Value);
        }
    }
    loop {
        let start = index;
        let test = if characters.get(index) == Some(&'@') {
            index += 1;
            let name_start = index;
            while index < characters.len() && is_name_character(characters[index]) {
                index += 1;
            }
            if characters.get(index) == Some(&'*') && index == name_start {
                index += 1;
                Test::Attribute(None)
            } else if index > name_start {
                Test::Attribute(Some(characters[name_start..index].iter().collect()))
            } else {
                return Err(ErrorKind::Value);
            }
        } else if characters.get(index) == Some(&'*') {
            index += 1;
            Test::Element(None)
        } else {
            while index < characters.len() && is_name_character(characters[index]) {
                index += 1;
            }
            let word: String = characters[start..index].iter().collect();
            if characters.get(index) == Some(&'(') {
                if characters.get(index + 1) != Some(&')') {
                    return Err(ErrorKind::Value);
                }
                index += 2;
                match word.as_str() {
                    "text" => Test::Text,
                    "node" => Test::Node,
                    _ => return Err(ErrorKind::Value),
                }
            } else {
                match word.as_str() {
                    "" => return Err(ErrorKind::Value),
                    "." => Test::Current,
                    ".." => Test::Parent,
                    _ => Test::Element(Some(word)),
                }
            }
        };
        let mut predicates = Vec::new();
        while characters.get(index) == Some(&'[') {
            let close = characters[index..]
                .iter()
                .position(|character| *character == ']')
                .ok_or(ErrorKind::Value)?
                + index;
            let inner: String = characters[index + 1..close].iter().collect();
            let inner = inner.trim();
            predicates.push(if inner == "last()" {
                Predicate::Last
            } else {
                match inner.parse::<usize>() {
                    Ok(position) if position >= 1 => Predicate::Position(position),
                    _ => return Err(ErrorKind::Value),
                }
            });
            index = close + 1;
        }
        steps.push(Step {
            descendants,
            test,
            predicates,
        });
        match characters.get(index) {
            None => return Ok(steps),
            Some('/') => {
                index += 1;
                descendants = characters.get(index) == Some(&'/');
                if descendants {
                    index += 1;
                }
                if index >= characters.len() {
                    return Err(ErrorKind::Value);
                }
            }
            Some(_) => return Err(ErrorKind::Value),
        }
    }
}

/// The nodes a step selects from one context node, in document order.
fn select_from(document: &Document, context: usize, test: &Test) -> Vec<usize> {
    let node = &document.nodes[context];
    match test {
        Test::Current => vec![context],
        Test::Parent => node.parent.into_iter().collect(),
        Test::Attribute(name) => node
            .attributes
            .iter()
            .copied()
            .filter(|attribute| match (&document.nodes[*attribute].kind, name) {
                (NodeKind::Attribute(_, _), None) => true,
                (NodeKind::Attribute(key, _), Some(name)) => key == name,
                _ => false,
            })
            .collect(),
        Test::Element(name) => node
            .children
            .iter()
            .copied()
            .filter(|child| match (&document.nodes[*child].kind, name) {
                (NodeKind::Element(_), None) => true,
                (NodeKind::Element(element), Some(name)) => element == name,
                _ => false,
            })
            .collect(),
        Test::Text => node
            .children
            .iter()
            .copied()
            .filter(|child| matches!(document.nodes[*child].kind, NodeKind::Text(_)))
            .collect(),
        Test::Node => node.children.clone(),
    }
}

/// The nodes an `XPath` selects in a document, in document order, each once.
fn select(document: &Document, steps: &[Step]) -> Vec<usize> {
    let mut context = vec![0];
    for step in steps {
        if step.descendants {
            let mut expanded = Vec::new();
            for node in &context {
                document.descendants_or_self(*node, &mut expanded);
            }
            context = expanded;
        }
        let mut selected = Vec::new();
        for node in &context {
            let mut candidates = select_from(document, *node, &step.test);
            for predicate in &step.predicates {
                candidates = match predicate {
                    Predicate::Position(position) => {
                        candidates.get(position - 1).copied().into_iter().collect()
                    }
                    Predicate::Last => candidates.last().copied().into_iter().collect(),
                };
            }
            selected.extend(candidates);
        }
        selected.sort_unstable();
        selected.dedup();
        context = selected;
    }
    context
}

/// A selected text as a cell shows it: a number when it reads as one.
fn cell_value(text: String) -> Value {
    let trimmed = text.trim();
    if !trimmed.is_empty()
        && trimmed.chars().all(|character| {
            character.is_ascii_digit() || matches!(character, '.' | '-' | '+' | 'e' | 'E')
        })
        && let Ok(number) = trimmed.parse::<f64>()
        && number.is_finite()
    {
        return Value::Number(number);
    }
    Value::Text(text)
}

/// The values an `XPath` selects in an XML text, one per row.
pub(super) fn filter_xml(xml: &str, path: &str) -> Result<Grid, ErrorKind> {
    let steps = parse_path(path)?;
    let document = parse_document(xml)?;
    let selected = select(&document, &steps);
    if selected.is_empty() {
        return Err(ErrorKind::Value);
    }
    Ok(selected
        .into_iter()
        .map(|node| vec![cell_value(document.string_value(node))])
        .collect())
}

impl Evaluator<'_> {
    /// `FILTERXML`, or `None` for any other name.
    pub(super) fn xml_call(
        &self,
        sheet: usize,
        name: &str,
        arguments: &[Expr],
    ) -> Option<Result<Grid, ErrorKind>> {
        if name != "FILTERXML" {
            return None;
        }
        Some((|| {
            if arguments.len() != 2 {
                return Err(ErrorKind::Value);
            }
            let xml = self.text(sheet, arguments.first())?;
            let path = self.text(sheet, arguments.get(1))?;
            filter_xml(&xml, &path)
        })())
    }
}

#[cfg(test)]
mod tests;
