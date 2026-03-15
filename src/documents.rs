use crate::fs::{atomic_write, ensure_parent_dirs};
use crate::math::{MathStyle, html_escape, render_math};
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

pub(crate) fn collect_markdown_paths(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    collect_markdown_paths_recursive(root, &mut paths)?;
    Ok(paths)
}

fn collect_markdown_paths_recursive(dir: &Path, paths: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }

        let path = entry.path();
        if file_type.is_dir() {
            let is_hidden = entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with('.'));
            if !is_hidden {
                collect_markdown_paths_recursive(&path, paths)?;
            }
            continue;
        }

        if file_type.is_file()
            && matches!(path.extension().and_then(|ext| ext.to_str()), Some("md"))
        {
            paths.push(path);
        }
    }
    Ok(())
}

pub(crate) fn doc_id_from_path(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in rel.components() {
        match component {
            Component::Normal(os_str) => {
                parts.push(os_str.to_string_lossy().into_owned());
            }
            _ => return None,
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

pub(crate) fn load_document(root: &Path, doc_id: &str) -> Result<String, DocError> {
    let path = resolve_doc_path(root, doc_id)?;
    std::fs::read_to_string(&path).map_err(|err| match err.kind() {
        ErrorKind::NotFound | ErrorKind::IsADirectory => DocError::NotFound,
        _ => DocError::Io(err),
    })
}

pub(crate) fn resolve_doc_path(root: &Path, doc_id: &str) -> Result<PathBuf, DocError> {
    let doc_path = doc_id_to_path(doc_id).ok_or(DocError::BadPath)?;
    let joined = root.join(doc_path);
    let resolved = match std::fs::canonicalize(&joined) {
        Ok(path) => path,
        Err(err) if err.kind() == ErrorKind::NotFound => return Err(DocError::NotFound),
        Err(err) => return Err(DocError::Io(err)),
    };
    if !resolved.starts_with(root) {
        return Err(DocError::NotFound);
    }
    Ok(resolved)
}

pub(crate) fn create_document(root: &Path, doc_id: &str, contents: &str) -> Result<(), DocError> {
    let doc_path = doc_id_to_path(doc_id).ok_or(DocError::BadPath)?;
    ensure_parent_dirs(root, &doc_path).map_err(map_io_to_doc_error)?;
    let target = root.join(&doc_path);

    match std::fs::symlink_metadata(&target) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(DocError::BadPath);
            }
            return Err(DocError::Io(std::io::Error::new(
                ErrorKind::AlreadyExists,
                "document already exists",
            )));
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => return Err(DocError::Io(err)),
    }

    atomic_write(&target, contents).map_err(DocError::Io)
}

pub(crate) struct SearchResult {
    pub(crate) doc_id: String,
    pub(crate) snippet: String,
}

pub(crate) fn search_documents(root: &Path, query: &str) -> std::io::Result<Vec<SearchResult>> {
    let mut results = Vec::new();
    let needle = query.to_lowercase();
    for path in collect_markdown_paths(root)? {
        let doc_id = match doc_id_from_path(root, &path) {
            Some(doc_id) => doc_id,
            None => continue,
        };
        let contents = std::fs::read_to_string(&path)?;
        if let Some(snippet) = find_match_snippet(&contents, &needle) {
            results.push(SearchResult { doc_id, snippet });
        }
    }
    results.sort_by(|a, b| a.doc_id.cmp(&b.doc_id));
    Ok(results)
}

pub(crate) struct RenderedDocument {
    pub(crate) html: String,
    pub(crate) has_mermaid: bool,
    pub(crate) has_abc: bool,
}

pub(crate) fn render_document_html(markdown: &str, doc_id: &str) -> RenderedDocument {
    let rendered = render_task_list_markdown(markdown, doc_id);
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_MATH);
    let parser = Parser::new_ext(&rendered, options).map(|event| {
        let event = rewrite_relative_md_links(event, doc_id);
        rewrite_relative_image_links(event, doc_id)
    });

    let mut has_mermaid = false;
    let mut has_abc = false;
    let mut in_mermaid = false;
    let mut in_abc = false;
    let mut mermaid_buffer = String::new();
    let mut abc_buffer = String::new();
    let mut events = Vec::new();

    for event in parser {
        if in_mermaid {
            match event {
                Event::End(TagEnd::CodeBlock) => {
                    let escaped = html_escape(&mermaid_buffer);
                    let html = format!(r#"<div class="mermaid">{escaped}</div>"#);
                    events.push(Event::Html(html.into()));
                    mermaid_buffer.clear();
                    in_mermaid = false;
                    has_mermaid = true;
                }
                Event::Text(text) => mermaid_buffer.push_str(&text),
                Event::SoftBreak | Event::HardBreak => mermaid_buffer.push('\n'),
                _ => {}
            }
            continue;
        }

        if in_abc {
            match event {
                Event::End(TagEnd::CodeBlock) => {
                    let escaped = html_escape(&abc_buffer);
                    let html = format!(r#"<div class="abc-notation">{escaped}</div>"#);
                    events.push(Event::Html(html.into()));
                    abc_buffer.clear();
                    in_abc = false;
                    has_abc = true;
                }
                Event::Text(text) => abc_buffer.push_str(&text),
                Event::SoftBreak | Event::HardBreak => abc_buffer.push('\n'),
                _ => {}
            }
            continue;
        }

        if let Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(ref info))) = event {
            if is_mermaid_info(info) {
                in_mermaid = true;
                mermaid_buffer.clear();
                continue;
            }
            if is_abc_info(info) {
                in_abc = true;
                abc_buffer.clear();
                continue;
            }
        }

        let event = match event {
            Event::InlineMath(latex) => {
                let html = render_math(&latex, MathStyle::Inline).into_html();
                Event::Html(html.into())
            }
            Event::DisplayMath(latex) => {
                let html = render_math(&latex, MathStyle::Display).into_html();
                Event::Html(html.into())
            }
            other => other,
        };
        events.push(event);
    }

    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, events.into_iter());

    RenderedDocument {
        html,
        has_mermaid,
        has_abc,
    }
}

