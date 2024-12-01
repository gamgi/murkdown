use std::collections::hash_map::Entry;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use crate::ast::{Node, NodeBuilder};
use crate::compiler::lang::Lang;
use crate::compiler::rule::{Context, LangSettings};
use crate::parser::Rule;
use crate::types::{AstMap, Dependency, LibError, LocationMap, Pointer, URI};

static PREPROCESSABLE_PROPS: &[&str] = &["src", "ref"];

/// Preprocess AST
pub fn preprocess(
    node: &mut Node,
    asts: &mut AstMap,
    locs: &LocationMap,
    context: &str,
    lang: &Lang,
) -> Result<(HashSet<Dependency>, HashSet<URI>), LibError> {
    let mut deps = HashSet::new();
    let mut new_asts = HashSet::new();
    let mut ctx = Context::default();

    preprocess_recursive(
        node,
        &mut ctx,
        asts,
        locs,
        context,
        &mut deps,
        &mut new_asts,
        lang,
        "",
    )?;
    Ok((deps, new_asts))
}

#[allow(clippy::too_many_arguments)]
fn preprocess_recursive<'a>(
    node: &mut Node,
    ctx: &mut Context<'a>,
    asts: &mut AstMap,
    locs: &LocationMap,
    context: &str,
    deps: &mut HashSet<Dependency>,
    new_asts: &mut HashSet<String>,
    lang: &'a Lang,
    base_path: &str,
) -> Result<(), LibError> {
    let path = node.build_path(base_path);
    ctx.set_parent(node);
    let mut rules = lang.get_rules("PREPROCESS", &path).peekable();
    let mut rules_stack = Vec::new();
    let mut merged_settings = rules.peek().map(|r| r.settings).unwrap_or_default();

    // Evaluate pre-yield
    for rule in rules {
        let mut instructions = rule.instructions.iter();
        merged_settings.merge(&rule.settings);
        lang.evaluate(&mut instructions, &mut *ctx, deps, node, &rule.settings)?;
        rules_stack.push((instructions, rule.settings));
    }
    let settings = merged_settings;

    match node.rule {
        Rule::RootA | Rule::RootB => {
            preprocess_headers(node);
            preprocess_includes(node, asts, locs, context, deps, &settings);
        }
        Rule::Block => {
            preprocess_headers(node);
            preprocess_includes(node, asts, locs, context, deps, &settings);
        }
        Rule::Section => {
            preprocess_paragraphs(node, &settings);
        }
        _ => {}
    }

    // NOTE: preprocess children after path may have changed (eg. new headers)
    if node.children.is_some() {
        let path = node.build_path(base_path);

        let children = node.children.as_mut().expect("is some");
        for child in children.iter_mut() {
            preprocess_recursive(child, ctx, asts, locs, context, deps, new_asts, lang, &path)?;
        }
    }

    // NOTE: moves ids to ast map after children have been preprocessed
    match node.rule {
        Rule::RootA | Rule::RootB => {
            preprocess_ids(node, asts, context, new_asts);
        }
        Rule::Block => {
            preprocess_ids(node, asts, context, new_asts);
        }
        _ => {}
    }

    // Evaluate post-yield
    rules_stack.reverse();
    for (mut instructions, settings) in rules_stack {
        lang.evaluate(&mut instructions, &mut *ctx, deps, node, &settings)?;
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
        Some("   ") => {
            let headers = node.headers.get_or_insert_with(Default::default);
            let header = Arc::from("CODE");
            if !headers.contains(&header) {
                headers.push(header);
            }
        }
        _ => {}
    }
}

