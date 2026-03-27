//! Pre-processing of magent response blocks in markdown.
//!
//! Magent is an LLM agent that writes responses inline in markdown files using
//! custom `<magent-*>` elements. This module transforms those elements into
//! structured HTML before pulldown-cmark processes the document, avoiding the
//! CommonMark HTML-block parsing issues that would otherwise garble the output.

use crate::html;
use pulldown_cmark::{Options, Parser};

/// Pre-process magent response blocks in raw markdown.
///
/// Finds `<magent-response>...</magent-response>` blocks (outside fenced code
/// blocks) and replaces them with structured HTML. Returns the processed
/// markdown and whether any magent blocks were found.
pub(super) fn render_magent_blocks(markdown: &str) -> (String, bool) {
    let mut output = String::with_capacity(markdown.len());
    let mut has_magent = false;
    let mut in_fence = false;
    let mut depth: usize = 0;
    let mut response_buf = String::new();

    for segment in markdown.split_inclusive('\n') {
        let (line, _) = super::split_line_ending(segment);

        if depth == 0 {
            if super::is_fence_line(line) {
                in_fence = !in_fence;
            }

            if !in_fence && is_response_open(line) {
                depth = 1;
                response_buf.clear();
                continue;
            }

            output.push_str(segment);
        } else if is_response_close(line) {
            depth -= 1;
            if depth == 0 {
                output.push_str(&render_response(&response_buf));
                output.push('\n');
                has_magent = true;
                response_buf.clear();
            } else {
                response_buf.push_str(segment);
            }
        } else {
            if is_response_open(line) {
                depth += 1;
            }
            response_buf.push_str(segment);
        }
    }

    // Unclosed response block: output raw content so nothing is silently lost.
    if depth > 0 {
        output.push_str("<magent-response>\n");
        output.push_str(&response_buf);
    }

    (output, has_magent)
}

fn is_response_open(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("<magent-response") && trimmed.ends_with('>')
}

fn is_response_close(line: &str) -> bool {
    line.trim().starts_with("</magent-response>")
}

// ---------------------------------------------------------------------------
// Response rendering
// ---------------------------------------------------------------------------

/// Render the inner content of a `<magent-response>` block to structured HTML.
///
/// The output is a single `<div class="magent-response">` with no internal
/// blank lines, so pulldown-cmark treats it as one HTML block.
fn render_response(inner: &str) -> String {
    let mut html = String::new();
    html.push_str("<div class=\"magent-response\">\n");
    render_elements(inner, &mut html);
    html.push_str("</div>");
    strip_blank_lines(&html)
}

