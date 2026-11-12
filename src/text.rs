//! Deliberately small, loss-aware Mind Elixir plaintext subset.
use crate::{MindMapData, MindMapError, Node};

fn invalid(line: usize, message: &str) -> MindMapError {
    MindMapError::InvalidData {
        path: format!("line {line}"),
        message: message.into(),
    }
}
/// Parse two-space indented `- Topic` lines, `\\n` and `\\\\` escapes.
/// Multiple top-level entries acquire a synthetic `Root`. Styles, references,
/// arrows and summaries are rejected, rather than silently discarded.
pub fn from_plaintext(input: &str) -> Result<MindMapData, MindMapError> {
    let mut lines = Vec::<(usize, Node)>::new();
    for (number, line) in input.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let spaces = line.bytes().take_while(|b| *b == b' ').count();
        if spaces % 2 != 0 {
            return Err(invalid(number + 1, "Indentation must use pairs of spaces"));
        }
        let content = line
            .get(spaces..)
            .and_then(|s| s.strip_prefix("- "))
            .ok_or_else(|| invalid(number + 1, "Expected - Topic"))?;
        if content.starts_with(['>', '}']) || content.contains("[^") || content.contains(" {\"") {
            return Err(invalid(
                number + 1,
                "References, arrows, summaries and style suffixes are unsupported",
            ));
        }
        let mut topic = String::new();
        let mut chars = content.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => topic.push('\n'),
                    Some('\\') => topic.push('\\'),
                    _ => return Err(invalid(number + 1, "Unknown or incomplete escape")),
                }
            } else {
                topic.push(c);
            }
        }
        let depth = spaces / 2;
        if depth > 128
            || (lines.is_empty() && depth != 0)
            || lines
                .last()
                .is_some_and(|(previous, _)| depth > previous + 1)
        {
            return Err(invalid(number + 1, "Invalid indentation depth"));
        }
        lines.push((depth, Node::new(format!("text-{}", number + 1), topic)));
    }
    if lines.is_empty() {
        return Err(invalid(1, "No nodes"));
    }
    let mut stack = Vec::<(usize, Node)>::new();
    let mut roots = Vec::new();
    for (depth, node) in lines {
        while stack.last().is_some_and(|(d, _)| *d >= depth) {
            attach(&mut stack, &mut roots);
        }
        stack.push((depth, node));
    }
    while !stack.is_empty() {
        attach(&mut stack, &mut roots);
    }
    let root = if roots.len() == 1 {
        roots.pop().ok_or_else(|| invalid(1, "No root"))?
    } else {
        Node::new("text-root", "Root").with_children(roots)
    };
    let document = MindMapData::new(root);
    document.validate()?;
    Ok(document)
}
fn attach(stack: &mut Vec<(usize, Node)>, roots: &mut Vec<Node>) {
    if let Some((_, node)) = stack.pop() {
        if let Some((_, parent)) = stack.last_mut() {
            parent.children.push(node);
        } else {
            roots.push(node);
        }
    }
}
/// Serialize labels and tree order only. Reject rich persistent fields so callers
/// cannot accidentally treat this subset as a complete backup; use JSON instead.
pub fn to_plaintext(data: &MindMapData) -> Result<String, MindMapError> {
    data.validate()?;
    if data.meta.is_some() {
        return Err(MindMapError::InvalidOperation(
            "Plaintext cannot preserve document metadata; use JSON".into(),
        ));
    }
    let mut result = String::new();
    let mut pending = vec![(&data.node_data, 0)];
    while let Some((node, depth)) = pending.pop() {
        if node.style.is_some()
            || node.note.is_some()
            || node.image.is_some()
            || node.metadata.is_some()
            || node.branch_color.is_some()
        {
            return Err(MindMapError::InvalidOperation(
                "Plaintext cannot preserve style, note, image or metadata; use JSON".into(),
            ));
        }
        let topic = node.topic.replace('\\', "\\\\").replace('\n', "\\n");
        if topic.starts_with(['>', '}']) || topic.contains("[^") || topic.contains(" {\"") {
            return Err(MindMapError::InvalidOperation(
                "Label conflicts with reserved plaintext syntax".into(),
            ));
        }
        result.push_str(&format!("{}- {topic}\n", "  ".repeat(depth)));
        pending.extend(node.children.iter().rev().map(|child| (child, depth + 1)));
    }
    Ok(result)
}
