use std::collections::hash_map::Entry;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use crate::ast::{Node, NodeBuilder};
use crate::compiler::lang::Lang;
use crate::compiler::rule::Context;
use crate::parser::Rule;
use crate::types::{AstMap, Dependency, LibError, LocationMap, Pointer};

static PREPROCESSABLE_PROPS: &[&str] = &["src", "ref"];

/// Preprocess AST
pub fn preprocess(
    node: &mut Node,
    asts: &mut AstMap,
    locs: &LocationMap,
    context: &str,
    lang: Option<&Lang>,
) -> Result<HashSet<Dependency>, LibError> {
    let mut deps = HashSet::new();
    let lang = lang.expect("language");
    let mut ctx = Context::default();

    preprocess_recursive(node, &mut ctx, asts, locs, context, &mut deps, lang, "")?;
    Ok(deps)
}

#[allow(clippy::too_many_arguments)]
fn preprocess_recursive<'a>(
    node: &mut Node,
    ctx: &mut Context<'a>,
    asts: &mut AstMap,
    locs: &LocationMap,
    context: &str,
    deps: &mut HashSet<Dependency>,
    lang: &'a Lang,
    base_path: &str,
) -> Result<(), LibError> {
    let path = node.build_path(base_path);
    let mut instructions = lang.get_instructions("PREPROCESS", &path);

    // Evaluate pre-yield
    lang.evaluate(&mut instructions, ctx, deps, node)?;

    if let Some(children) = node.children.as_mut() {
        for child in children.iter_mut() {
            preprocess_recursive(child, ctx, asts, locs, context, deps, lang, &path)?;
        }
    }

    // Evaluate post-yield
    lang.evaluate(&mut instructions, &mut *ctx, deps, node)?;

    match node.rule {
        Rule::Root => {
            preprocess_headers(node);
            preprocess_ids(node, asts, context);
            preprocess_includes(node, asts, locs, context, deps);
        }
        Rule::Block => {
            preprocess_headers(node);
            preprocess_ids(node, asts, context);
            preprocess_includes(node, asts, locs, context, deps);
        }
        _ => {}
    }

    Ok(())
}

/// Adds implicit headers to nodes
fn preprocess_headers(node: &mut Node) {
    match node.marker.as_deref() {
        Some("#") => {
            let headers = node.headers.get_or_insert_with(Default::default);
            let header = Arc::from("HEADING");
            if !headers.contains(&header) {
                headers.push(header);
            }
        }
        Some("*") => {
            let headers = node.headers.get_or_insert_with(Default::default);
            let header = Arc::from("LIST");
            if !headers.contains(&header) {
                headers.push(header);
            }
        }
        _ => {}
    }
}

/// Moves nodes with id to asts
fn preprocess_ids(node: &mut Node, asts: &mut AstMap, context: &str) {
    if let Some(id) = node.find_prop("id") {
        let uri = format!("parse:{context}#{id}");

        // pull node out and replace with new
        let mut new = node.clone();
        new.children = None;

        let old = std::mem::replace(node, new);
        let arc = match asts.entry(uri) {
            Entry::Occupied(r) => {
                let mut mutex = r.get().lock().expect("poisoned lock");
                *mutex = old;
                &r.get().clone()
            }
            Entry::Vacant(r) => &*r.insert(Arc::new(Mutex::new(old))),
        };

        // add pointer to new node
        let pointer = Pointer(Arc::downgrade(arc));
        node.pointer = Some(pointer);
    }
}

/// Adds include pointers to nodes and updates deps
fn preprocess_includes(
    node: &mut Node,
    asts: &mut AstMap,
    locs: &LocationMap,
    context: &str,
    deps: &mut HashSet<Dependency>,
) {
    let props = node
        .props
        .as_ref()
        .map_or(&[] as &[_], Vec::as_slice)
        .iter()
        .filter(|&(k, _)| PREPROCESSABLE_PROPS.contains(&&**k));

    for (key, uri_or_path) in props {
        let (scheme, path) = match &**key {
            "src" => uri_or_path
                .split_once(':')
                .unwrap_or(("parse", uri_or_path)),
            "ref" => uri_or_path.split_once(':').unwrap_or(("copy", uri_or_path)),
            _ => unreachable!(),
        };
        let (path, fragment) = path.rsplit_once('#').unwrap_or((path, ""));

        // NOTE: first resolve URI path to canonical form
        let uri_path = resolve_path(path, locs.keys().map(String::as_str), context).unwrap_or(path);

        let uri_path = match fragment.is_empty() {
            true => uri_path.to_string(),
            false => format!("{uri_path}#{fragment}"),
        };

        // NOTE: then resolve URI path to possible AST node
        let uri_path = resolve_scheme_path(&uri_path, scheme, asts.keys(), context)
            .unwrap_or(&uri_path)
            .to_string();

        let uri = format!("{scheme}:{uri_path}");

        // add dependency
        match &**key {
            "ref" => deps.insert(Dependency::URI("ref", uri.clone())),
            "src" => deps.insert(Dependency::URI("src", uri.clone())),
            _ => unreachable!(),
        };

        // add placeholder node to ast
        let arc = asts.entry(uri).or_insert_with(|| {
            let root = NodeBuilder::root().build().unwrap();
            Arc::new(Mutex::new(root))
        });

        // add pointer to node
        let pointer = Some(Pointer(Arc::downgrade(arc)));
        if let Some(children) = node.children.as_mut() {
            if node.rule == Rule::Section {
                todo!();
            } else if let Some(node) = get_ellipsis_node_recursive(children.as_mut_slice()) {
                node.pointer = pointer;
            } else {
                node.pointer = pointer;
            }
        } else {
            let section = NodeBuilder::new(Rule::Section)
                .pointer(pointer)
                .build()
                .unwrap();
            node.children = Some(vec![section]);
        }
    }
}

