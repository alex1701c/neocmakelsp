// todo compelete type
mod buildin;
mod findpackage;
mod includescanner;
use crate::utils::treehelper::{get_pos_type, PositionType};
use crate::CompletionResponse;
use buildin::{BUILDIN_COMMAND, BUILDIN_MODULE, BUILDIN_VARIABLE};
use lsp_types::{CompletionItem, CompletionItemKind, MessageType, Position};
use std::path::{Path, PathBuf};
pub fn rst_doc_read(doc: String, filename: &str) -> Vec<CompletionItem> {
    doc.lines()
        .filter(|line| line.starts_with(".. command:: "))
        .map(|line| &line[13..])
        .map(|line| format!("{line}()"))
        .map(|line| CompletionItem {
            label: line,
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(format!("defined command from {filename}\n{doc}")),
            ..Default::default()
        })
        .collect()
}

/// get the complet messages
pub async fn getcomplete(
    source: &str,
    location: Position,
    client: &tower_lsp::Client,
    local_path: &str,
) -> Option<CompletionResponse> {
    //let mut course2 = course.clone();
    //let mut hasid = false;
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(tree_sitter_cmake::language()).unwrap();
    let thetree = parse.parse(source, None);
    let tree = thetree.unwrap();
    let mut complete: Vec<CompletionItem> = vec![];
    let postype = get_pos_type(location, tree.root_node(), source, PositionType::NotFind);
    match postype {
        PositionType::Variable | PositionType::TargetLink | PositionType::TargetInclude => {
            if let Some(mut message) = getsubcomplete(
                tree.root_node(),
                source,
                Path::new(local_path),
                postype,
                Some(location),
            ) {
                complete.append(&mut message);
            }

            if let Ok(messages) = &*BUILDIN_COMMAND {
                complete.append(&mut messages.clone());
            }
            if let Ok(messages) = &*BUILDIN_VARIABLE {
                complete.append(&mut messages.clone());
            }
        }
        PositionType::FindPackage => {
            complete.append(&mut findpackage::CMAKE_SOURCE.clone());
        }
        #[cfg(unix)]
        PositionType::FindPkgConfig => {
            complete.append(&mut findpackage::PKGCONFIG_SOURCE.clone());
        }
        PositionType::Include => {
            if let Ok(messages) = &*BUILDIN_MODULE {
                complete.append(&mut messages.clone());
            }
        }
        _ => {}
    }

    if complete.is_empty() {
        client.log_message(MessageType::INFO, "Empty").await;
        None
    } else {
        Some(CompletionResponse::Array(complete))
    }
}
/// get the variable from the loop
/// use position to make only can complete which has show before
fn getsubcomplete(
    input: tree_sitter::Node,
    source: &str,
    local_path: &Path,
    postype: PositionType,
    location: Option<Position>,
) -> Option<Vec<CompletionItem>> {
    if let Some(location) = location {
        if input.start_position().row as u32 > location.line {
            return None;
        }
    }
    let newsource: Vec<&str> = source.lines().collect();
    let mut course = input.walk();
    //let mut course2 = course.clone();
    //let mut hasid = false;
    let mut complete: Vec<CompletionItem> = vec![];
    for child in input.children(&mut course) {
        if let Some(location) = location {
            if child.start_position().row as u32 > location.line {
                // if this child is below row, then break all loop
                break;
            }
        }
        match child.kind() {
            "bracket_comment" => {
                let start_y = child.start_position().row;
                let end_y = child.end_position().row;
                let mut output = String::new();
                for item in newsource.iter().take(end_y).skip(start_y + 1) {
                    output.push_str(&format!("{item}\n"));
                }
                complete.append(&mut rst_doc_read(
                    output,
                    local_path.file_name().unwrap().to_str().unwrap(),
                ));
            }
            "function_def" => {
                let h = child.start_position().row;
                let ids = child.child(0).unwrap();
                let ids = ids.child(2).unwrap();
                let x = ids.start_position().column;
                let y = ids.end_position().column;
                let name = &newsource[h][x..y];
                complete.push(CompletionItem {
                    label: format!("{name}()"),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some(format!(
                        "defined function\nfrom: {}",
                        local_path.file_name().unwrap().to_str().unwrap()
                    )),
                    ..Default::default()
                });
            }
            "macro_def" => {
                let h = child.start_position().row;
                let ids = child.child(0).unwrap();
                let ids = ids.child(2).unwrap();
                let x = ids.start_position().column;
                let y = ids.end_position().column;
                let name = &newsource[h][x..y];
                complete.push(CompletionItem {
                    label: format!("{name}()"),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some(format!(
                        "defined function\nfrom: {}",
                        local_path.file_name().unwrap().to_str().unwrap()
                    )),
                    ..Default::default()
                });
            }
            "if_condition" | "foreach_loop" => {
                if let Some(mut message) =
                    getsubcomplete(child, source, local_path, postype, location)
                {
                    complete.append(&mut message);
                }
            }
            "normal_command" => {
                let h = child.start_position().row;
                let ids = child.child(0).unwrap();
                //let ids = ids.child(2).unwrap();
                let x = ids.start_position().column;
                let y = ids.end_position().column;
                let name = newsource[h][x..y].to_lowercase();
                if name == "include" && child.child_count() >= 3 {
                    let ids = child.child(2).unwrap();
                    if ids.start_position().row == ids.end_position().row {
                        let h = ids.start_position().row;
                        let x = ids.start_position().column;
                        let y = ids.end_position().column;
                        let name = &newsource[h][x..y];
                        let subpath = {
                            if name.split('.').count() != 1 {
                                local_path.parent().unwrap().join(name)
                            } else {
                                Path::new(&format!("/usr/share/cmake/Modules/{name}.cmake"))
                                    .to_path_buf()
                            }
                        };
                        if let Ok(true) = cmake_try_exists(&subpath) {
                            if let Some(mut comps) =
                                includescanner::scanner_include_complete(&subpath, postype)
                            {
                                complete.append(&mut comps);
                            }
                        }
                    }
                } else if name == "mark_as_advanced" {
                    let mut advancedwalk = child.walk();
                    for identifier in child.children(&mut advancedwalk) {
                        if identifier.kind() == "argument"
                            && identifier.start_position().row == identifier.end_position().row
                        {
                            let startx = identifier.start_position().column;
                            let endx = identifier.end_position().column;
                            let row = identifier.start_position().row;
                            let variable = &newsource[row][startx..endx];
                            complete.push(CompletionItem {
                                label: variable.to_string(),
                                kind: Some(CompletionItemKind::VARIABLE),
                                detail: Some(format!(
                                    "defined var\nfrom: {}",
                                    local_path.file_name().unwrap().to_str().unwrap()
                                )),
                                ..Default::default()
                            });
                        }
                    }
                } else {
                    match postype {
                        PositionType::TargetLink | PositionType::TargetInclude => {
                            if name == "set" || name == "option" {
                                let ids = child.child(2).unwrap();
                                if ids.start_position().row == ids.end_position().row {
                                    let h = ids.start_position().row;
                                    let x = ids.start_position().column;
                                    let y = ids.end_position().column;
                                    let name = &newsource[h][x..y];
                                    complete.push(CompletionItem {
                                        label: name.to_string(),
                                        kind: Some(CompletionItemKind::VALUE),
                                        detail: Some(format!(
                                            "defined variable\nfrom: {}",
                                            local_path.file_name().unwrap().to_str().unwrap()
                                        )),
                                        ..Default::default()
                                    });
                                }
                            }
                            if name == "find_package" && child.child_count() >= 3 {
                                let ids = child.child(2).unwrap();
                                //let ids = ids.child(2).unwrap();
                                let x = ids.start_position().column;
                                let y = ids.end_position().column;
                                let package_name = &newsource[h][x..y];
                                let components_packages = {
                                    if child.child_count() >= 5 {
                                        let mut support_commponent = false;
                                        let count = child.child_count();
                                        let mut components_packages = Vec::new();
                                        for index in 3..count - 1 {
                                            let ids = child.child(index).unwrap();
                                            //let ids = ids.child(2).unwrap();
                                            let x = ids.start_position().column;
                                            let y = ids.end_position().column;
                                            let h = ids.start_position().row;
                                            let component = &newsource[h][x..y];
                                            if component == "COMPONENTS" {
                                                support_commponent = true;
                                            } else if component != "REQUIRED" {
                                                components_packages
                                                    .push(format!("{package_name}::{component}"));
                                            }
                                        }
                                        if support_commponent {
                                            Some(components_packages)
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                };
                                // mordern cmake like Qt5::Core
                                if let Some(components) = components_packages {
                                    for component in components {
                                        if let PositionType::TargetLink = postype {
                                            complete.push(CompletionItem {
                                                label: component,
                                                kind: Some(CompletionItemKind::VARIABLE),
                                                detail: Some(format!(
                                                    "package from: {package_name}",
                                                )),
                                                ..Default::default()
                                            });
                                        } else {
                                            complete.push(CompletionItem {
                                                label: component,
                                                kind: Some(CompletionItemKind::VARIABLE),
                                                detail: Some(format!(
                                                    "package from: {package_name}",
                                                )),
                                                ..Default::default()
                                            });
                                        }
                                    }
                                } else if let PositionType::TargetLink = postype {
                                    complete.push(CompletionItem {
                                        label: format!("{package_name}_LIBRARIES"),
                                        kind: Some(CompletionItemKind::VARIABLE),
                                        detail: Some(format!("package: {package_name}",)),
                                        ..Default::default()
                                    });
                                } else {
                                    complete.push(CompletionItem {
                                        label: format!("{package_name}_INCLUDE_DIRS"),
                                        kind: Some(CompletionItemKind::VARIABLE),
                                        detail: Some(format!("package: {package_name}",)),
                                        ..Default::default()
                                    });
                                }
                            }
                            #[cfg(unix)]
                            if name == "pkg_check_modules" && child.child_count() >= 3 {
                                let ids = child.child(2).unwrap();
                                //let ids = ids.child(2).unwrap();
                                let x = ids.start_position().column;
                                let y = ids.end_position().column;
                                let package_name = &newsource[h][x..y];
                                let modernpkgconfig = {
                                    if child.child_count() >= 5 {
                                        let ids = child.child(3).unwrap();
                                        //let ids = ids.child(2).unwrap();
                                        let x = ids.start_position().column;
                                        let y = ids.end_position().column;
                                        let atom = &newsource[h][x..y];
                                        if atom != "REQUIRED" {
                                            false
                                        } else {
                                            let ids = child.child(4).unwrap();
                                            //let ids = ids.child(2).unwrap();
                                            let x = ids.start_position().column;
                                            let y = ids.end_position().column;
                                            let atom = &newsource[h][x..y];
                                            atom == "IMPORTED_TARGET"
                                        }
                                    } else {
                                        false
                                    }
                                };
                                if modernpkgconfig {
                                    if let PositionType::TargetLink = postype {
                                        complete.push(CompletionItem {
                                            label: format!("PkgConfig::{package_name}"),
                                            kind: Some(CompletionItemKind::VARIABLE),
                                            detail: Some(format!("package: {package_name}",)),
                                            ..Default::default()
                                        });
                                    }
                                } else if let PositionType::TargetLink = postype {
                                    complete.push(CompletionItem {
                                        label: format!("{package_name}_LIBRARIES"),
                                        kind: Some(CompletionItemKind::VARIABLE),
                                        detail: Some(format!("package: {package_name}",)),
                                        ..Default::default()
                                    });
                                } else {
                                    complete.push(CompletionItem {
                                        label: format!("{package_name}_INCLUDE_DIRS"),
                                        kind: Some(CompletionItemKind::VARIABLE),
                                        detail: Some(format!("package: {package_name}",)),
                                        ..Default::default()
                                    });
                                }
                            }
                        }
                        PositionType::Variable => {
                            if name == "set" || name == "option" {
                                let ids = child.child(2).unwrap();
                                if ids.start_position().row == ids.end_position().row {
                                    let h = ids.start_position().row;
                                    let x = ids.start_position().column;
                                    let y = ids.end_position().column;
                                    let name = &newsource[h][x..y];
                                    complete.push(CompletionItem {
                                        label: name.to_string(),
                                        kind: Some(CompletionItemKind::VALUE),
                                        detail: Some(format!(
                                            "defined variable\nfrom: {}",
                                            local_path.file_name().unwrap().to_str().unwrap()
                                        )),
                                        ..Default::default()
                                    });
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    if complete.is_empty() {
        None
    } else {
        Some(complete)
    }
}

fn cmake_try_exists(input: &PathBuf) -> std::io::Result<bool> {
    match std::fs::metadata(input) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}