pub(crate) fn render_task_list_markdown(contents: &str, doc_id: &str) -> String {
    let mut output = String::with_capacity(contents.len());
    let mut in_fence = false;
    let mut task_index = 0usize;
    let mut in_task_list = false;
    let mut list_index = 0usize;

    for segment in contents.split_inclusive('\n') {
        let (line, ending) = split_line_ending(segment);
        if is_fence_line(line) {
            if in_task_list {
                list_index += 1;
                in_task_list = false;
            }
            in_fence = !in_fence;
            output.push_str(line);
            output.push_str(ending);
            continue;
        }

        if !in_fence && let Some(parts) = parse_task_line(line) {
            let checked = if parts.checked { " checked" } else { "" };
            let input = format!(
                "<input type=\"checkbox\" class=\"todo-checkbox\" data-task-index=\"{}\"{} />",
                task_index, checked
            );
            output.push_str(parts.prefix);
            output.push_str(&input);
            output.push_str(parts.suffix);
            output.push_str(ending);
            task_index += 1;
            in_task_list = true;
            continue;
        }

        if in_task_list && !in_fence {
            if is_task_list_marker(line) {
                output.push_str(&render_task_list_form(doc_id, list_index));
                list_index += 1;
                in_task_list = false;
                continue;
            }
            list_index += 1;
            in_task_list = false;
        }

        output.push_str(line);
        output.push_str(ending);
    }

    output
}

fn find_match_snippet(contents: &str, needle: &str) -> Option<String> {
    for line in contents.lines() {
        if line.to_lowercase().contains(needle) {
            return Some(line.trim().to_string());
        }
    }
    None
}

fn is_mermaid_info(info: &str) -> bool {
    let language = info.split_whitespace().next().unwrap_or("");
    language.eq_ignore_ascii_case("mermaid")
}

fn is_abc_info(info: &str) -> bool {
    let language = info.split_whitespace().next().unwrap_or("");
    language.eq_ignore_ascii_case("abc") || language.eq_ignore_ascii_case("abcjs")
}

pub(crate) fn collect_mentions(contents: &str) -> Vec<(String, String)> {
    let mut mentions = Vec::new();
    let mut in_fence = false;

    for line in contents.lines() {
        if is_fence_line(line) {
            in_fence = !in_fence;
            continue;
        }

        if in_fence {
            continue;
        }

        let mut seen = HashSet::new();
        for user in extract_mentions_from_line(line) {
            if seen.insert(user.clone()) {
                mentions.push((user, line.to_string()));
            }
        }
    }

    mentions
}

pub(crate) fn toggle_task_item(contents: &str, task_index: usize, checked: bool) -> Option<String> {
    let mut output = String::with_capacity(contents.len());
    let mut in_fence = false;
    let mut current = 0usize;
    let mut updated = false;

    for segment in contents.split_inclusive('\n') {
        let (line, ending) = split_line_ending(segment);
        if is_fence_line(line) {
            in_fence = !in_fence;
            output.push_str(line);
            output.push_str(ending);
            continue;
        }

        if !in_fence && let Some(parts) = parse_task_line(line) {
            if current == task_index {
                let mark = if checked { 'x' } else { ' ' };
                output.push_str(parts.prefix);
                output.push('[');
                output.push(mark);
                output.push(']');
                output.push_str(parts.suffix);
                output.push_str(ending);
                updated = true;
            } else {
                output.push_str(line);
                output.push_str(ending);
            }
            current += 1;
            continue;
        }

        output.push_str(line);
        output.push_str(ending);
    }

    if updated { Some(output) } else { None }
}

pub(crate) fn add_task_item_in_list(contents: &str, list_index: usize, text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return contents.to_string();
    }

    let new_line = format!("- [ ] {}", trimmed);
    let mut in_fence = false;
    let mut last_task_end: Option<usize> = None;
    let mut last_task_ending = "";
    let mut offset = 0usize;
    let mut current_list = 0usize;
    let mut in_task_list = false;
    let mut found_list = false;

    for segment in contents.split_inclusive('\n') {
        let (line, ending) = split_line_ending(segment);
        if is_fence_line(line) {
            if in_task_list {
                if current_list == list_index {
                    found_list = true;
                    break;
                }
                current_list += 1;
                in_task_list = false;
                last_task_end = None;
                last_task_ending = "";
            }
            in_fence = !in_fence;
        }

        if !in_fence && parse_task_line(line).is_some() {
            if !in_task_list {
                in_task_list = true;
            }
            last_task_end = Some(offset + line.len() + ending.len());
            last_task_ending = ending;
        } else if in_task_list {
            if current_list == list_index {
                found_list = true;
                break;
            }
            current_list += 1;
            in_task_list = false;
            last_task_end = None;
            last_task_ending = "";
        }

        offset += segment.len();
    }

    if in_task_list && current_list == list_index {
        found_list = true;
    }

    if list_index > 0 && !found_list {
        return contents.to_string();
    }

    if let Some(insert_at) = last_task_end {
        let mut output = String::with_capacity(contents.len() + new_line.len() + 2);
        let (prefix, suffix) = contents.split_at(insert_at);
        output.push_str(prefix);
        if last_task_ending.is_empty() {
            output.push_str(detect_line_ending(contents));
            output.push_str(&new_line);
        } else {
            output.push_str(&new_line);
            output.push_str(last_task_ending);
        }
        output.push_str(suffix);
        return output;
    }

    let mut output = String::with_capacity(contents.len() + new_line.len() + 2);
    output.push_str(contents);
    if !contents.is_empty() && !contents.ends_with('\n') && !contents.ends_with('\r') {
        output.push_str(detect_line_ending(contents));
    }
    output.push_str(&new_line);
    output
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlockKind {
    Fence,
    Table,
    ListItem,
    Heading,
    Paragraph,
    Blank,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BlockRange {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) kind: BlockKind,
}

