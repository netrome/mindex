use super::paths::doc_id_to_path;
use super::tasks::{is_task_list_marker, parse_task_line};
use super::{is_fence_line, split_line_ending};
use crate::math::{MathStyle, html_escape, render_math};
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use std::collections::HashMap;

pub(crate) struct RenderedDocument {
    pub(crate) html: String,
    pub(crate) has_mermaid: bool,
    pub(crate) has_abc: bool,
    pub(crate) has_code: bool,
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
    let mut has_code = false;
    let mut in_mermaid = false;
    let mut in_abc = false;
    let mut in_heading = false;
    let mut heading_level = pulldown_cmark::HeadingLevel::H1;
    let mut heading_text = String::new();
    let mut heading_events: Vec<Event> = Vec::new();
    let mut seen_slugs: HashMap<String, usize> = HashMap::new();
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

        if in_heading {
            match event {
                Event::End(TagEnd::Heading(..)) => {
                    let slug = unique_slug(&heading_text, &mut seen_slugs);
                    let tag = heading_level;
                    events.push(Event::Html(format!("<{tag} id=\"{slug}\">").into()));
                    events.append(&mut heading_events);
                    events.push(Event::Html(format!("</{tag}>\n").into()));
                    heading_text.clear();
                    in_heading = false;
                }
                Event::Text(ref text) | Event::Code(ref text) => {
                    heading_text.push_str(text);
                    heading_events.push(event);
                }
                Event::SoftBreak | Event::HardBreak => {
                    heading_text.push(' ');
                    heading_events.push(event);
                }
                _ => {
                    heading_events.push(event);
                }
            }
            continue;
        }

