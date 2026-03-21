use super::{detect_line_ending, is_fence_line, split_line_ending};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BlockRange {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) kind: BlockKind,
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

#[derive(Debug)]
pub(crate) enum ReorderError {
    InvalidRange,
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

pub(crate) fn line_count(contents: &str) -> usize {
    split_lines_preserve(contents).len()
}

pub(crate) fn lines_for_display(contents: &str) -> Vec<String> {
    split_lines_preserve(contents)
        .into_iter()
        .map(|segment| segment.text)
        .collect()
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

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

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
}