#[derive(Debug)]
pub(crate) enum ReorderError {
    InvalidRange,
}

pub(crate) fn line_count(contents: &str) -> usize {
    split_lines_preserve(contents).len()
}

pub(crate) fn lines_for_display(contents: &str) -> Vec<String> {
    split_lines_preserve(contents)
        .into_iter()
        .map(|segment| segment.text)
        .collect()
}

pub(crate) fn scan_block_ranges(contents: &str) -> Vec<BlockRange> {
    let lines = split_lines_preserve(contents);
    let mut blocks = Vec::new();
    let mut index = 0usize;

    while index < lines.len() {
        let line = lines[index].text.as_str();

        if is_fence_line(line) {
            let start = index;
            index += 1;
            while index < lines.len() && !is_fence_line(lines[index].text.as_str()) {
                index += 1;
            }
            if index < lines.len() {
                index += 1;
            }
            blocks.push(BlockRange {
                start,
                end: index.saturating_sub(1),
                kind: BlockKind::Fence,
            });
            continue;
        }

        if let Some(end) = detect_table_block(&lines, index) {
            blocks.push(BlockRange {
                start: index,
                end,
                kind: BlockKind::Table,
            });
            index = end + 1;
            continue;
        }

        if let Some(end) = detect_list_item_block(&lines, index) {
            blocks.push(BlockRange {
                start: index,
                end,
                kind: BlockKind::ListItem,
            });
            index = end + 1;
            continue;
        }

        if is_heading_line(line) {
            blocks.push(BlockRange {
                start: index,
                end: index,
                kind: BlockKind::Heading,
            });
            index += 1;
            continue;
        }

        if line.trim().is_empty() {
            blocks.push(BlockRange {
                start: index,
                end: index,
                kind: BlockKind::Blank,
            });
            index += 1;
            continue;
        }

        let start = index;
        index += 1;
        while index < lines.len() {
            let line = lines[index].text.as_str();
            if line.trim().is_empty() {
                break;
            }
            if is_fence_line(line) {
                break;
            }
            if is_heading_line(line) {
                break;
            }
            if is_list_item_start(line) {
                break;
            }
            if is_table_start(&lines, index) {
                break;
            }
            index += 1;
        }
        blocks.push(BlockRange {
            start,
            end: index.saturating_sub(1),
            kind: BlockKind::Paragraph,
        });
    }

    blocks
}

pub(crate) fn reorder_range(
    contents: &str,
    start_line: usize,
    end_line: usize,
    insert_before_line: usize,
) -> Result<Option<String>, ReorderError> {
    let mut segments = split_lines_preserve(contents);
    let line_total = segments.len();
    if start_line > end_line || end_line >= line_total || insert_before_line > line_total {
        return Err(ReorderError::InvalidRange);
    }

    let range_len = end_line - start_line + 1;
    if insert_before_line >= start_line && insert_before_line <= end_line + 1 {
        return Ok(None);
    }

    let moved: Vec<LineSegment> = segments.drain(start_line..=end_line).collect();
    let mut target = insert_before_line;
    if insert_before_line > end_line {
        target = insert_before_line - range_len;
    }
    segments.splice(target..target, moved);

    Ok(Some(join_lines_preserve(&segments, contents)))
}

#[derive(Clone)]
struct LineSegment {
    text: String,
    ending: String,
}

fn split_lines_preserve(contents: &str) -> Vec<LineSegment> {
    if contents.is_empty() {
        return Vec::new();
    }
    let mut segments = Vec::new();
    if contents.contains('\n') {
        for segment in contents.split_inclusive('\n') {
            let (text, ending) = split_line_ending(segment);
            segments.push(LineSegment {
                text: text.to_string(),
                ending: ending.to_string(),
            });
        }
        return segments;
    }
    if contents.contains('\r') {
        for segment in contents.split_inclusive('\r') {
            let (text, ending) = split_line_ending(segment);
            segments.push(LineSegment {
                text: text.to_string(),
                ending: ending.to_string(),
            });
        }
        return segments;
    }
    segments.push(LineSegment {
        text: contents.to_string(),
        ending: String::new(),
    });
    segments
}

fn join_lines_preserve(segments: &[LineSegment], original: &str) -> String {
    if segments.is_empty() {
        return String::new();
    }
    let default_ending = detect_line_ending(original);
    let trailing_newline = original.ends_with('\n') || original.ends_with('\r');
    let mut output = String::new();
    for (index, segment) in segments.iter().enumerate() {
        output.push_str(&segment.text);
        let is_last = index + 1 == segments.len();
        if is_last {
            if trailing_newline {
                let ending = if segment.ending.is_empty() {
                    default_ending
                } else {
                    segment.ending.as_str()
                };
                output.push_str(ending);
            }
        } else {
            let ending = if segment.ending.is_empty() {
                default_ending
            } else {
                segment.ending.as_str()
            };
            output.push_str(ending);
        }
    }
    output
}

fn is_table_start(lines: &[LineSegment], index: usize) -> bool {
    detect_table_block(lines, index).is_some()
}

