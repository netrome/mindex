use super::{detect_line_ending, is_fence_line, split_line_ending};
use std::collections::HashSet;

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

pub(super) struct TaskLineParts<'a> {
    pub(super) prefix: &'a str,
    pub(super) suffix: &'a str,
    pub(super) checked: bool,
}

pub(super) fn parse_task_line(line: &str) -> Option<TaskLineParts<'_>> {
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

pub(super) fn is_task_list_marker(line: &str) -> bool {
    line.trim() == "+"
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

fn is_username_char(byte: u8) -> bool {
    is_username_start(byte) || byte == b'-'
}

fn is_username_start(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

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
}
