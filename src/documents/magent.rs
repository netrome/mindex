//! Pre-processing of magent response blocks in markdown.
//!
//! Magent is an LLM agent that writes responses inline in markdown files using
//! custom `<magent-*>` elements. This module transforms those elements into
//! structured HTML before pulldown-cmark processes the document, avoiding the
//! CommonMark HTML-block parsing issues that would otherwise garble the output.

use crate::html;
use pulldown_cmark::{Options, Parser};

// ---------------------------------------------------------------------------
// Region detection (for agent view block merging)
// ---------------------------------------------------------------------------

/// A line range occupied by a `<magent-response>...</magent-response>` block.
pub(crate) struct MagentRegion {
    pub(crate) start_line: usize,
    pub(crate) end_line: usize,
}

/// Scan for `<magent-response>` open/close pairs and return their line ranges.
///
/// Handles nesting and skips fenced code blocks, matching the logic in
/// `strip_magent_blocks` and `render_magent_blocks`.
pub(crate) fn find_magent_regions(contents: &str) -> Vec<MagentRegion> {
    let mut regions = Vec::new();
    let mut in_fence = false;
    let mut depth: usize = 0;
    let mut region_start: usize = 0;

    for (line_idx, segment) in contents.split_inclusive('\n').enumerate() {
        let (line, _) = super::split_line_ending(segment);

        if depth == 0 {
            if super::is_fence_line(line) {
                in_fence = !in_fence;
            }
            if !in_fence && is_response_open(line) {
                depth = 1;
                region_start = line_idx;
            }
        } else if is_response_close(line) {
            depth -= 1;
            if depth == 0 {
                regions.push(MagentRegion {
                    start_line: region_start,
                    end_line: line_idx,
                });
            }
        } else if is_response_open(line) {
            depth += 1;
        }
    }

    regions
}

// ---------------------------------------------------------------------------
// Stripping (for normal document view)
// ---------------------------------------------------------------------------

/// Strip `<magent-response>...</magent-response>` blocks from raw markdown.
///
/// Removes response blocks entirely (outside fenced code blocks) while
/// preserving all other content — including `@magent` directive lines, which
/// render naturally as paragraphs.
pub(super) fn strip_magent_blocks(markdown: &str) -> String {
    let mut output = String::with_capacity(markdown.len());
    let mut in_fence = false;
    let mut depth: usize = 0;
    let mut discard_buf = String::new();

    for segment in markdown.split_inclusive('\n') {
        let (line, _) = super::split_line_ending(segment);

        if depth == 0 {
            if super::is_fence_line(line) {
                in_fence = !in_fence;
            }

            if !in_fence && is_response_open(line) {
                depth = 1;
                discard_buf.clear();
                continue;
            }

            output.push_str(segment);
        } else if is_response_close(line) {
            depth -= 1;
            if depth == 0 {
                discard_buf.clear();
            } else {
                discard_buf.push_str(segment);
            }
        } else {
            if is_response_open(line) {
                depth += 1;
            }
            discard_buf.push_str(segment);
        }
    }

    // Unclosed response block: output raw content so nothing is silently lost.
    if depth > 0 {
        output.push_str("<magent-response>\n");
        output.push_str(&discard_buf);
    }

    output
}

