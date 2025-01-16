// Use similar tags as indicatif
#[derive(Debug)]
pub(crate) enum TemplatePart {
    Newline,
    Message,
    /// HH:MM:SS
    Elapsed,
    /// xx B/KiB/MiB/GiB...
    Bytes,
    Pos,
    TotalBytes,
    Total,
    /// xx B/s, xx KiB/s...
    BytesPerSecond,
    /// HH:MM:SS
    Eta,
    Text(String),
}

#[derive(Debug)]
pub(crate) struct Template {
    pub(crate) parts: Vec<TemplatePart>,
}

impl Template {
    pub(crate) fn new(template: &str) -> Self {
        enum Fragment {
            Text(String),
            Tag(String),
        }
        use Fragment::*;
        let mut fragments = Vec::new();
        let mut current_text = String::new();
        let mut chars = template.chars().peekable();
        while let Some(ch) = chars.next() {
            match ch {
                '{' => {
                    if let Some('{') = chars.peek() {
                        chars.next();
                        current_text.push('{');
                    } else {
                        if !current_text.is_empty() {
                            fragments.push(Text(current_text.clone()));
                            current_text.clear();
                        }
                        let mut tag_content = String::new();
                        let mut found_closing_brace = false;
                        for ch2 in chars.by_ref() {
                            if ch2 == '}' {
                                found_closing_brace = true;
                                break;
                            }
                            tag_content.push(ch2);
                        }

                        if found_closing_brace {
                            fragments.push(Tag(tag_content));
                        } else {
                            current_text.push('{');
                            current_text.push_str(&tag_content);
                        }
                    }
                }
                '}' => {
                    if let Some('}') = chars.peek() {
                        chars.next();
                        current_text.push('}');
                    } else {
                        current_text.push('}');
                    }
                }
                _ => {
                    current_text.push(ch);
                }
            }
        }
        if !current_text.is_empty() {
            fragments.push(Text(current_text));
        }

        let mut results = Vec::new();
        fn push_text(results: &mut Vec<TemplatePart>, text: &str) {
            let texts = text.split('\n');
            for (j, text) in texts.enumerate() {
                if j > 0 {
                    results.push(TemplatePart::Newline);
                }
                results.push(TemplatePart::Text(text.to_string()));
            }
        }
        for i in fragments {
            match i {
                Text(text) => {
                    push_text(&mut results, &text);
                }
                Tag(tag) => match tag.as_str() {
                    // indicatif tag
                    "msg" => results.push(TemplatePart::Message),
                    "message" => results.push(TemplatePart::Message),
                    "elapsed" => results.push(TemplatePart::Elapsed),
                    // indicatif tag
                    "elapsed_precise" => results.push(TemplatePart::Elapsed),
                    "bytes" => results.push(TemplatePart::Bytes),
                    "pos" => results.push(TemplatePart::Pos),
                    "total_bytes" => results.push(TemplatePart::TotalBytes),
                    "total" => results.push(TemplatePart::Total),
                    "bytes_per_second" => results.push(TemplatePart::BytesPerSecond),
                    // indicatif tag
                    "bytes_per_sec" => results.push(TemplatePart::BytesPerSecond),
                    "eta" => results.push(TemplatePart::Eta),
                    _ => {
                        push_text(&mut results, &format!("{{{tag}}}"));
                    }
                },
            }
        }

        Template { parts: results }
    }
}