/// Moves nodes with id to asts
fn preprocess_ids(
    node: &mut Node,
    asts: &mut AstMap,
    context: &str,
    new_asts: &mut HashSet<String>,
) {
    if let Some(id) = node.find_prop("id") {
        let uri = match context.is_empty() {
            true => id.to_string(),
            false => format!("parse:{context}#{id}"),
        };

        if let Some(Pointer(weak)) = &node.pointer {
            // insert existing pointer node to asts at uri
            let arc = weak.upgrade().unwrap();
            match asts.entry(uri.clone()) {
                Entry::Occupied(_) => todo!("duplicate id"),
                Entry::Vacant(r) => &*r.insert(arc),
            };
            new_asts.insert(uri);
        } else {
            // pull node out and replace with new
            let mut new = node.clone();
            new.children = None;
            let old = std::mem::replace(node, new);

            // insert old node to asts at uri
            let arc = match asts.entry(uri.clone()) {
                Entry::Occupied(r) => {
                    let mut mutex = r.get().lock().expect("poisoned lock");
                    *mutex = old;
                    &r.get().clone()
                }
                Entry::Vacant(r) => &*r.insert(Arc::new(Mutex::new(old))),
            };
            new_asts.insert(uri);

            // add pointer to new node
            let pointer = Pointer(Arc::downgrade(arc));
            node.pointer = Some(pointer);
        }
    }
}

