use std::collections::HashMap;

use super::op::{OpId, Operation};

/// Graph for storing dependencies of operations
#[derive(Debug)]
pub struct OpGraph {
    vertices: HashMap<OpId, Operation>,
    // NOTE: Vec is used so we can return a slice to the adjecency list in [`OpGraph::get_dependencies`]
    adjecency: HashMap<OpId, Vec<OpId>>,
}

impl OpGraph {
    pub fn new() -> Self {
        OpGraph {
            vertices: HashMap::new(),
            adjecency: HashMap::new(),
        }
    }

    pub fn get(&self, id: &OpId) -> Option<&Operation> {
        self.vertices.get(id)
    }

    pub fn insert_node(&mut self, op: Operation) -> OpId {
        let id = OpId::from(&op);
        self.vertices.insert(id.clone(), op);
        id
    }

    pub fn insert_node_chain<I>(&mut self, ops: I)
    where
        I: IntoIterator<Item = Operation>,
    {
        let mut iter = ops.into_iter();

        if let Some(first) = iter.next() {
            let mut prev = self.insert_node(first);
            for op in iter {
                let next = self.insert_node(op);
                self.add_dependency(next.clone(), prev);
                prev = next;
            }
        }
    }

    pub fn add_dependency(&mut self, from: OpId, to: OpId) {
        let list = self.adjecency.entry(from).or_default();
        if !list.contains(&to) {
            list.push(to);
        }
    }

    /// Get first dependency vertex
    pub fn get_first_dependency(&self, from: &OpId) -> Option<&Operation> {
        self.get_dependencies(from)
            .first()
            .and_then(|id| self.vertices.get(id))
    }

    /// Get first dependency vertex id
    pub fn get_first_node_dependency(&self, node: &Operation) -> Option<OpId> {
        let from = OpId::from(node);
        self.get_dependencies(&from).first().cloned()
    }

    pub fn get_dependencies(&self, from: &OpId) -> &[OpId] {
        self.adjecency.get(from).map_or(&[], |v| v.as_slice())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&OpId, &Operation, &[OpId])> {
        self.vertices
            .iter()
            .map(move |(from, v)| match self.adjecency.get(from) {
                Some(edges) => (from, v, &edges[..]),
                None => (from, v, &[] as &[OpId]),
            })
    }

    pub fn len(&self) -> usize {
        self.vertices.len()
    }

    /// Clear the graph
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.adjecency.clear();
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc};

    use super::*;

    #[test]
    fn test_graph() {
        let mut graph = OpGraph::new();
        graph.insert_node(Operation::Load {
            id: Arc::from("foo"),
            path: PathBuf::new(),
        });
        graph.insert_node(Operation::Load {
            id: Arc::from("bar"),
            path: PathBuf::new(),
        });

        assert_eq!(graph.vertices.len(), 2);
    }

    #[test]
    fn test_graph_iter() {
        let mut graph = OpGraph::new();
        graph.insert_node(Operation::Load {
            id: Arc::from("foo"),
            path: PathBuf::new(),
        });
        graph.insert_node(Operation::Load {
            id: Arc::from("bar"),
            path: PathBuf::new(),
        });
        graph.add_dependency(OpId::load("foo"), OpId::load("bar"));

        let mut result = vec![];
        for (from, _vertex, edges) in graph.iter() {
            let deps = edges.iter().cloned().collect::<Vec<_>>();
            result.push((from, deps));
        }
        result.sort();
        assert_eq!(
            result,
            [
                (&OpId::load("bar"), vec![]),
                (&OpId::load("foo"), vec![OpId::load("bar")]),
            ]
        );
    }
}
