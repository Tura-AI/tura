use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use walkdir::WalkDir;

#[derive(Serialize)]
struct Inventory {
    schema: &'static str,
    tests: Vec<TestRow>,
    fields: Vec<FieldRow>,
    database_columns: Vec<DatabaseColumnRow>,
}

#[derive(Serialize)]
struct TestRow {
    test_file: String,
    test_name: String,
    public_boundary: Vec<String>,
    observable_output: Vec<String>,
    failure_condition: String,
    source_text_dependency: bool,
    hidden_skip: bool,
    replacement_test: Option<String>,
    delete_condition: String,
}

#[derive(Serialize)]
struct FieldRow {
    entity: String,
    field_or_variant: String,
    definition: String,
    production_references: Vec<String>,
    test_references: Vec<String>,
    retention_reason: String,
}

#[derive(Serialize)]
struct DatabaseColumnRow {
    table: String,
    column: String,
    schema_definition: String,
    production_references: Vec<String>,
    test_references: Vec<String>,
    retention_reason: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or(std::env::current_dir()?);
    let files = rust_files(&repo);
    let mut parsed = Vec::new();
    for path in &files {
        let source = fs::read_to_string(path)?;
        if let Ok(file) = syn::parse_file(&source) {
            parsed.push((path.clone(), source, file));
        }
    }
    let tests = collect_tests(&repo, &parsed);
    let fields = collect_fields(&repo, &parsed);
    let database_columns = collect_database_columns(&repo, &parsed);
    println!(
        "{}",
        serde_json::to_string_pretty(&Inventory {
            schema: "tura.runtime-session-phase0-inventory.v1",
            tests,
            fields,
            database_columns,
        })?
    );
    Ok(())
}

fn rust_files(repo: &Path) -> Vec<PathBuf> {
    let mut files = [
        "crates/runtime",
        "crates/gateway",
        "crates/router",
        "crates/session_log",
        "crates/lifecycle",
        "tests",
    ]
    .iter()
    .flat_map(|root| {
        WalkDir::new(repo.join(root))
            .into_iter()
            .filter_entry(|entry| {
                !matches!(
                    entry.file_name().to_str(),
                    Some("target" | ".git" | "node_modules")
                )
            })
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.file_type().is_file()
                    && entry.path().extension().and_then(|ext| ext.to_str()) == Some("rs")
            })
            .map(|entry| entry.into_path())
            .collect::<Vec<_>>()
    })
    .collect::<Vec<_>>();
    files.sort();
    files
}

fn collect_tests(repo: &Path, parsed: &[(PathBuf, String, syn::File)]) -> Vec<TestRow> {
    let mut rows = Vec::new();
    for (path, _source, file) in parsed {
        let relative = relative(repo, path);
        collect_test_items(&file.items, &relative, &mut rows);
    }
    rows.sort_by(|a, b| (&a.test_file, &a.test_name).cmp(&(&b.test_file, &b.test_name)));
    rows
}

fn collect_test_items(items: &[syn::Item], relative: &str, rows: &mut Vec<TestRow>) {
    for item in items {
        if let syn::Item::Mod(module) = item {
            if let Some((_, nested)) = &module.content {
                collect_test_items(nested, relative, rows);
            }
            continue;
        }
        let syn::Item::Fn(function) = item else {
            continue;
        };
        let is_test = function.attrs.iter().any(|attr| {
            attr.path().is_ident("test") || path_ends_with(attr.path(), &["tokio", "test"])
        });
        if !is_test {
            continue;
        }
        let mut visitor = FunctionVisitor::default();
        visitor.visit_block(&function.block);
        let ignored = function
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("ignore"));
        let source_text_dependency = visitor
            .calls
            .iter()
            .any(|call| matches!(call.as_str(), "read_to_string" | "include_str" | "read"))
            && visitor.string_literals.iter().any(|text| {
                let literal = text
                    .trim_matches(|character: char| character == '"' || character.is_whitespace());
                literal.ends_with(".rs")
                    || literal.ends_with(".ts")
                    || literal.ends_with(".tsx")
            });
        let replacement = source_text_dependency.then(|| "required_before_deletion".to_string());
        rows.push(TestRow {
            test_file: relative.to_string(),
            test_name: function.sig.ident.to_string(),
            public_boundary: visitor.calls.into_iter().collect(),
            observable_output: visitor.assertions.into_iter().collect(),
            failure_condition: if ignored {
                "explicit_ignore_requires_documented_activation".to_string()
            } else if visitor.assertion_count == 0 {
                "panic_or_result_error_only; mutation_required".to_string()
            } else {
                "assertion_or_result_error".to_string()
            },
            source_text_dependency,
            hidden_skip: ignored || visitor.hidden_success_return,
            replacement_test: replacement,
            delete_condition: if source_text_dependency {
                "delete_after_behavioral_replacement_fails_on_mutation".to_string()
            } else {
                "retain_while_observable_behavior_is_current".to_string()
            },
        });
    }
}

fn collect_fields(repo: &Path, parsed: &[(PathBuf, String, syn::File)]) -> Vec<FieldRow> {
    let targets = [
        "RuntimeManagement",
        "SessionManagement",
        "RuntimeState",
        "SessionState",
        "RuntimeTurnState",
    ];
    let mut declarations = Vec::new();
    for (path, _source, file) in parsed {
        let relative = relative(repo, path);
        for item in &file.items {
            match item {
                syn::Item::Struct(item) if targets.contains(&item.ident.to_string().as_str()) => {
                    for field in &item.fields {
                        if let Some(ident) = &field.ident {
                            declarations.push((
                                item.ident.to_string(),
                                ident.to_string(),
                                format!("{relative}:{}", line_of(field)),
                            ));
                        }
                    }
                }
                syn::Item::Enum(item) if targets.contains(&item.ident.to_string().as_str()) => {
                    for variant in &item.variants {
                        declarations.push((
                            item.ident.to_string(),
                            variant.ident.to_string(),
                            format!("{relative}:{}", line_of(variant)),
                        ));
                    }
                }
                _ => {}
            }
        }
    }
    declarations
        .into_iter()
        .map(|(entity, name, definition)| {
            let (production_references, test_references) =
                references(repo, parsed, &name, &definition);
            let retention_reason = evidence_reason(&production_references, &test_references);
            FieldRow {
                entity,
                field_or_variant: name,
                definition,
                production_references,
                test_references,
                retention_reason,
            }
        })
        .collect()
}