fn detect_table_block(lines: &[LineSegment], index: usize) -> Option<usize> {
    if index + 1 >= lines.len() {
        return None;
    }
    let header = lines[index].text.as_str();
    let separator = lines[index + 1].text.as_str();
    if !is_table_header_line(header) || !is_table_separator_line(separator) {
        return None;
    }
    let mut end = index + 1;
    let mut i = index + 2;
    while i < lines.len() {
        let line = lines[i].text.as_str();
        if line.trim().is_empty() || is_fence_line(line) {
            break;
        }
        if !is_table_row_line(line) {
            break;
        }
        end = i;
        i += 1;
    }
    Some(end)
}

fn detect_list_item_block(lines: &[LineSegment], index: usize) -> Option<usize> {
    let indent = list_item_indent(lines[index].text.as_str())?;
    let mut end = index;
    let mut i = index + 1;
    while i < lines.len() {
        let line = lines[i].text.as_str();
        if line.trim().is_empty() {
            let mut next = i + 1;
            while next < lines.len() && lines[next].text.trim().is_empty() {
                next += 1;
            }
            if next >= lines.len() {
                end = i;
                i += 1;
                continue;
            }
            let next_indent = leading_indent(lines[next].text.as_str());
            if next_indent > indent {
                end = i;
                i += 1;
                continue;
            }
            break;
        }
        let line_indent = leading_indent(line);
        if line_indent > indent {
            end = i;
            i += 1;
            continue;
        }
        if is_list_item_start(line) {
            break;
        }
        break;
    }
    Some(end)
}

fn is_list_item_start(line: &str) -> bool {
    list_item_indent(line).is_some()
}

fn list_item_indent(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() && matches!(bytes[index], b' ' | b'\t') {
        index += 1;
    }
    let indent = index;
    if index >= bytes.len() {
        return None;
    }

    match bytes[index] {
        b'-' | b'+' | b'*' => {
            index += 1;
            if index < bytes.len() && matches!(bytes[index], b' ' | b'\t') {
                return Some(indent);
            }
        }
        b'0'..=b'9' => {
            let start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            if index > start && index < bytes.len() && bytes[index] == b'.' {
                index += 1;
                if index < bytes.len() && matches!(bytes[index], b' ' | b'\t') {
                    return Some(indent);
                }
            }
        }
        _ => {}
    }

    None
}

fn is_heading_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let bytes = trimmed.as_bytes();
    let mut count = 0usize;
    while count < bytes.len() && bytes[count] == b'#' {
        count += 1;
    }
    if count == 0 || count > 6 {
        return false;
    }
    if count >= bytes.len() {
        return false;
    }
    matches!(bytes[count], b' ' | b'\t')
}

fn is_table_header_line(line: &str) -> bool {
    !line.trim().is_empty() && line.contains('|')
}

fn is_table_row_line(line: &str) -> bool {
    !line.trim().is_empty() && line.contains('|')
}

fn is_table_separator_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    if !trimmed.contains('|') || !trimmed.contains('-') {
        return false;
    }
    for ch in trimmed.chars() {
        match ch {
            '|' | ':' | '-' | ' ' | '\t' => {}
            _ => return false,
        }
    }
    true
}

fn leading_indent(line: &str) -> usize {
    line.chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .count()
}

fn render_task_list_form(doc_id: &str, list_index: usize) -> String {
    let escaped_doc_id = html_escape(doc_id);
    format!(
        "<form class=\"todo-quick-add\" method=\"post\" action=\"/api/doc/add-task\">\
<input type=\"hidden\" name=\"doc_id\" value=\"{escaped_doc_id}\" />\
<input type=\"hidden\" name=\"list_index\" value=\"{list_index}\" />\
<button type=\"submit\">+</button>\
<input name=\"text\" type=\"text\" placeholder=\"\" autocomplete=\"off\" required />\
</form>"
    )
}

fn is_task_list_marker(line: &str) -> bool {
    line.trim() == "+"
}

fn doc_id_to_path(doc_id: &str) -> Option<PathBuf> {
    if doc_id.is_empty() {
        return None;
    }
    let path = Path::new(doc_id);
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            _ => return None,
        }
    }
    if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
        return None;
    }
    Some(path.to_path_buf())
}

struct TaskLineParts<'a> {
    prefix: &'a str,
    suffix: &'a str,
    checked: bool,
}

fn parse_task_line(line: &str) -> Option<TaskLineParts<'_>> {
    let bytes = line.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t') {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }

    let marker = bytes[i];
    if !matches!(marker, b'-' | b'*' | b'+') {
        return None;
    }
    i += 1;

    let mut j = i;
    while j < bytes.len() && matches!(bytes[j], b' ' | b'\t') {
        j += 1;
    }
    if j == i {
        return None;
    }
    if j + 2 >= bytes.len() {
        return None;
    }
    if bytes[j] != b'[' {
        return None;
    }
    let status = bytes[j + 1];
    if !matches!(status, b' ' | b'x' | b'X') {
        return None;
    }
    if bytes[j + 2] != b']' {
        return None;
    }
    let after = j + 3;
    if after < bytes.len() && !matches!(bytes[after], b' ' | b'\t') {
        return None;
    }

    Some(TaskLineParts {
        prefix: &line[..j],
        suffix: &line[after..],
        checked: status != b' ',
    })
}

fn extract_mentions_from_line(line: &str) -> Vec<String> {
    let bytes = line.as_bytes();
    let mut mentions = Vec::new();
    let mut idx = 0usize;

    while idx < bytes.len() {
        if bytes[idx] == b'@' && is_mention_boundary(bytes, idx) {
            let start = idx + 1;
            if start < bytes.len() && is_username_start(bytes[start]) {
                let mut end = start + 1;
                while end < bytes.len() && is_username_char(bytes[end]) {
                    end += 1;
                }
                if let Some(user) = line.get(start..end) {
                    mentions.push(user.to_string());
                }
                idx = end;
                continue;
            }
        }
        idx += 1;
    }

    mentions
}

