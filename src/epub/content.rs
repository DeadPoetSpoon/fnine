use crate::error::AppError;
use scraper::{ElementRef, Html, Selector};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone)]
pub struct Run {
    pub text: String,
    pub note_key: String,
}

#[derive(Debug, Clone)]
pub struct Note {
    pub key: String,
    pub label: String,
    pub text: String,
    pub available: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ChapterContent {
    pub runs: Vec<Run>,
    pub notes: Vec<Note>,
}

impl ChapterContent {
    pub fn text(&self) -> String {
        self.runs.iter().map(|run| run.text.as_str()).collect()
    }
    pub fn weight(&self) -> usize {
        self.runs
            .iter()
            .map(|run| run.text.len() + run.note_key.len() + std::mem::size_of::<Run>())
            .sum::<usize>()
            + self
                .notes
                .iter()
                .map(|note| {
                    note.text.len()
                        + note.label.len()
                        + note.key.len()
                        + std::mem::size_of::<Note>()
                })
                .sum::<usize>()
    }
    fn push(&mut self, text: &str) {
        if let Some(last) = self.runs.last_mut().filter(|run| run.note_key.is_empty()) {
            last.text.push_str(text);
        } else {
            self.runs.push(Run {
                text: text.into(),
                note_key: String::new(),
            });
        }
    }
    fn newline(&mut self) {
        if self
            .runs
            .last()
            .is_some_and(|run| !run.text.ends_with('\n'))
        {
            self.push("\n");
        }
    }
}

fn semantic(element: ElementRef<'_>, kinds: &[&str]) -> bool {
    ["epub:type", "role", "class"].iter().any(|attribute| {
        element.value().attr(attribute).is_some_and(|value| {
            value
                .split_ascii_whitespace()
                .any(|token| kinds.contains(&token))
        })
    })
}

fn note_container(element: ElementRef<'_>) -> bool {
    semantic(
        element,
        &[
            "footnote",
            "endnote",
            "doc-footnote",
            "doc-endnote",
            "note",
            "footnote-item",
            "zhusi",
        ],
    )
}

fn superscript(element: ElementRef<'_>) -> bool {
    element.value().name() == "sup"
        || semantic(element, &["math-super", "superscript"])
        || element.value().attr("style").is_some_and(|style| {
            style
                .split(';')
                .filter_map(|rule| rule.split_once(':'))
                .any(|(name, value)| {
                    name.trim().eq_ignore_ascii_case("vertical-align")
                        && value.trim().eq_ignore_ascii_case("super")
                })
        })
}

fn raised(element: ElementRef<'_>) -> bool {
    element
        .descendants()
        .filter_map(ElementRef::wrap)
        .any(superscript)
        || element
            .ancestors()
            .filter_map(ElementRef::wrap)
            .take_while(|parent| !matches!(parent.value().name(), "p" | "div" | "li" | "body"))
            .any(superscript)
}

fn circled(character: char) -> bool {
    matches!(character, '①'..='⑳' | '㉑'..='㉟' | '㊱'..='㊿' | '❶'..='❿')
}

fn circled_number(character: char) -> Option<u32> {
    let (start, number) = match character {
        '①'..='⑳' => ('①', 1),
        '㉑'..='㉟' => ('㉑', 21),
        '㊱'..='㊿' => ('㊱', 36),
        '❶'..='❿' => ('❶', 1),
        _ => return None,
    };
    Some(character as u32 - start as u32 + number)
}

fn short_label(text: &str) -> bool {
    let label = text.trim_matches(['[', ']', '(', ')', '［', '］', '（', '）', '【', '】']);
    !label.is_empty()
        && label.len() <= 12
        && (label.chars().all(|character| character.is_ascii_digit())
            || (label.chars().count() == 1 && label.chars().next().is_some_and(circled)))
}

fn unlinked_marker(element: ElementRef<'_>) -> Option<String> {
    if !superscript(element)
        || element
            .ancestors()
            .filter_map(ElementRef::wrap)
            .any(|parent| {
                parent.value().name() == "a" || parent.value().name() == "li" || superscript(parent)
            })
        || element
            .descendants()
            .filter_map(ElementRef::wrap)
            .any(|child| child.value().name() == "a")
    {
        return None;
    }
    let label = clean(&element.text().collect::<String>());
    (label.chars().count() == 1 && label.chars().next().is_some_and(circled)).then_some(label)
}

struct InlineNote {
    label: String,
    text: Option<String>,
}

fn inline_notes(document: &Html) -> Result<VecDeque<InlineNote>, AppError> {
    let mut notes: Vec<InlineNote> = Vec::new();
    let mut pending: Vec<usize> = Vec::new();
    let mut bytes = 0;
    for element in document
        .root_element()
        .descendants()
        .filter_map(ElementRef::wrap)
    {
        if element
            .ancestors()
            .filter_map(ElementRef::wrap)
            .chain(std::iter::once(element))
            .any(|parent| {
                parent.value().attr("hidden").is_some()
                    || matches!(
                        parent.value().name(),
                        "head" | "script" | "style" | "noscript"
                    )
            })
        {
            continue;
        }
        if let Some(label) = unlinked_marker(element) {
            if notes.len() >= 2048 {
                return Err(AppError::TooLarge);
            }
            if matches!(label.as_str(), "①" | "❶") {
                pending.clear();
            }
            pending.push(notes.len());
            notes.push(InlineNote { label, text: None });
        } else if matches!(element.value().name(), "ul" | "ol") {
            let items = element
                .children()
                .filter_map(ElementRef::wrap)
                .collect::<Vec<_>>();
            let mut bodies = Vec::new();
            if !pending.is_empty() && items.len() == pending.len() {
                for (item, index) in items.iter().zip(&pending) {
                    let mut text = String::new();
                    plain_walk(*item, &mut text, 0, None);
                    let text = clean(&text);
                    let body = text
                        .strip_prefix(&notes[*index].label)
                        .or_else(|| {
                            let marker = text
                                .chars()
                                .next()
                                .filter(|character| circled(*character))?;
                            let number = notes[*index]
                                .label
                                .chars()
                                .next()
                                .and_then(circled_number)?;
                            (item
                                .value()
                                .attr("value")
                                .and_then(|value| value.parse::<u32>().ok())
                                == Some(number))
                            .then(|| &text[marker.len_utf8()..])
                        })
                        .unwrap_or("")
                        .trim();
                    if item.value().name() != "li" || body.is_empty() || body.len() > 64 * 1024 {
                        bodies.clear();
                        break;
                    }
                    bodies.push(body.to_string());
                }
            }
            if bodies.is_empty() && pending.len() == 1 && !items.is_empty() {
                let previous = element
                    .prev_siblings()
                    .filter_map(ElementRef::wrap)
                    .find(|previous| !previous.text().collect::<String>().trim().is_empty());
                let adjacent = previous.is_some_and(|previous| {
                    let labels = previous
                        .descendants()
                        .filter_map(ElementRef::wrap)
                        .filter_map(unlinked_marker)
                        .collect::<Vec<_>>();
                    labels == [notes[pending[0]].label.as_str()]
                });
                let numbered = items.iter().enumerate().all(|(index, item)| {
                    item.value().name() == "li"
                        && item
                            .value()
                            .attr("value")
                            .and_then(|value| value.parse::<usize>().ok())
                            == Some(index + 1)
                        && !item
                            .text()
                            .collect::<String>()
                            .trim()
                            .chars()
                            .next()
                            .is_some_and(circled)
                });
                if adjacent && numbered {
                    let mut text = String::new();
                    plain_walk(element, &mut text, 0, None);
                    let text = clean(&text);
                    if !text.is_empty() && text.len() <= 64 * 1024 {
                        bodies.push(text);
                    }
                }
            }
            if bodies.len() == pending.len() {
                for (index, body) in pending.iter().zip(bodies) {
                    bytes += body.len();
                    if bytes > 4 * 1024 * 1024 {
                        return Err(AppError::TooLarge);
                    }
                    notes[*index].text = Some(body);
                }
            }
            pending.clear();
        } else if matches!(
            element.value().name(),
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
        ) {
            pending.clear();
        }
    }
    Ok(notes.into())
}

pub fn resolve_target(source: &str, href: &str) -> Option<(String, String)> {
    if href.starts_with("//") || href.contains('\\') || href.chars().any(char::is_control) {
        return None;
    }
    let (resource, fragment) = href.split_once('#')?;
    if resource.contains(':') || resource.contains('?') {
        return None;
    }
    let fragment = percent_encoding::percent_decode_str(fragment)
        .decode_utf8()
        .ok()?
        .to_string();
    if fragment.is_empty() {
        return None;
    }
    let decoded = percent_encoding::percent_decode_str(resource)
        .decode_utf8()
        .ok()?;
    if decoded.contains(['\\', ':']) || decoded.starts_with("//") {
        return None;
    }
    let source = percent_encoding::percent_decode_str(source)
        .decode_utf8()
        .ok()?;
    let base = if resource.is_empty() {
        source.to_string()
    } else if decoded.starts_with('/') {
        decoded.to_string()
    } else {
        format!(
            "{}/{}",
            source
                .rsplit_once('/')
                .map(|(parent, _)| parent)
                .unwrap_or(""),
            decoded
        )
    };
    let mut parts = Vec::new();
    for part in base.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            value => parts.push(value),
        }
    }
    Some((format!("/{}", parts.join("/")), fragment))
}

