use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use tree_sitter::{Language as TsLanguage, Node, Parser};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Language {
    Python,
    JavaScript,
    TypeScript,
    Rust,
    Go,
}

impl Language {
    pub fn as_str(self) -> &'static str {
        match self {
            Language::Python => "python",
            Language::JavaScript => "javascript",
            Language::TypeScript => "typescript",
            Language::Rust => "rust",
            Language::Go => "go",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeUnit {
    pub id: String,
    pub file_path: String,
    pub language: String,
    pub unit_type: String,
    pub name: String,
    pub signature: String,
    pub code: String,
    pub structured_text: String,
    pub start_line: usize,
    pub end_line: usize,
}

pub fn detect_language(path: &Path) -> Option<Language> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    match ext.as_str() {
        "py" => Some(Language::Python),
        "js" | "jsx" | "mjs" | "cjs" => Some(Language::JavaScript),
        "ts" | "tsx" => Some(Language::TypeScript),
        "rs" => Some(Language::Rust),
        "go" => Some(Language::Go),
        _ => None,
    }
}

pub fn parse_file(path: &Path, rel_path: &str) -> Result<Vec<CodeUnit>> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read source file: {}", path.display()))?;
    parse_source(&source, rel_path, detect_language(path))
}

pub fn parse_source(source: &str, rel_path: &str, lang: Option<Language>) -> Result<Vec<CodeUnit>> {
    let Some(lang) = lang else {
        return Ok(Vec::new());
    };

    let mut parser = Parser::new();
    let ts_lang = tree_sitter_language(lang);
    parser
        .set_language(&ts_lang)
        .context("failed to set tree-sitter language")?;

    let Some(tree) = parser.parse(source, None) else {
        return Ok(Vec::new());
    };

    let mut units = Vec::new();
    collect_units(tree.root_node(), source, rel_path, lang, None, &mut units);

    if units.is_empty() {
        let code = source.lines().take(200).collect::<Vec<_>>().join("\n");
        let signature = code.lines().find(|l| !l.trim().is_empty()).unwrap_or_default();
        units.push(build_unit(
            rel_path,
            lang,
            "file",
            rel_path,
            signature.trim(),
            &code,
            1,
            source.lines().count().max(1),
        ));
    }

    Ok(units)
}

fn tree_sitter_language(lang: Language) -> TsLanguage {
    match lang {
        Language::Python => tree_sitter_python::LANGUAGE.into(),
        Language::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
        Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        Language::Rust => tree_sitter_rust::LANGUAGE.into(),
        Language::Go => tree_sitter_go::LANGUAGE.into(),
    }
}

fn collect_units(
    node: Node,
    source: &str,
    rel_path: &str,
    lang: Language,
    parent_class: Option<String>,
    units: &mut Vec<CodeUnit>,
) {
    let kind = node.kind();

    if let Some(class_kind) = class_kind(kind, lang) {
        if let Some(unit) = extract_class_unit(node, source, rel_path, lang, class_kind) {
            let class_name = unit.name.clone();
            units.push(unit);

            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_units(
                    child,
                    source,
                    rel_path,
                    lang,
                    Some(class_name.clone()),
                    units,
                );
            }
            return;
        }
    }

    if let Some(function_kind) = function_kind(kind, lang) {
        if let Some(unit) = extract_function_unit(
            node,
            source,
            rel_path,
            lang,
            function_kind,
            parent_class.as_deref(),
        ) {
            units.push(unit);
        }
    } else if let Some(unit) = extract_arrow_function_unit(node, source, rel_path, lang) {
        units.push(unit);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_units(child, source, rel_path, lang, parent_class.clone(), units);
    }
}

fn class_kind(kind: &str, lang: Language) -> Option<&'static str> {
    match lang {
        Language::Python if kind == "class_definition" => Some("class"),
        Language::JavaScript | Language::TypeScript if kind == "class_declaration" => Some("class"),
        Language::TypeScript if kind == "interface_declaration" => Some("interface"),
        Language::TypeScript if kind == "type_alias_declaration" => Some("type"),
        Language::Rust if kind == "struct_item" => Some("struct"),
        Language::Rust if kind == "enum_item" => Some("enum"),
        Language::Rust if kind == "trait_item" => Some("trait"),
        Language::Rust if kind == "impl_item" => Some("impl"),
        Language::Go if kind == "type_declaration" => Some("type"),
        _ => None,
    }
}

fn function_kind(kind: &str, lang: Language) -> Option<&'static str> {
    match lang {
        Language::Python if kind == "function_definition" => Some("function"),
        Language::JavaScript | Language::TypeScript if kind == "function_declaration" => {
            Some("function")
        }
        Language::JavaScript | Language::TypeScript if kind == "method_definition" => {
            Some("method")
        }
        Language::Rust if kind == "function_item" => Some("function"),
        Language::Go if kind == "function_declaration" => Some("function"),
        Language::Go if kind == "method_declaration" => Some("method"),
        _ => None,
    }
}