fn is_mention_boundary(bytes: &[u8], at: usize) -> bool {
    if at == 0 {
        return true;
    }
    !is_username_char(bytes[at - 1])
}

fn is_username_start(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn is_username_char(byte: u8) -> bool {
    is_username_start(byte) || byte == b'-'
}

fn is_fence_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

fn split_line_ending(segment: &str) -> (&str, &str) {
    if let Some(without_nl) = segment.strip_suffix('\n') {
        if let Some(without_cr) = without_nl.strip_suffix('\r') {
            return (without_cr, "\r\n");
        }
        return (without_nl, "\n");
    }
    if let Some(without_cr) = segment.strip_suffix('\r') {
        return (without_cr, "\r");
    }
    (segment, "")
}

fn detect_line_ending(contents: &str) -> &'static str {
    if contents.contains("\r\n") {
        "\r\n"
    } else if contents.contains('\n') {
        "\n"
    } else if contents.contains('\r') {
        "\r"
    } else {
        "\n"
    }
}

fn map_io_to_doc_error(err: std::io::Error) -> DocError {
    if err.kind() == ErrorKind::InvalidInput {
        DocError::BadPath
    } else {
        DocError::Io(err)
    }
}

pub(crate) fn rewrite_relative_md_links<'a>(event: Event<'a>, doc_id: &str) -> Event<'a> {
    match event {
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            if let Some(new_dest) = rewrite_relative_md_link(doc_id, dest_url.as_ref()) {
                Event::Start(Tag::Link {
                    link_type,
                    dest_url: new_dest.into(),
                    title,
                    id,
                })
            } else {
                Event::Start(Tag::Link {
                    link_type,
                    dest_url,
                    title,
                    id,
                })
            }
        }
        _ => event,
    }
}

pub(crate) fn rewrite_relative_image_links<'a>(event: Event<'a>, doc_id: &str) -> Event<'a> {
    match event {
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            if let Some(new_dest) = rewrite_relative_image_link(doc_id, dest_url.as_ref()) {
                Event::Start(Tag::Image {
                    link_type,
                    dest_url: new_dest.into(),
                    title,
                    id,
                })
            } else {
                Event::Start(Tag::Image {
                    link_type,
                    dest_url,
                    title,
                    id,
                })
            }
        }
        _ => event,
    }
}

fn rewrite_relative_md_link(doc_id: &str, dest_url: &str) -> Option<String> {
    let (path_part, fragment) = split_link_fragment(dest_url);
    if path_part.is_empty() || is_absolute_or_scheme(path_part) {
        return None;
    }

    let (prefix, resolved) = if path_part.ends_with(".md") {
        let resolved = resolve_relative_doc_id(doc_id, path_part)?;
        doc_id_to_path(&resolved)?;
        ("/doc/", resolved)
    } else if has_extension_ignore_ascii_case(path_part, ".pdf") {
        let resolved = resolve_relative_path(doc_id, path_part)?;
        ("/pdf/", resolved)
    } else {
        return None;
    };

    let mut new_dest = String::from(prefix);
    new_dest.push_str(&resolved);
    if let Some(fragment) = fragment {
        new_dest.push('#');
        new_dest.push_str(fragment);
    }
    Some(new_dest)
}

fn rewrite_relative_image_link(doc_id: &str, dest_url: &str) -> Option<String> {
    let (path_part, fragment) = split_link_fragment(dest_url);
    if path_part.is_empty() || is_absolute_or_scheme(path_part) {
        return None;
    }

    let resolved = resolve_relative_path(doc_id, path_part)?;

    let mut new_dest = String::from("/file/");
    new_dest.push_str(&resolved);
    if let Some(fragment) = fragment {
        new_dest.push('#');
        new_dest.push_str(fragment);
    }
    Some(new_dest)
}

fn split_link_fragment(dest_url: &str) -> (&str, Option<&str>) {
    match dest_url.split_once('#') {
        Some((path, fragment)) => (path, Some(fragment)),
        None => (dest_url, None),
    }
}

fn is_absolute_or_scheme(path: &str) -> bool {
    if path.starts_with('/') || path.contains("://") {
        return true;
    }
    if let Some(colon) = path.find(':') {
        let slash = path.find('/');
        if slash.is_none_or(|slash| colon < slash) {
            return true;
        }
    }
    false
}

fn resolve_relative_doc_id(doc_id: &str, dest_path: &str) -> Option<String> {
    let resolved = resolve_relative_path(doc_id, dest_path)?;
    Some(resolved)
}

fn has_extension_ignore_ascii_case(path: &str, ext: &str) -> bool {
    path.len() >= ext.len() && path[path.len() - ext.len()..].eq_ignore_ascii_case(ext)
}

