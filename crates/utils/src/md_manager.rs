use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Modification {
    pub action: String,
    pub section: String,
    pub content: Option<Vec<String>>,
}

pub struct MarkdownManager {
    section_prefix: String,
    empty_placeholder: String,
}

impl MarkdownManager {
    pub fn new(section_prefix: &str, empty_placeholder: &str) -> Self {
        Self {
            section_prefix: section_prefix.to_string(),
            empty_placeholder: empty_placeholder.to_string(),
        }
    }

    pub fn default_sentence_splitter(&self, text: &str) -> Vec<String> {
        if text.trim().is_empty() {
            return vec![];
        }
        let re = Regex::new(r"([.!?。！？]+)").unwrap();
        let processed_text = re.replace_all(text, "$1\n");
        processed_text
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect()
    }

    pub fn ensure_file(
        &self,
        file_path: &str,
        default_sections: Option<Vec<&str>>,
        default_section_content: Option<HashMap<&str, Vec<&str>>>,
    ) {
        if Path::new(file_path).exists() {
            return;
        }

        if let Some(parent) = Path::new(file_path).parent() {
            let _ = fs::create_dir_all(parent);
        }

        let default_content = default_section_content.unwrap_or_default();
        let sections = default_sections.unwrap_or_else(|| default_content.keys().copied().collect());

        let mut parts = Vec::new();
        for section in sections {
            let default_lines = vec![self.empty_placeholder.as_str()];
            let lines = default_content.get(section).unwrap_or(&default_lines);
            parts.push(format!(
                "{}{}\n{}\n",
                self.section_prefix,
                section,
                lines.join("\n")
            ));
        }

        let mut final_text = parts.join("\n").trim().to_string();
        if !parts.is_empty() {
            final_text.push('\n');
        }
        let _ = fs::write(file_path, final_text);
    }

    pub fn read(&self, file_path: &str) -> String {
        fs::read_to_string(file_path).unwrap_or_default()
    }

    pub fn write(&self, file_path: &str, content: &str) {
        if let Some(parent) = Path::new(file_path).parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(file_path, content);
    }

    pub fn parse_sections(&self, markdown_text: &str) -> HashMap<String, Vec<String>> {
        let escaped_prefix = regex::escape(self.section_prefix.trim());
        let pattern = format!(
            r"(?ms)(?:^|\n){}\s*(.*?)\n(.*?)(?=\n{}\s*|\z)",
            escaped_prefix, escaped_prefix
        );
        let re = Regex::new(&pattern).unwrap();

        let mut result = HashMap::new();
        for caps in re.captures_iter(markdown_text) {
            let header = caps[1].trim().to_string();
            let content = caps[2].to_string();
            let lines: Vec<String> = content
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty() && l != self.empty_placeholder)
                .collect();
            result.insert(header, lines);
        }
        result
    }

    pub fn get_section_order(&self, markdown_text: &str) -> Vec<String> {
        let escaped_prefix = regex::escape(self.section_prefix.trim());
        let pattern = format!(r"(?m)(?:^|\n){}\s*(.*?)\n", escaped_prefix);
        let re = Regex::new(&pattern).unwrap();

        re.captures_iter(markdown_text)
            .map(|caps| caps[1].trim().to_string())
            .collect()
    }

    pub fn render_sections(
        &self,
        section_dict: &HashMap<String, Vec<String>>,
        section_order: Option<Vec<String>>,
    ) -> String {
        let ordered = section_order.unwrap_or_default();
        let mut extras: Vec<String> = section_dict
            .keys()
            .filter(|k| !ordered.contains(k))
            .cloned()
            .collect();

        let mut final_order = ordered;
        final_order.append(&mut extras);

        let mut parts = Vec::new();
        for section in final_order {
            if let Some(lines) = section_dict.get(&section) {
                let content = if lines.is_empty() {
                    self.empty_placeholder.clone()
                } else {
                    lines.join("\n")
                };
                parts.push(format!("{}{}\n{}\n", self.section_prefix, section, content));
            }
        }

        let mut final_text = parts.join("\n").trim().to_string();
        if !parts.is_empty() {
            final_text.push('\n');
        }
        final_text
    }

    pub fn apply_modifications(
        &self,
        section_dict: &HashMap<String, Vec<String>>,
        modifications: Vec<Modification>,
        split_sentences_on_replace: bool,
        create_missing_section: bool,
    ) -> HashMap<String, Vec<String>> {
        let mut result = section_dict.clone();

        for modif in modifications {
            let section = modif.section;
            if section.is_empty() {
                continue;
            }

            if !result.contains_key(&section)
                && create_missing_section
                && modif.action != "delete_section"
            {
                result.insert(section.clone(), Vec::new());
            }

            match modif.action.as_str() {
                "replace_section" => {
                    let mut lines = Vec::new();
                    if let Some(content) = modif.content {
                        for c in content {
                            if split_sentences_on_replace {
                                lines.extend(self.default_sentence_splitter(&c));
                            } else {
                                lines.push(c.trim().to_string());
                            }
                        }
                    }
                    result.insert(section, lines);
                }
                "append_lines" => {
                    if let Some(content) = modif.content {
                        let lines = result.entry(section).or_insert_with(Vec::new);
                        for c in content {
                            lines.push(c.trim().to_string());
                        }
                    }
                }
                "delete_lines_containing" => {
                    if let Some(content) = modif.content {
                        if let Some(keyword) = content.first() {
                            let keyword = keyword.trim();
                            if !keyword.is_empty() {
                                if let Some(lines) = result.get_mut(&section) {
                                    lines.retain(|line| !line.contains(keyword));
                                }
                            }
                        }
                    }
                }
                "delete_exact_lines" => {
                    if let Some(content) = modif.content {
                        let target_lines: HashSet<String> = content.into_iter().collect();
                        if let Some(lines) = result.get_mut(&section) {
                            lines.retain(|line| !target_lines.contains(line));
                        }
                    }
                }
                "clear_section" => {
                    result.insert(section, Vec::new());
                }
                "delete_section" => {
                    result.remove(&section);
                }
                _ => {}
            }
        }
        result
    }
}