fn extract_class_unit(
    node: Node,
    source: &str,
    rel_path: &str,
    lang: Language,
    unit_type: &str,
) -> Option<CodeUnit> {
    let name = extract_name(node, source).unwrap_or_else(|| infer_name(unit_type, node, source));
    let code = node_text(node, source)?;
    let signature = first_non_empty_line(&code);

    Some(build_unit(
        rel_path,
        lang,
        unit_type,
        &name,
        signature,
        &code,
        node.start_position().row + 1,
        node.end_position().row + 1,
    ))
}

fn extract_function_unit(
    node: Node,
    source: &str,
    rel_path: &str,
    lang: Language,
    detected_kind: &str,
    parent_class: Option<&str>,
) -> Option<CodeUnit> {
    let code = node_text(node, source)?;
    let signature = first_non_empty_line(&code);

    let mut unit_type = detected_kind.to_string();
    if parent_class.is_some() {
        unit_type = "method".to_string();
    }

    if lang == Language::Rust
        && node
            .parent()
            .map(|p| p.kind() == "declaration_list")
            .unwrap_or(false)
        && node
            .parent()
            .and_then(|p| p.parent())
            .map(|pp| pp.kind() == "impl_item")
            .unwrap_or(false)
    {
        unit_type = "method".to_string();
    }

    let name = extract_name(node, source)
        .or_else(|| extract_rust_fn_name(signature))
        .unwrap_or_else(|| infer_name(&unit_type, node, source));

    Some(build_unit(
        rel_path,
        lang,
        &unit_type,
        &name,
        signature,
        &code,
        node.start_position().row + 1,
        node.end_position().row + 1,
    ))
}

fn extract_arrow_function_unit(
    node: Node,
    source: &str,
    rel_path: &str,
    lang: Language,
) -> Option<CodeUnit> {
    if !matches!(lang, Language::JavaScript | Language::TypeScript) {
        return None;
    }
    if node.kind() != "variable_declarator" {
        return None;
    }

    let value = node.child_by_field_name("value")?;
    if value.kind() != "arrow_function" && value.kind() != "function" {
        return None;
    }

    let name = node
        .child_by_field_name("name")
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "anonymous_arrow".to_string());

    let code = node_text(node, source)?;
    let signature = first_non_empty_line(&code);

    Some(build_unit(
        rel_path,
        lang,
        "function",
        &name,
        signature,
        &code,
        node.start_position().row + 1,
        node.end_position().row + 1,
    ))
}

fn node_text(node: Node, source: &str) -> Option<String> {
    node.utf8_text(source.as_bytes())
        .ok()
        .map(|s| s.trim().to_string())
}

fn extract_name(node: Node, source: &str) -> Option<String> {
    let bytes = source.as_bytes();

    if let Some(name_node) = node.child_by_field_name("name") {
        if let Ok(name) = name_node.utf8_text(bytes) {
            let n = name.trim();
            if !n.is_empty() {
                return Some(n.to_string());
            }
        }
    }

    if let Some(prop_node) = node.child_by_field_name("property") {
        if let Ok(name) = prop_node.utf8_text(bytes) {
            let n = name.trim();
            if !n.is_empty() {
                return Some(n.to_string());
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "identifier" | "type_identifier" | "property_identifier") {
            if let Ok(name) = child.utf8_text(bytes) {
                let n = name.trim();
                if !n.is_empty() {
                    return Some(n.to_string());
                }
            }
        }
    }

    None
}

fn extract_rust_fn_name(signature: &str) -> Option<String> {
    let idx = signature.find("fn ")?;
    let rest = &signature[idx + 3..];
    let mut out = String::new();
    for ch in rest.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            break;
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn infer_name(unit_type: &str, node: Node, source: &str) -> String {
    if let Some(name) = extract_name(node, source) {
        return name;
    }

    let text = node_text(node, source).unwrap_or_default();
    let signature = first_non_empty_line(&text);
    let mut prev_is_keyword = false;

    for tok in signature
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .filter(|s| !s.is_empty())
    {
        if prev_is_keyword {
            return tok.to_string();
        }

        prev_is_keyword = matches!(tok, "fn" | "class" | "struct" | "trait" | "enum" | "interface");
    }

    unit_type.to_string()
}

fn first_non_empty_line(code: &str) -> &str {
    code.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
}

fn build_unit(
    file_path: &str,
    lang: Language,
    unit_type: &str,
    name: &str,
    signature: &str,
    code: &str,
    start_line: usize,
    end_line: usize,
) -> CodeUnit {
    let mut hasher = DefaultHasher::new();
    file_path.hash(&mut hasher);
    name.hash(&mut hasher);
    start_line.hash(&mut hasher);
    end_line.hash(&mut hasher);
    let id = format!("{:x}", hasher.finish());

    let structured_text = format!(
        "File: {file}\nLanguage: {lang}\nType: {unit_type}\nName: {name}\nLines: {start}-{end}\nSignature:\n{signature}\n\nCode:\n{code}",
        file = file_path,
        lang = lang.as_str(),
        unit_type = unit_type,
        name = name,
        start = start_line,
        end = end_line,
        signature = signature,
        code = code,
    );

    CodeUnit {
        id,
        file_path: file_path.to_string(),
        language: lang.as_str().to_string(),
        unit_type: unit_type.to_string(),
        name: name.to_string(),
        signature: signature.to_string(),
        code: code.to_string(),
        structured_text,
        start_line,
        end_line,
    }
}