fn get_ellipsis_node_recursive(nodes: &mut [Node]) -> Option<&mut Node> {
    for node in nodes.iter_mut() {
        if node.pointer.is_some() || node.find_prop("src").is_some() {
            continue;
        } else if matches!(node.rule, Rule::Ellipsis | Rule::EllipsisEOI) {
            return Some(node);
        } else if let Some(children) = node.children.as_mut() {
            if let Some(result) = get_ellipsis_node_recursive(children.as_mut_slice()) {
                return Some(result);
            }
        }
    }
    None
}

/// Resolve path to matching entry from a list
pub fn resolve_path<'a, I>(path: &str, paths: I, context: &str) -> Option<&'a str>
where
    I: Iterator<Item = &'a str>,
{
    // Partition paths by matching context
    let (mut paths_within, mut paths_without): (Vec<&str>, Vec<&str>) =
        paths.partition(|u| u.starts_with(context));

    paths_within.sort();

    // First search in context
    if let Some(result) = paths_within.iter().find(|k| k.ends_with(path)) {
        return Some(result);
    }

    paths_without.sort();

    // Then search in context siblings
    if let Some(idx) = context.find('/') {
        if idx != context.len() {
            let sibling = &context[0..idx];
            if let Some(result) = paths_without
                .iter()
                .find(|k| k.starts_with(sibling) && k.ends_with(path))
            {
                return Some(result);
            }
        }
    }

    // Finally search from root
    if let Some(result) = paths_without.iter().find(|k| k.ends_with(path)) {
        return Some(result);
    }
    None
}

/// Resolve path to matching entry from a list with a given scheme prefix
pub fn resolve_scheme_path<'a, I>(
    path: &str,
    scheme: &str,
    paths: I,
    context: &str,
) -> Option<&'a str>
where
    I: Iterator<Item = &'a String>,
{
    let prefix = format!("{scheme}:");
    let paths = paths.filter_map(|p| p.strip_prefix(&prefix));
    resolve_path(path, paths, context)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{ast::NodeBuilder, types::ExecArtifact};

    #[test]
    fn test_preprocess_adds_pointer() {
        let mut asts = AstMap::default();
        let mut node = NodeBuilder::root()
            .children(vec![NodeBuilder::block(">")
                .add_prop(("src".into(), "bar".into()))
                .done()])
            .done();
        let mut locs = LocationMap::default();
        locs.insert("bar".to_string(), PathBuf::from("something.txt"));
        let lang = Some(Lang::default());
        preprocess(&mut node, &mut asts, &mut locs, "", lang.as_ref()).unwrap();

        let section = node.children.as_ref().unwrap().first().unwrap();
        let block = section.children.as_ref().unwrap().first().unwrap();
        assert!(block.pointer.is_some());
    }

    #[test]
    fn test_preprocess_adds_pointer_at_ellipsis() {
        let mut asts = AstMap::default();
        let mut node = NodeBuilder::root()
            .children(vec![NodeBuilder::block(">")
                .add_prop(("src".into(), "bar".into()))
                .children(vec![
                    Node::new_line("foo"),
                    NodeBuilder::new(Rule::Ellipsis).done(),
                    Node::new_line("baz"),
                ])
                .done()])
            .done();
        let mut locs = LocationMap::default();
        let lang = Some(Lang::default());

        preprocess(&mut node, &mut asts, &mut locs, "", lang.as_ref()).unwrap();

        let section = node.children.as_ref().unwrap().first().unwrap();
        let block = section
            .children
            .as_ref()
            .unwrap()
            .into_iter()
            .nth(1)
            .unwrap();

        assert!(block.pointer.is_some());
    }

    #[test]
    fn test_preprocess_adds_asts() {
        let mut asts = AstMap::default();
        let mut node = NodeBuilder::root()
            .children(vec![NodeBuilder::block(">")
                .add_prop(("src".into(), "bar".into()))
                .done()])
            .done();
        let mut locs = LocationMap::default();
        locs.insert("bar".to_string(), PathBuf::from("something.txt"));
        let lang = Some(Lang::default());

        preprocess(&mut node, &mut asts, &mut locs, "", lang.as_ref()).unwrap();

        let ast_keys = asts.keys().collect::<Vec<_>>();
        assert_eq!(ast_keys, vec!["parse:bar"]);
    }

    #[test]
    fn test_preprocess_moves_id_nodes_to_asts() {
        let mut asts = AstMap::default();
        let block = NodeBuilder::block(">")
            .add_prop(("id".into(), "bar".into()))
            .done();

        let mut node = NodeBuilder::root().children(vec![block.clone()]).done();
        let mut locs = LocationMap::default();
        let lang = Some(Lang::default());
        preprocess(&mut node, &mut asts, &mut locs, "foo", lang.as_ref()).unwrap();

        let new_block = node.children.as_ref().unwrap().first().unwrap();
        assert!(new_block.pointer.is_some());

        let moved_block = asts.get("parse:foo#bar").unwrap().lock().unwrap();
        assert_eq!(*moved_block, block);
    }

    #[test]
    fn test_preprocess_resolves_paths_and_fragments() {
        let mut asts = AstMap::default();
        asts.insert(
            "parse:other.md#bar".to_string(),
            Arc::new(Mutex::new(Node::new_line("other"))),
        );

        let mut node = NodeBuilder::root()
            .children(vec![NodeBuilder::block(">")
                .add_prop(("src".into(), "#bar".into()))
                .done()])
            .done();
        let mut locs = LocationMap::default();
        let lang = Some(Lang::default());
        locs.insert("file.md".to_string(), PathBuf::from("file.md"));
        locs.insert("other.md".to_string(), PathBuf::from("other.md"));
        preprocess(&mut node, &mut asts, &mut locs, "file.md", lang.as_ref()).unwrap();

        let section = node.children.as_ref().unwrap().first().unwrap();
        let block = section.children.as_ref().unwrap().first().unwrap();
        assert!(block.pointer.is_some());
    }

    #[test]
    fn test_preprocess_runs_precompile() {
        let mut asts = AstMap::default();
        let mut node = NodeBuilder::root()
            .add_section(vec![NodeBuilder::block(">")
                .headers(Some(vec![Arc::from("DATE")]))
                .done()])
            .done();
        let mut locs = LocationMap::default();
        let lang = Some(Lang::default());

        let deps = preprocess(&mut node, &mut asts, &mut locs, "", lang.as_ref()).unwrap();

        assert_eq!(
            deps,
            HashSet::from([
                Dependency::URI("src", "exec:date".to_string()),
                Dependency::Exec {
                    cmd: "date".to_string(),
                    input: None,
                    artifact: ExecArtifact::Stdout("text/plain".to_string()),
                    id: "date".into(),
                }
            ])
        );

        let section = node.children.as_ref().unwrap().first().unwrap();
        let block = section.children.as_ref().unwrap().first().unwrap();

        assert_eq!(block.props, Some(vec![("src".into(), "exec:date".into())]));
    }
}