fn plain_walk(
    element: ElementRef<'_>,
    output: &mut String,
    depth: usize,
    origin: Option<(&str, &str, &str)>,
) {
    if depth > 256 {
        return;
    }
    if matches!(
        element.value().name(),
        "script" | "style" | "head" | "noscript"
    ) || element.value().attr("hidden").is_some()
        || semantic(element, &["backlink", "doc-backlink"])
    {
        return;
    }
    if element.value().name() == "a" && element.value().attr("href").is_some() {
        let label = element.text().collect::<String>();
        let backlink = element
            .value()
            .attr("href")
            .and_then(|href| href.split_once('#'))
            .is_some_and(|(_, fragment)| {
                ["ref", "fnref", "noteref"]
                    .iter()
                    .any(|prefix| fragment.starts_with(prefix))
            });
        let reciprocal = origin.is_some_and(|(source, path, fragment)| {
            element
                .value()
                .attr("href")
                .and_then(|href| resolve_target(source, href))
                .is_some_and(|target| target.0 == path && target.1 == fragment)
        });
        if backlink || reciprocal || matches!(label.trim(), "↩" | "↵" | "↑" | "back" | "return")
        {
            return;
        }
    }
    let block = matches!(
        element.value().name(),
        "p" | "div" | "li" | "aside" | "section" | "blockquote" | "br" | "h1" | "h2" | "h3" | "tr"
    );
    if block && !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
    for child in element.children() {
        if let Some(text) = child.value().as_text() {
            output.push_str(text);
        } else if let Some(child) = ElementRef::wrap(child) {
            plain_walk(child, output, depth + 1, origin);
        }
    }
    if block && !output.ends_with('\n') {
        output.push('\n');
    }
}