fn resolve_relative_path(doc_id: &str, dest_path: &str) -> Option<String> {
    let mut parts: Vec<&str> = doc_id.split('/').collect();
    if parts.is_empty() {
        return None;
    }
    parts.pop();

    for part in dest_path.split('/') {
        match part {
            "" => return None,
            "." => {}
            ".." => {
                parts.pop()?;
            }
            _ => parts.push(part),
        }
    }

    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

pub(crate) fn normalize_newlines(contents: &str) -> String {
    if !contents.contains('\r') {
        return contents.to_string();
    }
    let normalized = contents.replace("\r\n", "\n");
    normalized.replace('\r', "\n")
}

#[derive(Debug)]
pub(crate) enum DocError {
    BadPath,
    NotFound,
    Io(std::io::Error),
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use crate::test_support::create_temp_root;

    #[test]
    fn rewrite_relative_md_links__should_rewrite_relative_md_links() {
        // Given
        let markdown = "\
[B](b.md)
[Up](../c.md)
[Dot](./d.md)
[Frag](b.md#section)
[Pdf](tickets/show.pdf)
[PdfUpper](tickets/show.PDF#page=3)
[PdfUp](../ticket.pdf#page=2)
[Abs](https://example.com/a.md)
[PdfAbs](https://example.com/ticket.pdf)
[Root](/notes/e.md)
[PdfRoot](/tickets/root.pdf)
[Other](f.txt)
";
        let mut body = String::new();
        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);

        // When
        let parser = Parser::new_ext(markdown, options)
            .map(|event| rewrite_relative_md_links(event, "notes/a.md"));
        pulldown_cmark::html::push_html(&mut body, parser);

        // Then
        assert!(body.contains(r#"href="/doc/notes/b.md""#));
        assert!(body.contains(r#"href="/doc/c.md""#));
        assert!(body.contains(r#"href="/doc/notes/d.md""#));
        assert!(body.contains(r#"href="/doc/notes/b.md#section""#));
        assert!(body.contains(r#"href="/pdf/notes/tickets/show.pdf""#));
        assert!(body.contains(r#"href="/pdf/notes/tickets/show.PDF#page=3""#));
        assert!(body.contains(r#"href="/pdf/ticket.pdf#page=2""#));
        assert!(body.contains(r#"href="https://example.com/a.md""#));
        assert!(body.contains(r#"href="https://example.com/ticket.pdf""#));
        assert!(body.contains(r#"href="/notes/e.md""#));
        assert!(body.contains(r#"href="/tickets/root.pdf""#));
        assert!(body.contains(r#"href="f.txt""#));
    }

    #[test]
    fn rewrite_relative_image_links__should_rewrite_relative_image_links() {
        // Given
        let markdown = "\
![A](images/a.png)
![Up](../b.jpg)
![Abs](https://example.com/a.png)
![Root](/c.png)
";
        let mut body = String::new();
        let options = Options::empty();

        // When
        let parser = Parser::new_ext(markdown, options)
            .map(|event| rewrite_relative_image_links(event, "notes/doc.md"));
        pulldown_cmark::html::push_html(&mut body, parser);

        // Then
        assert!(body.contains(r#"src="/file/notes/images/a.png""#));
        assert!(body.contains(r#"src="/file/b.jpg""#));
        assert!(body.contains(r#"src="https://example.com/a.png""#));
        assert!(body.contains(r#"src="/c.png""#));
    }

    #[test]
    fn normalize_newlines__should_convert_crlf_to_lf() {
        // Given
        let contents = "a\r\nb\rc";

        // When
        let normalized = normalize_newlines(contents);

        // Then
        assert_eq!(normalized, "a\nb\nc");
    }

    #[test]
    fn scan_block_ranges__should_detect_common_blocks() {
        // Given
        let contents = "\
# Title
\n\
Paragraph line 1
Paragraph line 2
\n\
- item one
  continuation
- item two
\n\
```
code
```
\n\
| A | B |
| --- | --- |
| 1 | 2 |
\n\
Final.\n";

        // When
        let blocks = scan_block_ranges(contents);

        // Then
        let expected = vec![
            (0, 0, BlockKind::Heading),
            (1, 1, BlockKind::Blank),
            (2, 3, BlockKind::Paragraph),
            (4, 4, BlockKind::Blank),
            (5, 6, BlockKind::ListItem),
            (7, 7, BlockKind::ListItem),
            (8, 8, BlockKind::Blank),
            (9, 11, BlockKind::Fence),
            (12, 12, BlockKind::Blank),
            (13, 15, BlockKind::Table),
            (16, 16, BlockKind::Blank),
            (17, 17, BlockKind::Paragraph),
        ];
        assert_eq!(blocks.len(), expected.len());
        for (block, (start, end, kind)) in blocks.iter().zip(expected.iter()) {
            assert_eq!(block.start, *start);
            assert_eq!(block.end, *end);
            assert_eq!(block.kind, *kind);
        }
    }

    #[test]
    fn reorder_range__should_move_lines() {
        // Given
        let contents = "a\nb\nc\n";

        // When
        let updated = reorder_range(contents, 0, 0, 2).expect("reorder");

        // Then
        assert_eq!(updated.unwrap(), "b\na\nc\n");
    }

    #[test]
    fn reorder_range__should_preserve_missing_trailing_newline() {
        // Given
        let contents = "a\nb";

        // When
        let updated = reorder_range(contents, 1, 1, 0).expect("reorder");

        // Then
        let updated = updated.unwrap();
        assert_eq!(updated, "b\na");
        assert!(!updated.ends_with('\n'));
    }

    #[test]
    fn collect_markdown_paths__should_ignore_non_md_files_and_symlinks() {
        // Given
        let root = create_temp_root("collect");
        std::fs::write(root.join("a.md"), "# A").expect("write a.md");
        std::fs::write(root.join("b.txt"), "B").expect("write b.txt");
        std::fs::create_dir_all(root.join("notes")).expect("create notes dir");
        std::fs::write(root.join("notes").join("c.md"), "# C").expect("write c.md");

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(root.join("a.md"), root.join("link.md")).expect("create symlink");
        }

        // When
        let mut doc_ids: Vec<String> = collect_markdown_paths(&root)
            .expect("collect paths")
            .into_iter()
            .filter_map(|path| doc_id_from_path(&root, &path))
            .collect();
        doc_ids.sort();

        // Then
        assert_eq!(doc_ids, vec!["a.md".to_string(), "notes/c.md".to_string()]);

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn collect_markdown_paths__should_skip_hidden_directories() {
        // Given
        let root = create_temp_root("collect-hidden");
        std::fs::write(root.join("a.md"), "# A").expect("write a.md");
        std::fs::create_dir_all(root.join(".git")).expect("create .git dir");
        std::fs::write(root.join(".git").join("HEAD.md"), "ref").expect("write .git/HEAD.md");
        std::fs::create_dir_all(root.join(".obsidian")).expect("create .obsidian dir");
        std::fs::write(root.join(".obsidian").join("config.md"), "{}").expect("write config.md");
        std::fs::create_dir_all(root.join("visible")).expect("create visible dir");
        std::fs::write(root.join("visible").join("b.md"), "# B").expect("write b.md");

        // When
        let mut doc_ids: Vec<String> = collect_markdown_paths(&root)
            .expect("collect paths")
            .into_iter()
            .filter_map(|path| doc_id_from_path(&root, &path))
            .collect();
        doc_ids.sort();

        // Then
        assert_eq!(
            doc_ids,
            vec!["a.md".to_string(), "visible/b.md".to_string()]
        );

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn create_document__should_create_file_and_parent_dirs() {
        // Given
        let root = create_temp_root("create");

        // When
        create_document(&root, "notes/new.md", "# New\n").expect("create document");

        // Then
        let contents = std::fs::read_to_string(root.join("notes/new.md")).expect("read file");
        assert_eq!(contents, "# New\n");
    }

    #[test]
    fn create_document__should_reject_duplicate_paths() {
        // Given
        let root = create_temp_root("create-existing");
        std::fs::write(root.join("a.md"), "A").expect("write a.md");

        // When
        let err = create_document(&root, "a.md", "B").expect_err("should fail");

        // Then
        match err {
            DocError::Io(err) => assert_eq!(err.kind(), ErrorKind::AlreadyExists),
            _ => panic!("expected already exists error"),
        }
    }

    #[test]
    fn create_document__should_reject_parent_traversal() {
        // Given
        let root = create_temp_root("create-bad-path");

        // When
        let err = create_document(&root, "../outside.md", "oops").expect_err("should fail");

        // Then
        assert!(matches!(err, DocError::BadPath));
    }

    #[cfg(unix)]
    #[test]
    fn create_document__should_reject_symlinked_parent() {
        use std::os::unix::fs::symlink;

        // Given
        let root = create_temp_root("create-symlink");
        let outside = create_temp_root("create-symlink-outside");
        symlink(&outside, root.join("link")).expect("create symlink");

        // When
        let err = create_document(&root, "link/escape.md", "oops").expect_err("should fail");

        // Then
        assert!(matches!(err, DocError::BadPath));
    }

    #[test]
    fn render_task_list_markdown__should_inject_checkboxes_and_skip_fences() {
        // Given
        let contents = "\
- [ ] one
* [x] two
+ [X] three
+
```md
- [ ] nope
```
";

        // When
        let rendered = render_task_list_markdown(contents, "notes.md");

        // Then
        assert_eq!(rendered.matches("todo-checkbox").count(), 3);
        assert!(rendered.contains("data-task-index=\"0\""));
        assert!(rendered.contains("data-task-index=\"1\""));
        assert!(rendered.contains("data-task-index=\"2\""));
        assert!(rendered.contains("data-task-index=\"1\" checked"));
        assert!(rendered.contains("data-task-index=\"2\" checked"));
        assert_eq!(rendered.matches("todo-quick-add").count(), 1);
        assert!(!rendered.contains("+\n"));
        assert!(rendered.contains("```md\n- [ ] nope\n```"));
    }

    #[test]
    fn render_task_list_markdown__should_render_per_list_forms() {
        // Given
        let contents = "- [ ] One\n+\n\n- [ ] Two\n- [ ] Three\n+\n";

        // When
        let rendered = render_task_list_markdown(contents, "todo.md");

        // Then
        assert_eq!(rendered.matches("todo-quick-add").count(), 2);
        assert!(rendered.contains("name=\"list_index\" value=\"0\""));
        assert!(rendered.contains("name=\"list_index\" value=\"1\""));
    }

    #[test]
    fn render_task_list_markdown__should_escape_doc_id_in_form() {
        // Given — a doc_id containing characters that could break HTML attributes
        let contents = "- [ ] One\n+\n";

        // When
        let rendered = render_task_list_markdown(contents, "path/with\"quotes.md");

        // Then — the doc_id should be HTML-escaped in the value attribute
        assert!(rendered.contains("value=\"path/with&quot;quotes.md\""));
        assert!(!rendered.contains("value=\"path/with\"quotes.md\""));
    }

    #[test]
    fn collect_mentions__should_find_mentions_and_skip_fences() {
        // Given
        let contents = "\
Ping @marten about the @roadmap.
```md
@ignored
```
Follow up with @marten and @marten again.
Edge: email@example.com and @not+valid and @ok-name.
";

        // When
        let mentions = collect_mentions(contents);

        // Then
        assert_eq!(
            mentions,
            vec![
                (
                    "marten".to_string(),
                    "Ping @marten about the @roadmap.".to_string()
                ),
                (
                    "roadmap".to_string(),
                    "Ping @marten about the @roadmap.".to_string()
                ),
                (
                    "marten".to_string(),
                    "Follow up with @marten and @marten again.".to_string()
                ),
                (
                    "not".to_string(),
                    "Edge: email@example.com and @not+valid and @ok-name.".to_string()
                ),
                (
                    "ok-name".to_string(),
                    "Edge: email@example.com and @not+valid and @ok-name.".to_string()
                ),
            ]
        );
    }

    #[test]
    fn toggle_task_item__should_update_target() {
        // Given
        let contents = "\
- [ ] one
- [x] two
";

        // When
        let updated = toggle_task_item(contents, 1, false).expect("updated");

        // Then
        assert!(updated.contains("- [ ] one"));
        assert!(updated.contains("- [ ] two"));
    }

    #[test]
    fn toggle_task_item__should_ignore_tasks_inside_fences() {
        // Given
        let contents = "\
```
- [ ] nope
```
- [ ] yes
";

        // When
        let updated = toggle_task_item(contents, 0, true).expect("updated");

        // Then
        assert!(updated.contains("```"));
        assert!(updated.contains("- [ ] nope"));
        assert!(updated.contains("- [x] yes"));
    }

    #[test]
    fn toggle_task_item__should_return_none_for_missing_index() {
        // Given
        let contents = "- [ ] one\n";

        // When
        let updated = toggle_task_item(contents, 3, true);

        // Then
        assert!(updated.is_none());
    }

    #[test]
    fn add_task_item_in_list__should_insert_after_last_task() {
        // Given
        let contents = "- [ ] One\n- [x] Two\nNotes\n";

        // When
        let updated = add_task_item_in_list(contents, 0, "Three");

        // Then
        assert_eq!(updated, "- [ ] One\n- [x] Two\n- [ ] Three\nNotes\n");
    }

    #[test]
    fn add_task_item_in_list__should_append_when_no_tasks() {
        // Given
        let contents = "Notes\n";

        // When
        let updated = add_task_item_in_list(contents, 0, "New task");

        // Then
        assert_eq!(updated, "Notes\n- [ ] New task");
    }

    #[test]
    fn add_task_item_in_list__should_ignore_fenced_tasks() {
        // Given
        let contents = "```\n- [ ] Nope\n```\n";

        // When
        let updated = add_task_item_in_list(contents, 0, "Yep");

        // Then
        assert_eq!(updated, "```\n- [ ] Nope\n```\n- [ ] Yep");
    }

    #[test]
    fn add_task_item_in_list__should_target_list_index() {
        // Given
        let contents = "- [ ] One\n\n- [ ] Two\n- [ ] Three\n";

        // When
        let updated = add_task_item_in_list(contents, 1, "Four");

        // Then
        assert_eq!(updated, "- [ ] One\n\n- [ ] Two\n- [ ] Three\n- [ ] Four\n");
    }

    #[test]
    fn add_task_item_in_list__should_noop_when_list_missing() {
        // Given
        let contents = "- [ ] One\n";

        // When
        let updated = add_task_item_in_list(contents, 2, "Two");

        // Then
        assert_eq!(updated, contents);
    }

    // -- search --

    #[test]
    fn find_match_snippet__should_return_first_matching_line() {
        // Given
        let contents = "First line\nSecond contains hello world\nThird line\n";

        // When
        let snippet = find_match_snippet(contents, "hello");

        // Then
        assert_eq!(snippet, Some("Second contains hello world".to_string()));
    }

    #[test]
    fn find_match_snippet__should_be_case_insensitive() {
        // Given
        let contents = "Title: Hello World\n";

        // When
        let snippet = find_match_snippet(contents, "hello");

        // Then
        assert_eq!(snippet, Some("Title: Hello World".to_string()));
    }

    #[test]
    fn find_match_snippet__should_return_none_when_no_match() {
        // Given
        let contents = "Nothing relevant here\n";

        // When
        let snippet = find_match_snippet(contents, "missing");

        // Then
        assert_eq!(snippet, None);
    }

    #[test]
    fn search_documents__should_find_matching_docs() {
        // Given
        let root = create_temp_root("search");
        std::fs::write(root.join("alpha.md"), "Alpha has the needle").expect("write");
        std::fs::write(root.join("beta.md"), "Beta has nothing").expect("write");
        std::fs::write(root.join("gamma.md"), "Gamma also has the needle").expect("write");

        // When
        let results = search_documents(&root, "needle").expect("search");

        // Then
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].doc_id, "alpha.md");
        assert_eq!(results[0].snippet, "Alpha has the needle");
        assert_eq!(results[1].doc_id, "gamma.md");

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn search_documents__should_return_empty_for_no_matches() {
        // Given
        let root = create_temp_root("search-empty");
        std::fs::write(root.join("doc.md"), "Nothing here").expect("write");

        // When
        let results = search_documents(&root, "missing").expect("search");

        // Then
        assert!(results.is_empty());

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    // -- rendering --

    #[test]
    fn render_document_html__should_render_basic_markdown() {
        // Given
        let markdown = "# Hello\n\nA paragraph.\n";

        // When
        let result = render_document_html(markdown, "test.md");

        // Then
        assert!(result.html.contains("<h1>Hello</h1>"));
        assert!(result.html.contains("<p>A paragraph.</p>"));
        assert!(!result.has_mermaid);
        assert!(!result.has_abc);
    }

    #[test]
    fn render_document_html__should_extract_mermaid_blocks() {
        // Given
        let markdown = "```mermaid\ngraph TD;\nA-->B;\n```\n";

        // When
        let result = render_document_html(markdown, "test.md");

        // Then
        assert!(result.has_mermaid);
        assert!(result.html.contains(r#"class="mermaid"#));
        assert!(result.html.contains("A--&gt;B;"));
    }

    #[test]
    fn render_document_html__should_extract_abc_blocks() {
        // Given
        let markdown = "```abc\nX:1\nT:Test\nK:C\n```\n";

        // When
        let result = render_document_html(markdown, "test.md");

        // Then
        assert!(result.has_abc);
        assert!(result.html.contains(r#"class="abc-notation"#));
    }

    #[test]
    fn render_document_html__should_render_inline_math() {
        // Given
        let markdown = "Equation: $x^2$\n";

        // When
        let result = render_document_html(markdown, "test.md");

        // Then
        assert!(result.html.contains("<math"));
    }

    #[test]
    fn render_document_html__should_render_tables() {
        // Given
        let markdown = "| A | B |\n|---|---|\n| 1 | 2 |\n";

        // When
        let result = render_document_html(markdown, "test.md");

        // Then
        assert!(result.html.contains("<table>"));
    }
}
