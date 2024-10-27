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
) -> Result<HashSet<Dependency>, LibError> {
    let mut deps = HashSet::new();
    let lang = Lang::new(include_str!("compiler/markdown.lang")).unwrap();
    let mut ctx = Context::default();

    preprocess_recursive(node, &mut ctx, asts, locs, context, &mut deps, &lang, "")?;
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
            preprocess_includes(node, asts, locs, context, deps);
        }
        Rule::Block => {
            preprocess_headers(node);
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
        let (schema, path) = match &**key {
            "src" => uri_or_path
                .split_once(':')
                .unwrap_or(("parse", uri_or_path)),
            "ref" => uri_or_path.split_once(':').unwrap_or(("copy", uri_or_path)),
            _ => unreachable!(),
        };
        // NOTE: resolves URI path to canonical form
        let uri_path = resolve_path(path, locs.keys(), context).unwrap_or(path);
        let uri = format!("{schema}:{uri_path}");

        // add dependency
        deps.insert(Dependency::URI(uri.clone()));

        // add placeholder node to ast
        let arc = asts.entry(uri).or_insert_with(|| {
            let root = NodeBuilder::root().build().unwrap();
            Arc::new(Mutex::new(root))
        });

        // add pointer to node
        let pointer = Pointer(Arc::downgrade(arc));
        if node.children.is_some() {
            if node.rule == Rule::Section {
                todo!();
            } else {
                node.pointer = Some(pointer);
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

/// Resolve path to matching entry from a list
pub fn resolve_path<'a, I>(path: &str, paths: I, context: &str) -> Option<&'a str>
where
    I: Iterator<Item = &'a String>,
{
    // Partition paths by matching context
    let (mut paths_within, mut paths_without): (Vec<&String>, Vec<&String>) =
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::ast::NodeBuilder;

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
        preprocess(&mut node, &mut asts, &mut locs, "").unwrap();

        let section = node.children.as_ref().unwrap().first().unwrap();
        let block = section.children.as_ref().unwrap().first().unwrap();
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

        preprocess(&mut node, &mut asts, &mut locs, "").unwrap();

        let ast_keys = asts.keys().collect::<Vec<_>>();
        assert_eq!(ast_keys, vec!["parse:bar"]);
    }

    #[test]
    fn test_preprocess_runs_precompile() {
        let mut asts = AstMap::default();
        let mut node = NodeBuilder::root()
            .add_section(vec![NodeBuilder::block(">")
                .headers(Some(vec![Arc::from("DUMMY")]))
                .done()])
            .done();
        let mut locs = LocationMap::default();

        let deps = preprocess(&mut node, &mut asts, &mut locs, "").unwrap();

        assert_eq!(
            deps,
            HashSet::from([Dependency::Exec {
                cmd: "date".to_string(),
                artifact: None,
                name: "date".into()
            }])
        );
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
        assert_eq!(resolve_path("bar", map.keys(), "bbb"), Some("bbb/bar"));
    }

    #[test]
    fn test_adjecent() {
        let mut map = HashMap::<String, ()>::new();
        map.insert("aaa/bar".to_string(), ());
        map.insert("bbb/111/foo".to_string(), ());
        map.insert("bbb/222/bar".to_string(), ());

        // prefer sibling match
        assert_eq!(
            resolve_path("bar", map.keys(), "bbb/111"),
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
            resolve_path("#id", map.keys(), "bbb/111"),
            Some("bbb/111#id")
        );
        assert_eq!(
            resolve_path("#win", map.keys(), "bbb/111"),
            Some("bbb/222#win")
        );
    }

    #[test]
    fn test_schemas() {
        let mut map = HashMap::<String, ()>::new();
        map.insert("bbb/baz#id".to_string(), ());
        map.insert("bbb/baz#win".to_string(), ());
        map.insert("aaa/bar#id".to_string(), ());
        map.insert("aaa/bar#win".to_string(), ());
        // prefer sibling match
        assert_eq!(
            resolve_path("#id", map.keys(), "bbb/bar"),
            Some("bbb/baz#id")
        );
        // then from top
        assert_eq!(
            resolve_path("#win", map.keys(), "???/bar"),
            Some("aaa/bar#win")
        );
        // should not match other schemas
        assert_eq!(resolve_path("???:#win", map.keys(), "bbb/bar"), None);
    }
}
