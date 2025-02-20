/// Get the tree of ast
use crate::utils::treehelper::point_to_position;
use lsp_types::{DocumentSymbol, DocumentSymbolResponse, MessageType, SymbolKind};
use tower_lsp::Client;

const COMMAND_KEYWORDS: [&str; 5] = [
    "set",
    "option",
    "project",
    "target_link_libraries",
    "target_include_directories",
];
pub async fn getast(client: &Client, context: &str) -> Option<DocumentSymbolResponse> {
    let line = context.lines().count();
    if line > 10000 {
        client
            .log_message(MessageType::INFO, "use simple ast")
            .await;
    }
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(tree_sitter_cmake::language()).unwrap();
    let thetree = parse.parse(context, None);
    let tree = thetree.unwrap();
    getsubast(tree.root_node(), context, line > 10000).map(DocumentSymbolResponse::Nested)
}
#[allow(deprecated)]
fn getsubast(input: tree_sitter::Node, source: &str, simple: bool) -> Option<Vec<DocumentSymbol>> {
    let newsource: Vec<&str> = source.lines().collect();
    let mut course = input.walk();
    let mut asts: Vec<DocumentSymbol> = vec![];
    for child in input.children(&mut course) {
        match child.kind() {
            "function_def" => {
                let h = child.start_position().row;
                let ids = child.child(0).unwrap();
                let ids = ids.child(2).unwrap();
                let x = ids.start_position().column;
                let y = ids.end_position().column;
                let name = &newsource[h][x..y];
                asts.push(DocumentSymbol {
                    name: name.to_string(),
                    detail: None,
                    kind: SymbolKind::FUNCTION,
                    tags: None,
                    deprecated: None,
                    range: lsp_types::Range {
                        start: lsp_types::Position {
                            line: child.start_position().row as u32,
                            character: child.start_position().column as u32,
                        },
                        end: lsp_types::Position {
                            line: child.end_position().row as u32,
                            character: child.end_position().column as u32,
                        },
                    },
                    selection_range: lsp_types::Range {
                        start: lsp_types::Position {
                            line: child.start_position().row as u32,
                            character: child.start_position().column as u32,
                        },
                        end: lsp_types::Position {
                            line: child.end_position().row as u32,
                            character: child.end_position().column as u32,
                        },
                    },
                    children: if simple {
                        None
                    } else {
                        getsubast(child, source, simple)
                    },
                });
            }
            "macro_def" => {
                let h = child.start_position().row;
                let ids = child.child(0).unwrap();
                let ids = ids.child(2).unwrap();
                let x = ids.start_position().column;
                let y = ids.end_position().column;
                let name = &newsource[h][x..y];
                asts.push(DocumentSymbol {
                    name: name.to_string(),
                    detail: None,
                    kind: SymbolKind::FUNCTION,
                    tags: None,
                    deprecated: None,
                    range: lsp_types::Range {
                        start: lsp_types::Position {
                            line: child.start_position().row as u32,
                            character: child.start_position().column as u32,
                        },
                        end: lsp_types::Position {
                            line: child.end_position().row as u32,
                            character: child.end_position().column as u32,
                        },
                    },
                    selection_range: lsp_types::Range {
                        start: lsp_types::Position {
                            line: child.start_position().row as u32,
                            character: child.start_position().column as u32,
                        },
                        end: lsp_types::Position {
                            line: child.end_position().row as u32,
                            character: child.end_position().column as u32,
                        },
                    },
                    children: if simple {
                        None
                    } else {
                        getsubast(child, source, simple)
                    },
                });
            }
            "if_condition" | "foreach_loop" => {
                asts.push(DocumentSymbol {
                    name: "Closure".to_string(),
                    detail: None,
                    kind: SymbolKind::NAMESPACE,
                    tags: None,
                    deprecated: None,
                    range: lsp_types::Range {
                        start: lsp_types::Position {
                            line: child.start_position().row as u32,
                            character: child.start_position().column as u32,
                        },
                        end: lsp_types::Position {
                            line: child.end_position().row as u32,
                            character: child.end_position().column as u32,
                        },
                    },
                    selection_range: lsp_types::Range {
                        start: lsp_types::Position {
                            line: child.start_position().row as u32,
                            character: child.start_position().column as u32,
                        },
                        end: lsp_types::Position {
                            line: child.end_position().row as u32,
                            character: child.end_position().column as u32,
                        },
                    },
                    children: if simple {
                        None
                    } else {
                        getsubast(child, source, simple)
                    },
                });
            }
            "normal_command" => {
                let start = point_to_position(child.start_position());
                let end = point_to_position(child.end_position());
                let h = child.start_position().row;
                let ids = child.child(0).unwrap();
                //let ids = ids.child(2).unwrap();
                let x = ids.start_position().column;
                let y = ids.end_position().column;
                let command_name = &newsource[h][x..y];
                if COMMAND_KEYWORDS.contains(&command_name.to_lowercase().as_str()) {
                    let ids = child.child(2).unwrap();
                    if ids.start_position().row == ids.end_position().row {
                        let h = ids.start_position().row;
                        let x = ids.start_position().column;
                        let y = ids.end_position().column;
                        if x != y {
                            let varname = &newsource[h][x..y];
                            asts.push(DocumentSymbol {
                                name: format!("{command_name}: {varname}"),
                                detail: None,
                                kind: SymbolKind::VARIABLE,
                                tags: None,
                                deprecated: None,
                                range: lsp_types::Range { start, end },
                                selection_range: lsp_types::Range {
                                    start: lsp_types::Position {
                                        line: h as u32,
                                        character: x as u32,
                                    },
                                    end: lsp_types::Position {
                                        line: h as u32,
                                        character: y as u32,
                                    },
                                },
                                children: None,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if asts.is_empty() {
        None
    } else {
        Some(asts)
    }
}
