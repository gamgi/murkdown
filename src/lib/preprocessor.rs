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
mod test_find_key {
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