/// Pre-process magent response blocks in raw markdown.
///
/// Finds `<magent-response>...</magent-response>` blocks (outside fenced code
/// blocks) and replaces them with structured HTML. Returns the processed
/// markdown and whether any magent blocks were found.
///
/// Not called from the normal document view (which uses `strip_magent_blocks`),
/// but will be used by the agent view.
#[allow(dead_code)]
pub(crate) fn render_magent_blocks(markdown: &str) -> (String, bool) {
    let mut output = String::with_capacity(markdown.len());
    let mut has_magent = false;
    let mut in_fence = false;
    let mut depth: usize = 0;
    let mut response_buf = String::new();
    let mut edit_index: usize = 0;

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
                output.push_str(&render_response(&response_buf, &mut edit_index));
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
fn render_response(inner: &str, edit_index: &mut usize) -> String {
    let mut html = String::new();
    html.push_str("<div class=\"magent-response\">\n");
    render_elements(inner, &mut html, edit_index);
    html.push_str("</div>");
    strip_blank_lines(&html)
}

/// Walk through response content, emitting HTML for each magent element and
/// rendering interstitial text as markdown.
fn render_elements(content: &str, html: &mut String, edit_index: &mut usize) {
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
                        render_element(tag.name, tag.attrs, element_inner, html, edit_index);
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

fn render_element(name: &str, attrs: &str, inner: &str, html: &mut String, edit_index: &mut usize) {
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
            html.push_str("\" data-edit-index=\"");
            html.push_str(&edit_index.to_string());
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
            *edit_index += 1;
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
// Accept edit
// ---------------------------------------------------------------------------

/// Accept a proposed magent edit by index.
///
/// Finds the Nth `<magent-edit status="proposed">` block in the document,
/// applies its search/replace to the document body, and changes the block's
/// status to `"accepted"`. Returns the updated document or `None` if the
/// edit was not found or the search text doesn't match.
pub(crate) fn accept_magent_edit(contents: &str, edit_index: usize) -> Option<String> {
    let (search, replace, edit_start, edit_tag_end) = find_proposed_edit(contents, edit_index)?;

    // Find the search text in document body (outside <magent-response> blocks).
    let match_pos = find_in_body(contents, &search)?;

    let mut output = String::with_capacity(contents.len());

    // Everything before the match, the replacement, everything after.
    output.push_str(&contents[..match_pos]);
    output.push_str(&replace);
    let after_match = match_pos + search.len();

    // The search text should appear in the body before the edit block. If it
    // doesn't, refuse to apply — reconstructing the document would be fragile.
    if after_match > edit_start {
        return None;
    }

    // Emit the rest, swapping the edit block's status to "accepted".
    output.push_str(&contents[after_match..edit_start]);
    let edit_block = &contents[edit_start..edit_tag_end];
    output.push_str(&edit_block.replacen("status=\"proposed\"", "status=\"accepted\"", 1));
    output.push_str(&contents[edit_tag_end..]);

    Some(output)
}

/// Find the byte position of `needle` in the document body (outside
/// `<magent-response>...</magent-response>` blocks). Returns the position
/// of the first match, or `None` if not found.
fn find_in_body(contents: &str, needle: &str) -> Option<usize> {
    let response_regions = collect_response_regions(contents);
    let mut search_from = 0;

    while let Some(rel) = contents[search_from..].find(needle) {
        let abs = search_from + rel;
        if !response_regions.iter().any(|(s, e)| abs >= *s && abs < *e) {
            return Some(abs);
        }
        search_from = abs + 1;
    }

    None
}

/// Collect byte ranges of all `<magent-response>...</magent-response>` blocks.
fn collect_response_regions(contents: &str) -> Vec<(usize, usize)> {
    let mut regions = Vec::new();
    let mut pos = 0;

    while let Some(open_offset) = contents[pos..].find("<magent-response") {
        let abs_open = pos + open_offset;
        // Find the matching close, accounting for nesting.
        let after_open = match contents[abs_open..].find('>') {
            Some(gt) => abs_open + gt + 1,
            None => break,
        };
        let mut depth = 1usize;
        let mut scan = after_open;
        while depth > 0 {
            let next_open = contents[scan..].find("<magent-response");
            let next_close = contents[scan..].find("</magent-response>");
            match (next_open, next_close) {
                (_, Some(c)) if next_open.is_none_or(|o| c < o) => {
                    depth -= 1;
                    scan += c + "</magent-response>".len();
                }
                (Some(o), _) => {
                    depth += 1;
                    scan += o + "<magent-response".len();
                }
                _ => break,
            }
        }
        if depth == 0 {
            regions.push((abs_open, scan));
            pos = scan;
        } else {
            break;
        }
    }

    regions
}

/// Locate the Nth edit block and return (search, replace, block_start, block_end).
///
/// The index counts *all* `<magent-edit>` blocks (proposed and accepted) to
/// match the `data-edit-index` attribute emitted during rendering. Returns
/// `None` if the block at `target_index` is not proposed.
fn find_proposed_edit(
    contents: &str,
    target_index: usize,
) -> Option<(String, String, usize, usize)> {
    let mut current = 0usize;
    let mut search_from = 0;

    while let Some(offset) = contents[search_from..].find("<magent-edit") {
        let abs = search_from + offset;
        let Some(gt) = contents[abs..].find('>') else {
            break;
        };
        let tag_str = &contents[abs..abs + gt + 1];

        let content_start = abs + gt + 1;
        let close_tag = "</magent-edit>";
        let Some(close_offset) = contents[content_start..].find(close_tag) else {
            break;
        };
        let block_end = content_start + close_offset + close_tag.len();

        if current == target_index {
            // Only accept proposed edits.
            if !tag_str.contains("status=\"proposed\"") {
                return None;
            }
            let inner = &contents[content_start..content_start + close_offset];
            let search = extract_inner(inner, "magent-search")?;
            let replace = extract_inner(inner, "magent-replace")?;
            return Some((
                search.trim().to_string(),
                replace.trim().to_string(),
                abs,
                block_end,
            ));
        }

        current += 1;
        search_from = block_end;
    }

    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    // -- find_magent_regions --------------------------------------------------

    #[test]
    fn find_magent_regions__no_magent_content() {
        let md = "# Hello\n\nA paragraph.\n";
        let regions = find_magent_regions(md);
        assert!(regions.is_empty());
    }

    #[test]
    fn find_magent_regions__single_response() {
        let md = "\
@magent hello

<magent-response>
Hello there!
</magent-response>
";
        let regions = find_magent_regions(md);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].start_line, 2);
        assert_eq!(regions[0].end_line, 4);
    }

    #[test]
    fn find_magent_regions__multiple_responses() {
        let md = "\
Text.

<magent-response>
First.
</magent-response>

<magent-response>
Second.
</magent-response>
";
        let regions = find_magent_regions(md);
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].start_line, 2);
        assert_eq!(regions[0].end_line, 4);
        assert_eq!(regions[1].start_line, 6);
        assert_eq!(regions[1].end_line, 8);
    }

    #[test]
    fn find_magent_regions__skips_fenced_code_blocks() {
        let md = "\
```
<magent-response>
Not a region.
</magent-response>
```
";
        let regions = find_magent_regions(md);
        assert!(regions.is_empty());
    }

    #[test]
    fn find_magent_regions__nested_response() {
        let md = "\
<magent-response>
<magent-response>
Nested.
</magent-response>
Outer.
</magent-response>
";
        let regions = find_magent_regions(md);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].start_line, 0);
        assert_eq!(regions[0].end_line, 5);
    }

    #[test]
    fn find_magent_regions__unclosed_response() {
        let md = "\
<magent-response>
Unclosed.
";
        let regions = find_magent_regions(md);
        assert!(regions.is_empty());
    }

    // -- strip_magent_blocks --------------------------------------------------

    #[test]
    fn strip_magent_blocks__no_magent_content() {
        let md = "# Hello\n\nA paragraph.\n";
        let result = strip_magent_blocks(md);
        assert_eq!(result, md);
    }

    #[test]
    fn strip_magent_blocks__removes_response_keeps_directive() {
        let md = "\
@magent hello

<magent-response>
Hello there!
</magent-response>
";
        let result = strip_magent_blocks(md);

        assert!(result.contains("@magent hello"));
        assert!(!result.contains("magent-response"));
        assert!(!result.contains("Hello there!"));
    }

    #[test]
    fn strip_magent_blocks__removes_nested_content() {
        let md = "\
<magent-response>
<magent-thinking>
Reasoning here.
</magent-thinking>
<magent-tool-call tool=\"search\">
<magent-input>query</magent-input>
</magent-tool-call>
The answer.
</magent-response>
";
        let result = strip_magent_blocks(md);

        assert!(!result.contains("magent-thinking"));
        assert!(!result.contains("magent-tool-call"));
        assert!(!result.contains("The answer."));
        assert!(result.trim().is_empty());
    }

    #[test]
    fn strip_magent_blocks__skips_fenced_code_blocks() {
        let md = "\
```
<magent-response>
This should not be stripped.
</magent-response>
```
";
        let result = strip_magent_blocks(md);

        assert!(result.contains("<magent-response>"));
        assert!(result.contains("This should not be stripped."));
    }

    #[test]
    fn strip_magent_blocks__multiple_responses() {
        let md = "\
Text before.

<magent-response>
First.
</magent-response>

<magent-response>
Second.
</magent-response>

Text after.
";
        let result = strip_magent_blocks(md);

        assert!(result.contains("Text before."));
        assert!(result.contains("Text after."));
        assert!(!result.contains("First."));
        assert!(!result.contains("Second."));
    }

    #[test]
    fn strip_magent_blocks__unclosed_response_preserved() {
        let md = "\
<magent-response>
Unclosed content.
";
        let result = strip_magent_blocks(md);

        assert!(result.contains("<magent-response>"));
        assert!(result.contains("Unclosed content."));
    }

    #[test]
    fn strip_magent_blocks__nested_response_tags() {
        let md = "\
<magent-response>
<magent-tool-result tool=\"search\">
<magent-response>
Nested.
</magent-response>
</magent-tool-result>
Outer text.
</magent-response>
";
        let result = strip_magent_blocks(md);

        assert!(!result.contains("Nested."));
        assert!(!result.contains("Outer text."));
        assert!(result.trim().is_empty());
    }

    #[test]
    fn strip_magent_blocks__preserves_surrounding_markdown() {
        let md = "\
# Title

Some text before.

<magent-response>
Answer.
</magent-response>

More text after.
";
        let result = strip_magent_blocks(md);

        assert!(result.starts_with("# Title\n"));
        assert!(result.contains("Some text before."));
        assert!(result.contains("More text after."));
        assert!(!result.contains("Answer."));
    }

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

    // -- accept_magent_edit ---------------------------------------------------

    #[test]
    fn accept_magent_edit__should_apply_replacement_and_update_status() {
        let doc = "\
# Notes

- [Rust](htps://rust-lang.org)

<magent-response>
Fixed the URL:
<magent-edit status=\"proposed\">
<magent-search>htps://rust-lang.org</magent-search>
<magent-replace>https://rust-lang.org</magent-replace>
</magent-edit>
</magent-response>
";
        let result = accept_magent_edit(doc, 0).expect("should succeed");

        // The document body should have the replacement applied.
        assert!(result.contains("- [Rust](https://rust-lang.org)"));
        // The edit block status should be updated.
        assert!(result.contains("status=\"accepted\""));
        assert!(!result.contains("status=\"proposed\""));
    }

    #[test]
    fn accept_magent_edit__should_select_by_index() {
        let doc = "\
AAA BBB

<magent-response>
<magent-edit status=\"proposed\">
<magent-search>AAA</magent-search>
<magent-replace>aaa</magent-replace>
</magent-edit>
<magent-edit status=\"proposed\">
<magent-search>BBB</magent-search>
<magent-replace>bbb</magent-replace>
</magent-edit>
</magent-response>
";
        let result = accept_magent_edit(doc, 1).expect("should succeed");

        // Only the second edit should be applied in the document body.
        assert!(result.contains("AAA bbb"));
        // Only the second edit block should be accepted.
        assert!(
            result.contains("status=\"proposed\""),
            "first edit stays proposed"
        );
        assert!(
            result.contains("status=\"accepted\""),
            "second edit is accepted"
        );
    }

    #[test]
    fn accept_magent_edit__should_reject_already_accepted_index() {
        let doc = "\
old-thing new-thing

<magent-response>
<magent-edit status=\"accepted\">
<magent-search>old-thing</magent-search>
<magent-replace>new-thing</magent-replace>
</magent-edit>
<magent-edit status=\"proposed\">
<magent-search>new-thing</magent-search>
<magent-replace>NEW-THING</magent-replace>
</magent-edit>
</magent-response>
";
        // Index 0 is the already-accepted edit — should return None.
        assert!(accept_magent_edit(doc, 0).is_none());

        // Index 1 is the proposed edit — should apply.
        let result = accept_magent_edit(doc, 1).expect("should succeed");
        assert!(result.contains("old-thing NEW-THING"));
    }

    #[test]
    fn accept_magent_edit__should_return_none_for_missing_index() {
        let doc = "\
text

<magent-response>
<magent-edit status=\"proposed\">
<magent-search>text</magent-search>
<magent-replace>TEXT</magent-replace>
</magent-edit>
</magent-response>
";
        assert!(accept_magent_edit(doc, 5).is_none());
    }

    #[test]
    fn accept_magent_edit__index_matches_rendering_order_not_proposed_only() {
        let doc = "\
AAA BBB

<magent-response>
<magent-edit status=\"accepted\">
<magent-search>old</magent-search>
<magent-replace>new</magent-replace>
</magent-edit>
<magent-edit status=\"proposed\">
<magent-search>BBB</magent-search>
<magent-replace>bbb</magent-replace>
</magent-edit>
</magent-response>
";
        // The proposed edit is the second block (data-edit-index="1" in rendered HTML).
        // Accepting index 1 should target it.
        let result = accept_magent_edit(doc, 1).expect("should succeed");
        assert!(result.contains("AAA bbb"));

        // Index 0 is the already-accepted edit — should return None.
        assert!(accept_magent_edit(doc, 0).is_none());
    }

    #[test]
    fn accept_magent_edit__body_match_after_edit_block_returns_none() {
        // Contrived: the search text appears only after the edit block.
        let doc = "\
<magent-response>
<magent-edit status=\"proposed\">
<magent-search>AFTER</magent-search>
<magent-replace>after</magent-replace>
</magent-edit>
</magent-response>

AFTER
";
        // The match is in the body but after the edit block. The function should
        // return None rather than risk corrupting the document.
        assert!(accept_magent_edit(doc, 0).is_none());
    }

    #[test]
    fn accept_magent_edit__should_return_none_for_malformed_edit() {
        let doc = "\
some text

<magent-response>
<magent-edit status=\"proposed\">
no search or replace tags here
</magent-edit>
</magent-response>
";
        assert!(accept_magent_edit(doc, 0).is_none());
    }

    #[test]
    fn accept_magent_edit__should_return_none_when_search_not_found() {
        let doc = "\
completely different content

<magent-response>
<magent-edit status=\"proposed\">
<magent-search>does not exist</magent-search>
<magent-replace>replacement</magent-replace>
</magent-edit>
</magent-response>
";
        assert!(accept_magent_edit(doc, 0).is_none());
    }
}