        if let Event::Start(Tag::Heading { level, .. }) = event {
            in_heading = true;
            heading_level = level;
            heading_text.clear();
            heading_events.clear();
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
            has_code = true;
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
        has_code,
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

fn is_mermaid_info(info: &str) -> bool {
    let language = info.split_whitespace().next().unwrap_or("");
    language.eq_ignore_ascii_case("mermaid")
}

fn is_abc_info(info: &str) -> bool {
    let language = info.split_whitespace().next().unwrap_or("");
    language.eq_ignore_ascii_case("abc") || language.eq_ignore_ascii_case("abcjs")
}

fn heading_slug(text: &str) -> String {
    let mut slug = String::with_capacity(text.len());
    for ch in text.to_lowercase().chars() {
        if ch.is_alphanumeric() || ch == '-' {
            slug.push(ch);
        } else if ch == ' ' {
            slug.push('-');
        }
    }
    slug
}

fn unique_slug(text: &str, seen: &mut HashMap<String, usize>) -> String {
    let base = heading_slug(text);
    let count = seen.entry(base.clone()).or_insert(0);
    let slug = if *count == 0 {
        base.clone()
    } else {
        format!("{base}-{count}")
    };
    *count += 1;
    slug
}

fn render_task_list_form(doc_id: &str, list_index: usize) -> String {
    let escaped_doc_id = html_escape(doc_id);
    format!(
        "<form class=\"todo-quick-add\" method=\"post\" action=\"/api/d/add-task\">\
<input type=\"hidden\" name=\"doc_id\" value=\"{escaped_doc_id}\" />\
<input type=\"hidden\" name=\"list_index\" value=\"{list_index}\" />\
<button type=\"submit\">+</button>\
<input name=\"text\" type=\"text\" placeholder=\"\" autocomplete=\"off\" required />\
</form>"
    )
}

fn rewrite_relative_md_link(doc_id: &str, dest_url: &str) -> Option<String> {
    let (path_part, fragment) = split_link_fragment(dest_url);
    if path_part.is_empty() || is_absolute_or_scheme(path_part) {
        return None;
    }

    let (prefix, resolved) = if path_part.ends_with(".md") {
        let resolved = resolve_relative_path(doc_id, path_part)?;
        doc_id_to_path(&resolved)?;
        ("/d/", resolved)
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

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use pulldown_cmark::{Options, Parser};

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
        assert!(body.contains(r#"href="/d/notes/b.md""#));
        assert!(body.contains(r#"href="/d/c.md""#));
        assert!(body.contains(r#"href="/d/notes/d.md""#));
        assert!(body.contains(r#"href="/d/notes/b.md#section""#));
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

    // -- rendering --

    #[test]
    fn render_document_html__should_render_basic_markdown() {
        // Given
        let markdown = "# Hello\n\nA paragraph.\n";

        // When
        let result = render_document_html(markdown, "test.md");

        // Then
        assert!(result.html.contains("<h1 id=\"hello\">Hello</h1>"));
        assert!(result.html.contains("<p>A paragraph.</p>"));
        assert!(!result.has_mermaid);
        assert!(!result.has_abc);
        assert!(!result.has_code);
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
    fn render_document_html__should_set_has_code_for_fenced_code_blocks() {
        // Given
        let markdown = "```rust\nfn main() {}\n```\n";

        // When
        let result = render_document_html(markdown, "test.md");

        // Then
        assert!(result.has_code);
        assert!(!result.has_mermaid);
        assert!(!result.has_abc);
    }

    #[test]
    fn render_document_html__should_not_set_has_code_for_mermaid_blocks() {
        // Given
        let markdown = "```mermaid\ngraph TD;\nA-->B;\n```\n";

        // When
        let result = render_document_html(markdown, "test.md");

        // Then
        assert!(!result.has_code);
        assert!(result.has_mermaid);
    }

    #[test]
    fn render_document_html__should_not_set_has_code_for_abc_blocks() {
        // Given
        let markdown = "```abc\nX:1\nT:Test\nK:C\n```\n";

        // When
        let result = render_document_html(markdown, "test.md");

        // Then
        assert!(!result.has_code);
        assert!(result.has_abc);
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

    // -- heading slugs --

    #[test]
    fn heading_slug__should_generate_github_flavored_slug() {
        assert_eq!(heading_slug("Hello World"), "hello-world");
        assert_eq!(heading_slug("2. Proposed Design"), "2-proposed-design");
        assert_eq!(heading_slug("What is this?!"), "what-is-this");
        assert_eq!(heading_slug("kebab-case"), "kebab-case");
        assert_eq!(heading_slug("UPPER CASE"), "upper-case");
        assert_eq!(heading_slug("a/b/c"), "abc");
    }

    #[test]
    fn unique_slug__should_deduplicate() {
        // Given
        let mut seen = HashMap::new();

        // When / Then
        assert_eq!(unique_slug("Hello", &mut seen), "hello");
        assert_eq!(unique_slug("Hello", &mut seen), "hello-1");
        assert_eq!(unique_slug("Hello", &mut seen), "hello-2");
        assert_eq!(unique_slug("Other", &mut seen), "other");
    }

    #[test]
    fn render_document_html__should_add_id_to_headings() {
        // Given
        let markdown = "## 2. Proposed Design\n\nSome text.\n";

        // When
        let result = render_document_html(markdown, "test.md");

        // Then
        assert!(
            result
                .html
                .contains("<h2 id=\"2-proposed-design\">2. Proposed Design</h2>")
        );
    }

    #[test]
    fn render_document_html__should_preserve_inline_formatting_in_headings() {
        // Given
        let markdown = "## Hello **world**\n";

        // When
        let result = render_document_html(markdown, "test.md");

        // Then
        assert!(
            result
                .html
                .contains("<h2 id=\"hello-world\">Hello <strong>world</strong></h2>")
        );
    }

    #[test]
    fn render_document_html__should_deduplicate_heading_ids() {
        // Given
        let markdown = "## Section\n\n## Section\n\n## Section\n";

        // When
        let result = render_document_html(markdown, "test.md");

        // Then
        assert!(result.html.contains("id=\"section\""));
        assert!(result.html.contains("id=\"section-1\""));
        assert!(result.html.contains("id=\"section-2\""));
    }

    #[test]
    fn render_document_html__should_support_anchor_links_to_headings() {
        // Given
        let markdown = "## Target\n\n[Go](#target)\n";

        // When
        let result = render_document_html(markdown, "test.md");

        // Then
        assert!(result.html.contains("<h2 id=\"target\">Target</h2>"));
        assert!(result.html.contains("href=\"#target\""));
    }
}