/// Walk through response content, emitting HTML for each magent element and
/// rendering interstitial text as markdown.
fn render_elements(content: &str, html: &mut String) {
    let mut pos = 0;

    while pos < content.len() {
        match find_next_element(&content[pos..]) {
            None => {
                let text = content[pos..].trim();
                if !text.is_empty() {
                    html.push_str(&render_markdown_fragment(text));
                }
                break;
            }
            Some(tag) => {
                let text_before = content[pos..pos + tag.offset].trim();
                if !text_before.is_empty() {
                    html.push_str(&render_markdown_fragment(text_before));
                }

                let abs_content_start = pos + tag.content_start;
                let close_pattern = format!("</{}>", tag.name);

                match content[abs_content_start..].find(&close_pattern) {
                    None => {
                        // Malformed: no closing tag. Render remaining as text.
                        let rest = content[pos + tag.offset..].trim();
                        if !rest.is_empty() {
                            html.push_str(&render_markdown_fragment(rest));
                        }
                        break;
                    }
                    Some(inner_len) => {
                        let element_inner =
                            &content[abs_content_start..abs_content_start + inner_len];
                        render_element(tag.name, tag.attrs, element_inner, html);
                        pos = abs_content_start + inner_len + close_pattern.len();
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tag discovery
// ---------------------------------------------------------------------------

struct TagMatch<'a> {
    /// Byte offset of `<` from the start of the searched string.
    offset: usize,
    /// Tag name, e.g. `magent-thinking`.
    name: &'a str,
    /// Byte offset *after* `>` from the start of the searched string.
    content_start: usize,
    /// Raw attributes string, e.g. `tool="search"`.
    attrs: &'a str,
}

/// Find the next `<magent-*>` opening tag in `haystack`.
///
/// Closing tags (`</magent-*>`) are not matched because they contain a `/`
/// between `<` and `magent`, so the literal `<magent-` search skips them.
fn find_next_element<'a>(haystack: &'a str) -> Option<TagMatch<'a>> {
    let mut search_from = 0;

    while let Some(idx) = haystack[search_from..].find("<magent-") {
        let abs_idx = search_from + idx;

        if let Some(gt_offset) = haystack[abs_idx..].find('>') {
            let tag_str = &haystack[abs_idx + 1..abs_idx + gt_offset];
            let (name, attrs) = match tag_str.find(|c: char| c.is_whitespace()) {
                Some(ws) => (&tag_str[..ws], tag_str[ws..].trim()),
                None => (tag_str, ""),
            };

            // Skip nested response tags (shouldn't appear, but be safe).
            if name == "magent-response" {
                search_from = abs_idx + gt_offset + 1;
                continue;
            }

            return Some(TagMatch {
                offset: abs_idx,
                name,
                content_start: abs_idx + gt_offset + 1,
                attrs,
            });
        }

        search_from = abs_idx + 1;
    }

    None
}

// ---------------------------------------------------------------------------
// Element rendering
// ---------------------------------------------------------------------------

fn render_element(name: &str, attrs: &str, inner: &str, html: &mut String) {
    match name {
        "magent-thinking" => {
            html.push_str("<details class=\"magent-thinking\">\n");
            html.push_str("<summary>Thinking</summary>\n");
            html.push_str(&render_markdown_fragment(inner.trim()));
            html.push_str("</details>\n");
        }
        "magent-tool-call" => {
            let tool = extract_attr_value(attrs, "tool").unwrap_or("tool");
            html.push_str("<details class=\"magent-tool-call\">\n<summary>");
            html.push_str(&html::escape(tool));
            html.push_str("</summary>\n");
            if let Some(input) = extract_inner(inner, "magent-input") {
                html.push_str("<pre class=\"magent-input\"><code>");
                html.push_str(&html::escape(input.trim()));
                html.push_str("</code></pre>\n");
            }
            html.push_str("</details>\n");
        }
        "magent-tool-result" => {
            let tool = extract_attr_value(attrs, "tool").unwrap_or("tool");
            html.push_str("<details class=\"magent-tool-result\">\n<summary>");
            html.push_str(&html::escape(tool));
            html.push_str(" result</summary>\n");
            html.push_str("<pre class=\"magent-result\"><code>");
            html.push_str(&html::escape(inner.trim()));
            html.push_str("</code></pre>\n");
            html.push_str("</details>\n");
        }
        "magent-edit" => {
            let status = extract_attr_value(attrs, "status").unwrap_or("proposed");
            html.push_str("<div class=\"magent-edit\" data-status=\"");
            html.push_str(&html::escape(status));
            html.push_str("\">\n");
            if let Some(search) = extract_inner(inner, "magent-search") {
                html.push_str("<div class=\"magent-edit-search\"><pre><code>");
                html.push_str(&html::escape(search.trim()));
                html.push_str("</code></pre></div>\n");
            }
            if let Some(replace) = extract_inner(inner, "magent-replace") {
                html.push_str("<div class=\"magent-edit-replace\"><pre><code>");
                html.push_str(&html::escape(replace.trim()));
                html.push_str("</code></pre></div>\n");
            }
            html.push_str("</div>\n");
        }
        _ => {
            let trimmed = inner.trim();
            if !trimmed.is_empty() {
                html.push_str(&render_markdown_fragment(trimmed));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract a quoted attribute value, e.g. `tool="search"` → `Some("search")`.
fn extract_attr_value<'a>(attrs: &'a str, name: &str) -> Option<&'a str> {
    let pattern = format!("{name}=\"");
    let start = attrs.find(&pattern)? + pattern.len();
    let end = start + attrs[start..].find('"')?;
    Some(&attrs[start..end])
}

/// Extract the text content of a sub-element.
///
/// Given content containing `<magent-input>text</magent-input>`, returns `text`.
fn extract_inner<'a>(content: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}");
    let open_start = content.find(&open)?;
    let gt = open_start + content[open_start..].find('>')? + 1;
    let close = format!("</{tag}>");
    let close_start = gt + content[gt..].find(&close)?;
    Some(&content[gt..close_start])
}

/// Render a markdown fragment to HTML using pulldown-cmark.
fn render_markdown_fragment(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_MATH);
    let parser = Parser::new_ext(text, options);
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, parser);
    html
}

/// Remove blank lines so pulldown-cmark treats the output as a single HTML block.
fn strip_blank_lines(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    for line in html.split('\n') {
        if !line.trim().is_empty() {
            result.push_str(line);
            result.push('\n');
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    // -- render_magent_blocks -------------------------------------------------

    #[test]
    fn render_magent_blocks__no_magent_content() {
        let md = "# Hello\n\nA paragraph.\n";
        let (result, has_magent) = render_magent_blocks(md);

        assert!(!has_magent);
        assert_eq!(result, md);
    }

    #[test]
    fn render_magent_blocks__simple_text_response() {
        let md = "\
@magent hello

<magent-response>
Hello there!
</magent-response>
";
        let (result, has_magent) = render_magent_blocks(md);

        assert!(has_magent);
        assert!(result.contains("class=\"magent-response\""));
        assert!(result.contains("<p>Hello there!</p>"));
        assert!(!result.contains("<magent-response>"));
    }

    #[test]
    fn render_magent_blocks__response_with_thinking() {
        let md = "\
<magent-response>
<magent-thinking>
The user wants a greeting.
</magent-thinking>
Hi!
</magent-response>
";
        let (result, has_magent) = render_magent_blocks(md);

        assert!(has_magent);
        assert!(result.contains("class=\"magent-thinking\""));
        assert!(result.contains("<summary>Thinking</summary>"));
        assert!(result.contains("The user wants a greeting."));
        assert!(result.contains("<p>Hi!</p>"));
    }

    #[test]
    fn render_magent_blocks__response_with_tool_call_and_result() {
        let md = "\
<magent-response>
<magent-tool-call tool=\"search\">
<magent-input>query text</magent-input>
</magent-tool-call>
<magent-tool-result tool=\"search\">
3 matches found.
</magent-tool-result>
Based on the results, the answer is 42.
</magent-response>
";
        let (result, has_magent) = render_magent_blocks(md);

        assert!(has_magent);
        assert!(result.contains("class=\"magent-tool-call\""));
        assert!(result.contains("<summary>search</summary>"));
        assert!(result.contains("query text"));
        assert!(result.contains("class=\"magent-tool-result\""));
        assert!(result.contains("search result</summary>"));
        assert!(result.contains("3 matches found."));
        assert!(result.contains("<p>Based on the results, the answer is 42.</p>"));
    }

    #[test]
    fn render_magent_blocks__response_with_edit() {
        let md = "\
<magent-response>
Fixed the URL:
<magent-edit status=\"proposed\">
<magent-search>htps://example.com</magent-search>
<magent-replace>https://example.com</magent-replace>
</magent-edit>
</magent-response>
";
        let (result, has_magent) = render_magent_blocks(md);

        assert!(has_magent);
        assert!(result.contains("class=\"magent-edit\""));
        assert!(result.contains("data-status=\"proposed\""));
        assert!(result.contains("class=\"magent-edit-search\""));
        assert!(result.contains("htps://example.com"));
        assert!(result.contains("class=\"magent-edit-replace\""));
        assert!(result.contains("https://example.com"));
    }

    #[test]
    fn render_magent_blocks__skips_fenced_code_blocks() {
        let md = "\
```
<magent-response>
This should not be processed.
</magent-response>
```
";
        let (result, has_magent) = render_magent_blocks(md);

        assert!(!has_magent);
        assert!(result.contains("<magent-response>"));
    }

    #[test]
    fn render_magent_blocks__multiple_responses() {
        let md = "\
<magent-response>
First.
</magent-response>

<magent-response>
Second.
</magent-response>
";
        let (result, has_magent) = render_magent_blocks(md);

        assert!(has_magent);
        assert!(result.contains("<p>First.</p>"));
        assert!(result.contains("<p>Second.</p>"));
        assert_eq!(result.matches("magent-response").count(), 2);
    }

    #[test]
    fn render_magent_blocks__unclosed_response_preserved() {
        let md = "\
<magent-response>
Unclosed content.
";
        let (result, has_magent) = render_magent_blocks(md);

        assert!(!has_magent);
        assert!(result.contains("<magent-response>"));
        assert!(result.contains("Unclosed content."));
    }

    #[test]
    fn render_magent_blocks__nested_response_in_tool_result() {
        let md = "\
<magent-response>
<magent-tool-result tool=\"search\">
file.md:1: some text
<magent-response>
Nested response content.
</magent-response>
</magent-tool-result>
The summary.
</magent-response>
";
        let (result, has_magent) = render_magent_blocks(md);

        assert!(has_magent);
        assert!(result.contains("class=\"magent-tool-result\""));
        assert!(result.contains("Nested response content."));
        assert!(result.contains("<p>The summary.</p>"));
    }

    #[test]
    fn render_magent_blocks__html_escaping_in_tool_result() {
        let md = "\
<magent-response>
<magent-tool-result tool=\"run\">
1 < 2 && 3 > 2
</magent-tool-result>
</magent-response>
";
        let (result, has_magent) = render_magent_blocks(md);

        assert!(has_magent);
        assert!(result.contains("1 &lt; 2 &amp;&amp; 3 &gt; 2"));
    }

    #[test]
    fn render_magent_blocks__no_blank_lines_in_output() {
        let md = "\
<magent-response>
<magent-thinking>
Thinking hard.
</magent-thinking>
The answer.
</magent-response>
";
        let (result, _) = render_magent_blocks(md);

        // Extract the HTML block (between the first <div and the trailing newlines)
        let div_start = result.find("<div").unwrap();
        let div_end = result.rfind("</div>").unwrap() + "</div>".len();
        let html_block = &result[div_start..div_end];

        // No blank lines within the HTML block
        assert!(
            !html_block.contains("\n\n"),
            "HTML block should not contain blank lines, got:\n{html_block}"
        );
    }

    #[test]
    fn render_magent_blocks__multiple_edits_in_one_response() {
        let md = "\
<magent-response>
Fixed URLs:
<magent-edit status=\"proposed\">
<magent-search>htps://a.com</magent-search>
<magent-replace>https://a.com</magent-replace>
</magent-edit>
<magent-edit status=\"proposed\">
<magent-search>htps://b.com</magent-search>
<magent-replace>https://b.com</magent-replace>
</magent-edit>
</magent-response>
";
        let (result, has_magent) = render_magent_blocks(md);

        assert!(has_magent);
        assert_eq!(result.matches("magent-edit-search").count(), 2);
        assert!(result.contains("htps://a.com"));
        assert!(result.contains("htps://b.com"));
    }

    #[test]
    fn render_magent_blocks__preserves_surrounding_markdown() {
        let md = "\
# Title

Some text before.

<magent-response>
Answer.
</magent-response>

More text after.
";
        let (result, has_magent) = render_magent_blocks(md);

        assert!(has_magent);
        assert!(result.starts_with("# Title\n"));
        assert!(result.contains("Some text before."));
        assert!(result.contains("More text after."));
    }

    // -- extract_attr_value ---------------------------------------------------

    #[test]
    fn extract_attr_value__finds_value() {
        assert_eq!(
            extract_attr_value("tool=\"search\"", "tool"),
            Some("search")
        );
    }

    #[test]
    fn extract_attr_value__multiple_attrs() {
        assert_eq!(
            extract_attr_value("tool=\"read\" status=\"ok\"", "status"),
            Some("ok")
        );
    }

    #[test]
    fn extract_attr_value__missing_attr() {
        assert_eq!(extract_attr_value("tool=\"read\"", "status"), None);
    }

    // -- extract_inner --------------------------------------------------------

    #[test]
    fn extract_inner__simple() {
        let content = "<magent-input>hello</magent-input>";
        assert_eq!(extract_inner(content, "magent-input"), Some("hello"));
    }

    #[test]
    fn extract_inner__with_surrounding_text() {
        let content = "before\n<magent-search>find me</magent-search>\nafter";
        assert_eq!(extract_inner(content, "magent-search"), Some("find me"));
    }

    #[test]
    fn extract_inner__missing_tag() {
        assert_eq!(extract_inner("no tags here", "magent-input"), None);
    }
}