#[cfg(test)]
mod tests_resolve_path {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn test_within() {
        let mut map = HashMap::<String, ()>::new();
        map.insert("aaa/bar".to_string(), ());
        map.insert("bbb/bar".to_string(), ());
        assert_eq!(
            resolve_path("bar", map.keys().map(String::as_str), "bbb"),
            Some("bbb/bar")
        );
    }

    #[test]
    fn test_adjecent() {
        let mut map = HashMap::<String, ()>::new();
        map.insert("aaa/bar".to_string(), ());
        map.insert("bbb/111/foo".to_string(), ());
        map.insert("bbb/222/bar".to_string(), ());

        // prefer sibling match
        assert_eq!(
            resolve_path("bar", map.keys().map(String::as_str), "bbb/111"),
            Some("bbb/222/bar")
        );
    }

    #[test]
    fn test_fragments() {
        let mut map = HashMap::<String, ()>::new();
        map.insert("bbb/111#id".to_string(), ());
        map.insert("bbb/222#id".to_string(), ());
        map.insert("bbb/222#win".to_string(), ());
        map.insert("aaa/111#id".to_string(), ());
        map.insert("aaa/111#win".to_string(), ());
        assert_eq!(
            resolve_path("#id", map.keys().map(String::as_str), "bbb/111"),
            Some("bbb/111#id")
        );

        assert_eq!(
            resolve_path("#win", map.keys().map(String::as_str), "bbb/111"),
            Some("bbb/222#win")
        );
    }
}

#[cfg(test)]
mod tests_resolve_scheme_path {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn test_schemas() {
        let mut map = HashMap::<String, ()>::new();
        map.insert("this:bbb/baz#id".to_string(), ());
        map.insert("other:bbb/baz#win".to_string(), ());
        map.insert("this:aaa/bar#id".to_string(), ());
        map.insert("this:aaa/bar#win".to_string(), ());
        // prefer sibling match
        assert_eq!(
            resolve_scheme_path("#id", "this", map.keys(), "bbb/bar"),
            Some("bbb/baz#id")
        );
        // then from top
        assert_eq!(
            resolve_scheme_path("#win", "this", map.keys(), "???/bar"),
            Some("aaa/bar#win")
        );
        // should not match other schemas
        assert_eq!(resolve_scheme_path("baz#win", "this", map.keys(), ""), None);
    }
}