/// Adds include pointers to nodes and updates deps
fn preprocess_includes(
    node: &mut Node,
    asts: &mut AstMap,
    locs: &LocationMap,
    context: &str,
    deps: &mut HashSet<Dependency>,
    settings: &LangSettings,
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
                .unwrap_or((settings.default_src.unwrap_or("parse"), uri_or_path)),
            "ref" => uri_or_path
                .split_once(':')
                .unwrap_or((settings.default_ref.unwrap_or("write"), uri_or_path)),
            _ => unreachable!(),
        };

        let (scheme, is_resolved) = match scheme.split_once('?') {
            Some((s, _)) => (s, true),
            None => (scheme, false),
        };

        let uri_path = if is_resolved {
            // NOTE: schemes with ? are pre-resolved
            path.to_string()
        } else {
            // TODO: improve and clarify resolving
            let (path, fragment) = path.rsplit_once('#').unwrap_or((path, ""));

            // NOTE: first resolve URI path to canonical form
            let prefix =
                resolve_path(path, locs.keys().map(String::as_str), context).unwrap_or_default();

            let uri_path = match fragment.is_empty() {
                true if prefix.is_empty() => format!("{context}#{path}"),
                true => prefix.to_string(),
                false if prefix.is_empty() => format!("#{fragment}"),
                false => format!("{prefix}#{fragment}"),
            };

            // NOTE: then resolve URI path to possible AST node
            let uri_path = resolve_scheme_path(&uri_path, scheme, asts.keys(), context)
                .unwrap_or(&uri_path)
                .to_string();
            uri_path
        };

        let uri = format!("{scheme}:{uri_path}");

        // add dependency
        match &**key {
            "ref" => deps.insert(Dependency::URI("ref", uri.clone())),
            "src" => deps.insert(Dependency::URI("src", uri.clone())),
            _ => unreachable!(),
        };

        // add placeholder node to ast
        let arc = asts.entry(uri.clone()).or_insert_with(|| {
            let root = NodeBuilder::root().build().unwrap();
            Arc::new(Mutex::new(root))
        });

        if &**key == "ref" {
            continue;
        }

        // add pointer to node
        let pointer = Some(Pointer(Arc::downgrade(arc)));
        if let Some(children) = node.children.as_mut() {
            if let Some(node) = get_ellipsis_node_recursive(children.as_mut_slice()) {
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

/// Join adjacent lines in sections into paragraphs
fn preprocess_paragraphs(node: &mut Node, settings: &LangSettings) {
    if !settings.is_paragraphable {
        return;
    }

    if node.headers.is_none() {
        if let Some(children) = node.children.take() {
            let res = Vec::<Node>::new();
            let new_children = children.into_iter().fold(res, |mut res, mut right| {
                if let Some(left) = res.last_mut() {
                    let left_v = left.value.as_deref();
                    let right_v = right.value.as_deref();

                    if matches!(left.rule, Rule::Line | Rule::Paragraph)
                        && matches!(right.rule, Rule::Line)
                        && !right_v.unwrap().is_empty()
                    {
                        match (left_v, right_v) {
                            (_, Some("")) => unreachable!(),
                            (Some(""), _) => res.push(right),
                            (Some(_), Some(r)) => match left.rule {
                                Rule::Line => {
                                    let l = left_v.unwrap();
                                    let value = Arc::from(format!("{l} {r}"));
                                    let new = std::mem::take(left);
                                    left.value = Some(value);
                                    left.rule = Rule::Paragraph;
                                    left.children = Some(vec![new, right]);
                                }
                                Rule::Paragraph => {
                                    let l = left_v.unwrap();
                                    let value = Arc::from(format!("{l} {r}"));
                                    left.value = Some(value);
                                    left.children.as_mut().unwrap().push(right);
                                }
                                _ => unreachable!(),
                            },
                            _ => res.push(right),
                        }
                    } else if matches!(left.rule, Rule::Line) && !left_v.unwrap().is_empty() {
                        // turn left into paragraph
                        let l = left_v.unwrap();
                        let value = Arc::from(l);
                        let new = std::mem::take(left);
                        left.value = Some(value);
                        left.rule = Rule::Paragraph;
                        left.children = Some(vec![new]);

                        res.push(right);
                    } else if matches!(right.rule, Rule::Line) && !right_v.unwrap().is_empty() {
                        // turn right into paragraph
                        let r = right_v.unwrap();
                        let value = Arc::from(r);
                        let new = std::mem::take(&mut right);
                        right.value = Some(value);
                        right.rule = Rule::Paragraph;
                        right.children = Some(vec![new]);

                        res.push(right);
                    } else {
                        res.push(right);
                    }
                } else {
                    res.push(right);
                }
                res
            });
            node.children = Some(new_children);
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

    use indoc::indoc;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{ast::NodeBuilder, types::ExecArtifact};

    #[test]
    fn test_preprocess_adds_pointer_to_block() {
        let mut asts = AstMap::default();
        let mut node = NodeBuilder::root()
            // NOTE: no section
            .children(vec![NodeBuilder::block(">")
                .add_prop(("src".into(), "bar".into()))
                .done()])
            .done();
        let mut locs = LocationMap::default();
        locs.insert("bar".to_string(), PathBuf::from("something.txt").into());
        let lang = Lang::markdown();
        preprocess(&mut node, &mut asts, &mut locs, "", &lang).unwrap();

        let section = node.children.as_ref().unwrap().first().unwrap();
        let block = section.children.as_ref().unwrap().first().unwrap();
        assert!(block.pointer.is_some());
    }

    #[test]
    fn test_preprocess_adds_pointer_to_block_section() {
        let mut asts = AstMap::default();
        let mut node = NodeBuilder::root()
            .add_section(vec![NodeBuilder::block(">")
                .add_prop(("src".into(), "bar".into()))
                .done()])
            .done();
        let mut locs = LocationMap::default();
        locs.insert("bar".to_string(), PathBuf::from("something.txt").into());
        let lang = Lang::markdown();
        preprocess(&mut node, &mut asts, &mut locs, "", &lang).unwrap();

        let section = node.children.as_ref().unwrap().first().unwrap();
        let block = section.children.as_ref().unwrap().first().unwrap();
        let block_section = block.children.as_ref().unwrap().first().unwrap();

        assert_eq!(block_section.rule, Rule::Section);
        assert!(block_section.pointer.is_some());
    }

    #[test]
    fn test_preprocess_adds_pointer_at_ellipsis() {
        let mut asts = AstMap::default();
        let mut node = NodeBuilder::root()
            .children(vec![NodeBuilder::block(">")
                .add_prop(("src".into(), "bar".into()))
                .add_section(vec![
                    Node::line("foo"),
                    NodeBuilder::new(Rule::Ellipsis).done(),
                    Node::line("baz"),
                ])
                .done()])
            .done();
        let mut locs = LocationMap::default();
        let lang = Lang::markdown();

        preprocess(&mut node, &mut asts, &mut locs, "", &lang).unwrap();

        let block = node.children.as_ref().unwrap().first().unwrap();
        let section = block.children.as_ref().unwrap().first().unwrap();
        let ellipsis = section
            .children
            .as_ref()
            .unwrap()
            .into_iter()
            .nth(1)
            .unwrap();

        assert_eq!(ellipsis.rule, Rule::Ellipsis);
        assert!(ellipsis.pointer.is_some());
    }

    #[test]
    fn test_preprocess_does_not_add_pointer_for_ref() {
        let mut asts = AstMap::default();
        let mut node = NodeBuilder::root()
            .add_section(vec![NodeBuilder::block(">")
                .add_prop(("ref".into(), "bar".into()))
                .done()])
            .done();
        let mut locs = LocationMap::default();
        let lang = Lang::markdown();
        preprocess(&mut node, &mut asts, &mut locs, "", &lang).unwrap();

        let section = node.children.as_ref().unwrap().first().unwrap();
        let block = section.children.as_ref().unwrap().first().unwrap();

        assert_eq!(block.rule, Rule::Block);
        assert!(block.pointer.is_none());
    }

    #[test]
    fn test_preprocess_adds_asts() {
        let mut asts = AstMap::default();
        let mut node = NodeBuilder::root()
            .children(vec![
                NodeBuilder::block(">")
                    .add_prop(("src".into(), "foo".into()))
                    .done(),
                NodeBuilder::block(">")
                    .add_prop(("src".into(), "bar".into()))
                    .done(),
                NodeBuilder::block(">")
                    .add_prop(("src".into(), "exec?:baz".into()))
                    .done(),
                NodeBuilder::block(">")
                    .add_prop(("src".into(), "exec:code.sh".into()))
                    .done(),
            ])
            .done();
        let mut locs = LocationMap::default();
        locs.insert("bar".to_string(), PathBuf::from("something.txt").into());
        locs.insert("file.md".to_string(), PathBuf::from("file.md").into());
        locs.insert(
            "path/code.sh".to_string(),
            PathBuf::from("path/code.sh").into(),
        );
        let lang = Lang::markdown();

        preprocess(&mut node, &mut asts, &mut locs, "file.md", &lang).unwrap();

        let mut ast_keys = asts.keys().collect::<Vec<_>>();
        ast_keys.sort();
        assert_eq!(
            ast_keys,
            vec![
                "exec:baz",
                "exec:path/code.sh",
                "parse:bar",
                "parse:file.md#foo"
            ]
        );
    }

    #[test]
    fn test_preprocess_moves_id_to_asts_and_processes_children() {
        let mut asts = AstMap::default();
        let block = NodeBuilder::block(">")
            .add_prop(("id".into(), "bar".into()))
            .add_section(vec![NodeBuilder::block("*")
                .children(vec![Node::line("foo")])
                .done()])
            .done();
        let expected_block = NodeBuilder::block(">")
            .add_prop(("id".into(), "bar".into()))
            .add_section(vec![NodeBuilder::block("*")
                .headers(Some(vec![Arc::from("LIST")]))
                .children(vec![Node::line("foo")])
                .done()])
            .done();

        let mut node = NodeBuilder::root().children(vec![block.clone()]).done();
        let mut locs = LocationMap::default();
        let lang = Lang::markdown();
        preprocess(&mut node, &mut asts, &mut locs, "foo", &lang).unwrap();

        let moved_block = asts.get("parse:foo#bar").unwrap().lock().unwrap();
        assert_eq!(*moved_block, expected_block);
    }

    #[test]
    fn test_preprocess_moves_node_id_to_asts() {
        let mut asts = AstMap::default();
        let block = NodeBuilder::block(">")
            .add_prop(("id".into(), "bar".into()))
            .done();

        let mut node = NodeBuilder::root().children(vec![block.clone()]).done();
        let mut locs = LocationMap::default();
        let lang = Lang::markdown();
        preprocess(&mut node, &mut asts, &mut locs, "foo", &lang).unwrap();

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
            Arc::new(Mutex::new(Node::line("other"))),
        );

        let mut node = NodeBuilder::root()
            .children(vec![NodeBuilder::block(">")
                .add_prop(("src".into(), "#bar".into()))
                .done()])
            .done();
        let mut locs = LocationMap::default();
        let lang = Lang::markdown();
        locs.insert("file.md".to_string(), PathBuf::from("file.md").into());
        locs.insert("other.md".to_string(), PathBuf::from("other.md").into());
        preprocess(&mut node, &mut asts, &mut locs, "file.md", &lang).unwrap();

        let section = node.children.as_ref().unwrap().first().unwrap();
        let block = section.children.as_ref().unwrap().first().unwrap();
        assert!(block.pointer.is_some());
    }

    #[test]
    fn test_preprocess_returns_deps_and_new_asts() {
        let mut asts = AstMap::default();

        let mut node = NodeBuilder::root()
            .children(vec![
                NodeBuilder::block(">")
                    .add_prop(("id".into(), "code".into()))
                    .done(),
                NodeBuilder::block(">")
                    .add_prop(("src".into(), "exec:code".into()))
                    .done(),
            ])
            .done();
        let mut locs = LocationMap::default();
        let lang = Lang::markdown();
        locs.insert("file.md".to_string(), PathBuf::from("file.md").into());
        let (deps, new_asts) =
            preprocess(&mut node, &mut asts, &mut locs, "file.md", &lang).unwrap();

        assert_eq!(
            deps,
            HashSet::from([Dependency::URI("src", "exec:file.md#code".to_string()),])
        );
        assert_eq!(new_asts, HashSet::from(["parse:file.md#code".to_string()]));
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
        locs.insert("file.md".to_string(), PathBuf::from("file.md").into());
        let lang = Lang::markdown();

        let (deps, _) = preprocess(&mut node, &mut asts, &mut locs, "file.md", &lang).unwrap();

        assert_eq!(
            deps,
            HashSet::from([
                Dependency::Exec {
                    cmd: "date".to_string(),
                    input: None,
                    artifact: ExecArtifact::Stdout("text/plain".to_string()),
                    id: "date".into(),
                },
                Dependency::URI("src", "exec:date".to_string()),
            ])
        );

        let section = node.children.as_ref().unwrap().first().unwrap();
        let block = section.children.as_ref().unwrap().first().unwrap();

        assert_eq!(block.props, Some(vec![("src".into(), "exec?:date".into())]));
    }

    #[test]
    fn test_preprocess_builds_paragraphs_but_retains_empty_lines() {
        let rules = indoc! {
            r#"
            RULES FOR test PRODUCE text/plain
            PREPROCESS RULES:
            [SEC...]$
              IS PARAGRAPHABLE
            "#
        };
        let mut asts = AstMap::default();
        let mut node = NodeBuilder::root().add_section(vec![Node::line("")]).done();
        let mut locs = LocationMap::default();
        let lang = Lang::new(rules).unwrap();
        preprocess(&mut node, &mut asts, &mut locs, "", &lang).unwrap();

        let section = node.children.as_ref().unwrap().first().unwrap();
        let children = section.children.as_ref().unwrap();
        assert_eq!(children, &vec![Node::line(""),]);
    }

    #[test]
    fn test_preprocess_builds_paragraphs() {
        let rules = indoc! {
            r#"
            RULES FOR test PRODUCE text/plain
            PREPROCESS RULES:
            [SEC...]$
              IS PARAGRAPHABLE
            "#
        };
        let mut asts = AstMap::default();
        let mut node = NodeBuilder::root()
            .add_section(vec![
                Node::line("foo"),
                Node::line("bar"),
                Node::line("baz"),
                Node::ellipsis(None),
                Node::line("foo"),
                Node::line("bar"),
                Node::ellipsis(None),
                Node::line(""),
                Node::line("foo"),
                Node::line(""),
                Node::ellipsis(None),
                Node::line("bar"),
            ])
            .done();
        let mut locs = LocationMap::default();
        let lang = Lang::new(rules).unwrap();
        preprocess(&mut node, &mut asts, &mut locs, "", &lang).unwrap();

        let section = node.children.as_ref().unwrap().first().unwrap();
        let children = section.children.as_ref().unwrap();
        assert_eq!(
            children,
            &vec![
                Node::paragraph(&["foo", "bar", "baz"]),
                Node::ellipsis(None),
                Node::paragraph(&["foo", "bar"]),
                Node::ellipsis(None),
                Node::line(""),
                Node::paragraph(&["foo"]),
                Node::line(""),
                Node::ellipsis(None),
                Node::paragraph(&["bar"]),
            ]
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