fn collect_database_columns(
    repo: &Path,
    parsed: &[(PathBuf, String, syn::File)],
) -> Vec<DatabaseColumnRow> {
    let connection = repo.join("crates/session_log/src/store/connection.rs");
    let Ok(source) = fs::read_to_string(&connection) else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    let mut table = None::<String>;
    for (index, raw) in source.lines().enumerate() {
        let line = raw.trim().trim_matches('"').trim();
        if let Some(rest) = line.strip_prefix("CREATE TABLE IF NOT EXISTS ") {
            table = Some(rest.trim_end_matches(" (").to_string());
            continue;
        }
        if line.starts_with(");") || line == ");" {
            table = None;
            continue;
        }
        let Some(table_name) = table.clone() else {
            continue;
        };
        let Some(column) = line.split_whitespace().next() else {
            continue;
        };
        if column.is_empty() || matches!(column, "FOREIGN" | "PRIMARY" | "UNIQUE") {
            continue;
        }
        let column = column.trim_end_matches(',').to_string();
        let definition = format!("crates/session_log/src/store/connection.rs:{}", index + 1);
        let (production_references, test_references) =
            references(repo, parsed, &column, &definition);
        let retention_reason = evidence_reason(&production_references, &test_references);
        rows.push(DatabaseColumnRow {
            table: table_name,
            column,
            schema_definition: definition,
            production_references,
            test_references,
            retention_reason,
        });
    }
    rows
}

fn references(
    repo: &Path,
    parsed: &[(PathBuf, String, syn::File)],
    needle: &str,
    definition: &str,
) -> (Vec<String>, Vec<String>) {
    let mut production = Vec::new();
    let mut tests = Vec::new();
    for (path, source, _file) in parsed {
        let relative = relative(repo, path);
        for (index, line) in source.lines().enumerate() {
            let location = format!("{relative}:{}", index + 1);
            if location == definition || !contains_identifier(line, needle) {
                continue;
            }
            if relative.contains("/tests/")
                || relative.starts_with("tests/")
                || relative.ends_with("_test.rs")
            {
                tests.push(location);
            } else {
                production.push(location);
            }
        }
    }
    production.truncate(40);
    tests.truncate(40);
    (production, tests)
}

fn evidence_reason(production: &[String], tests: &[String]) -> String {
    match (production.is_empty(), tests.is_empty()) {
        (false, false) => "current_production_and_behavior_test_evidence".to_string(),
        (false, true) => "missing_behavior_assertion; phase6_delete_or_test".to_string(),
        (true, false) => "test_only_or_dead_production_field; phase6_delete_candidate".to_string(),
        (true, true) => "no_current_evidence; delete_candidate".to_string(),
    }
}

fn contains_identifier(line: &str, needle: &str) -> bool {
    line.match_indices(needle).any(|(index, _)| {
        let before = line[..index].chars().next_back();
        let after = line[index + needle.len()..].chars().next();
        !before.is_some_and(is_ident) && !after.is_some_and(is_ident)
    })
}

fn is_ident(value: char) -> bool {
    value == '_' || value.is_ascii_alphanumeric()
}

#[derive(Default)]
struct FunctionVisitor {
    calls: BTreeSet<String>,
    assertions: BTreeSet<String>,
    string_literals: BTreeSet<String>,
    assertion_count: usize,
    hidden_success_return: bool,
}

impl<'ast> Visit<'ast> for FunctionVisitor {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = node.func.as_ref() {
            if let Some(segment) = path.path.segments.last() {
                self.calls.insert(segment.ident.to_string());
            }
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        self.calls.insert(node.method.to_string());
        visit::visit_expr_method_call(self, node);
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if let Some(segment) = node.path.segments.last() {
            let name = segment.ident.to_string();
            if name.starts_with("assert") || name == "panic" || name == "bail" {
                self.assertions.insert(name.clone());
                self.assertion_count += 1;
            }
            if name == "include_str" {
                self.calls.insert(name);
                self.string_literals.insert(node.tokens.to_string());
            }
        }
        visit::visit_macro(self, node);
    }

    fn visit_expr_return(&mut self, node: &'ast syn::ExprReturn) {
        if let Some(syn::Expr::Call(call)) = node.expr.as_deref() {
            if let syn::Expr::Path(path) = call.func.as_ref() {
                self.hidden_success_return |= path
                    .path
                    .segments
                    .last()
                    .is_some_and(|segment| segment.ident == "Ok");
            }
        }
        visit::visit_expr_return(self, node);
    }

    fn visit_lit_str(&mut self, node: &'ast syn::LitStr) {
        self.string_literals.insert(node.value());
    }
}

fn path_ends_with(path: &syn::Path, expected: &[&str]) -> bool {
    let actual = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    actual.len() >= expected.len()
        && actual[actual.len() - expected.len()..]
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual == expected)
}

fn relative(repo: &Path, path: &Path) -> String {
    path.strip_prefix(repo)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn line_of<T: Spanned>(value: &T) -> usize {
    value.span().start().line
}