fn clean(text: &str) -> String {
    text.lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn target_text(
    document: &Html,
    fragment: &str,
    explicit: bool,
    source: &str,
    origin: Option<(&str, &str)>,
) -> Option<String> {
    let target = document
        .select(&Selector::parse("[id], a[name]").ok()?)
        .find(|element| {
            element.value().attr("id") == Some(fragment)
                || element.value().attr("name") == Some(fragment)
        })?;
    if !explicit && raised(target) {
        return None;
    }
    let container = target
        .ancestors()
        .filter_map(ElementRef::wrap)
        .find(|element| note_container(*element))
        .or_else(|| note_container(target).then_some(target))
        .or_else(|| {
            target
                .ancestors()
                .filter_map(ElementRef::wrap)
                .find(|element| element.value().name() == "li")
        })
        .or_else(|| {
            if matches!(target.value().name(), "a" | "span" | "sup") {
                target
                    .ancestors()
                    .filter_map(ElementRef::wrap)
                    .find(|element| matches!(element.value().name(), "p" | "div" | "aside"))
            } else {
                None
            }
        })
        .unwrap_or(target);
    let reciprocal = origin.is_some_and(|(path, fragment)| {
        container
            .descendants()
            .filter_map(ElementRef::wrap)
            .any(|element| {
                element
                    .value()
                    .attr("href")
                    .and_then(|href| resolve_target(source, href))
                    .is_some_and(|target| target.0 == path && target.1 == fragment)
            })
    });
    let looks_like_note = reciprocal
        || note_container(container)
        || ["note", "fn", "endnote", "footnote"]
            .iter()
            .any(|prefix| fragment.to_ascii_lowercase().starts_with(prefix));
    if !explicit && !looks_like_note {
        return None;
    }
    let mut text = String::new();
    let backlink = origin.map(|(path, fragment)| (source, path, fragment));
    plain_walk(container, &mut text, 0, backlink);
    if clean(&text).is_empty() && container.value().name() == "a" {
        if let Some(parent) = container
            .parent()
            .and_then(ElementRef::wrap)
            .filter(|element| matches!(element.value().name(), "p" | "li" | "div"))
        {
            plain_walk(parent, &mut text, 0, backlink);
        } else if let Some(next) = container.next_siblings().find_map(ElementRef::wrap) {
            plain_walk(next, &mut text, 0, backlink);
        }
    }
    let text = clean(&text);
    (!text.is_empty() && !short_label(&text) && text.len() <= 64 * 1024).then_some(text)
}

struct Extractor<'ebook> {
    epub: &'ebook rbook::Epub,
    source: &'ebook str,
    documents: HashMap<String, Html>,
    targets: HashMap<String, usize>,
    result: ChapterContent,
    resource_bytes: usize,
    note_bytes: usize,
    inline_notes: VecDeque<InlineNote>,
}

impl Extractor<'_> {
    fn add_note(&mut self, label: &str, text: Option<String>) -> Result<usize, AppError> {
        if self.result.notes.len() >= 2048 {
            return Err(AppError::TooLarge);
        }
        let index = self.result.notes.len();
        let available = text.is_some();
        let text = text.unwrap_or_default();
        self.note_bytes += text.len();
        if self.note_bytes > 4 * 1024 * 1024 {
            return Err(AppError::TooLarge);
        }
        self.result.notes.push(Note {
            key: format!("note-{index}"),
            label: label.to_string(),
            text,
            available,
        });
        Ok(index)
    }

    fn reference(&mut self, element: ElementRef<'_>) -> Result<bool, AppError> {
        let explicit = semantic(element, &["noteref", "doc-noteref"]);
        let label = clean(&element.text().collect::<String>());
        let note_backlink = !explicit
            && element
                .ancestors()
                .filter_map(ElementRef::wrap)
                .any(note_container);
        let leading_bracketed_marker = !explicit
            && !raised(element)
            && label.starts_with(['[', '［', '【'])
            && element
                .ancestors()
                .filter_map(ElementRef::wrap)
                .find(|parent| matches!(parent.value().name(), "p" | "li"))
                .is_some_and(|parent| {
                    clean(&parent.text().collect::<String>())
                        .strip_prefix(&label)
                        .is_some_and(|body| !body.trim().is_empty())
                });
        if semantic(element, &["backlink", "doc-backlink"])
            || note_backlink
            || leading_bracketed_marker
            || (!explicit && !raised(element) && !short_label(&label))
        {
            return Ok(false);
        }
        let Some(href) = element.value().attr("href") else {
            return Ok(false);
        };
        let origin_id = element.value().attr("id").or_else(|| {
            element
                .parent()
                .and_then(ElementRef::wrap)
                .and_then(|parent| parent.value().attr("id"))
        });
        let origin = origin_id.and_then(|id| {
            resolve_target(self.source, "#origin").map(|(path, _)| (path, id.to_string()))
        });
        let target = resolve_target(self.source, href);
        let canonical = target
            .as_ref()
            .map(|(path, fragment)| format!("{path}#{fragment}"));
        let existing = canonical
            .as_ref()
            .and_then(|key| self.targets.get(key))
            .copied();
        let note_index = if let Some(index) = existing {
            index
        } else {
            let text = if let Some((path, fragment)) = &target {
                if !self.documents.contains_key(path) {
                    let resource = self.epub.manifest().iter().find(|entry| {
                        percent_encoding::percent_decode_str(entry.href().path().as_str())
                            .decode_utf8()
                            .is_ok_and(|candidate| candidate == path.as_str())
                    });
                    if let Some(resource) = resource {
                        let html = resource
                            .read_str()
                            .map_err(|error| AppError::BadRequest(error.to_string()))?;
                        self.resource_bytes += html.len();
                        if self.resource_bytes > 256 * 1024 * 1024 {
                            return Err(AppError::TooLarge);
                        }
                        self.documents
                            .insert(path.clone(), Html::parse_document(&html));
                    }
                }
                self.documents.get(path).and_then(|document| {
                    target_text(
                        document,
                        fragment,
                        explicit,
                        path,
                        origin
                            .as_ref()
                            .map(|(path, fragment)| (path.as_str(), fragment.as_str())),
                    )
                })
            } else {
                None
            };
            if !explicit && text.is_none() {
                return Ok(false);
            }
            let index = self.add_note(&label, text)?;
            if let Some(canonical) = canonical {
                self.targets.insert(canonical, index);
            }
            index
        };
        self.result.runs.push(Run {
            text: if label.is_empty() { "*".into() } else { label },
            note_key: self.result.notes[note_index].key.clone(),
        });
        Ok(true)
    }

    fn walk(&mut self, element: ElementRef<'_>, depth: usize) -> Result<(), AppError> {
        if depth > 256 {
            return Err(AppError::TooLarge);
        }
        let name = element.value().name();
        if matches!(name, "head" | "script" | "style" | "noscript")
            || element.value().attr("hidden").is_some()
        {
            return Ok(());
        }
        if name == "a" && self.reference(element)? {
            return Ok(());
        }
        if unlinked_marker(element).is_some()
            && let Some(note) = self.inline_notes.pop_front()
        {
            let index = self.add_note(&note.label, note.text)?;
            self.result.runs.push(Run {
                text: note.label,
                note_key: self.result.notes[index].key.clone(),
            });
            return Ok(());
        }
        let block = matches!(
            name,
            "p" | "div"
                | "section"
                | "aside"
                | "li"
                | "h1"
                | "h2"
                | "h3"
                | "h4"
                | "blockquote"
                | "br"
                | "tr"
        );
        if block {
            self.result.newline();
        }
        for child in element.children() {
            if let Some(text) = child.value().as_text() {
                let text = text
                    .split_inclusive(char::is_whitespace)
                    .map(|piece| {
                        let trimmed = piece.trim_end_matches(char::is_whitespace);
                        if trimmed.len() < piece.len() {
                            format!("{trimmed} ")
                        } else {
                            piece.to_string()
                        }
                    })
                    .collect::<String>();
                if !text.trim().is_empty() || !self.result.runs.is_empty() {
                    self.result.push(&text);
                }
            } else if let Some(child) = ElementRef::wrap(child) {
                self.walk(child, depth + 1)?;
            }
        }
        if block {
            self.result.newline();
        }
        Ok(())
    }
}

pub fn extract(epub: &rbook::Epub, source: &str, html: &str) -> Result<ChapterContent, AppError> {
    let document = Html::parse_document(html);
    let mut extractor = Extractor {
        epub,
        source,
        documents: HashMap::new(),
        targets: HashMap::new(),
        result: ChapterContent::default(),
        resource_bytes: 0,
        note_bytes: 0,
        inline_notes: inline_notes(&document)?,
    };
    extractor.walk(document.root_element(), 0)?;
    Ok(extractor.result)
}
