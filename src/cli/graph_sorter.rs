use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::Debug;
use std::hash::Hash;

use super::graph::OpGraph;
use super::op::OpId;

type IndegreeMap<T> = HashMap<T, usize>;
type EdgeMap<T> = HashMap<T, HashSet<T>>;

/// Sorter internal state
#[derive(Debug, Default)]
pub(crate) struct SorterState<T: Eq + Hash> {
    edges: EdgeMap<T>,
    indegree: IndegreeMap<T>,
    indegree_zero: Vec<T>,
}

#[inline]
pub fn add_edge<T>(edges: &mut EdgeMap<T>, from: T, to: T)
where
    T: Eq + Hash + Clone,
{
    edges
        .entry(from)
        .and_modify(|pointees| {
            pointees.insert(to.clone());
        })
        .or_insert_with(|| {
            let mut s = HashSet::new();
            s.insert(to.clone());
            s
        });
}

impl From<&OpGraph> for SorterState<OpId> {
    fn from(graph: &OpGraph) -> Self {
        let mut state = SorterState::default();
        for (from, _, deps) in graph.iter() {
            if deps.is_empty() {
                state.indegree_zero.push(from.clone());
            } else {
                state.indegree.insert(from.clone(), deps.len());
                for to in deps {
                    add_edge(&mut state.edges, to.clone(), from.clone())
                }
            }
        }
        state
    }
}

pub(crate) fn grouped_topological_sort<T, Id>(graph: T) -> Result<Vec<Vec<Id>>, Box<dyn Error>>
where
    Id: Eq + Hash + Clone + Debug + Ord,
    SorterState<Id>: From<T>,
{
    let mut res = vec![];
    let mut state = SorterState::from(graph);
    let mut current = state.indegree_zero.clone();
    while !current.is_empty() {
        current.sort();
        res.push(current.clone());
        let mut new_zero_indegree = vec![];

        for v in current {
            if let Some(edges) = state.edges.get(&v) {
                for child in edges.iter() {
                    if let Some(degree) = state.indegree.get_mut(&child) {
                        *degree -= 1;
                        if *degree == 0 {
                            new_zero_indegree.push(child.clone());
                        }
                    }
                }
            }
        }
        current = new_zero_indegree;
    }

    Ok(res)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::cli::op::Operation;

    #[test]
    fn test_topological_sort() {
        let mut graph = OpGraph::new();
        let load = graph.add_node(Operation::Load {
            id: "file.md".into(),
            path: PathBuf::from("examples/file.md"),
        });
        let parse = graph.add_node(Operation::Parse { id: "file.md".into() });
        graph.add_dependency(parse.clone(), load.clone());

        let result = grouped_topological_sort(&graph).unwrap();
        assert_eq!(result, vec![vec![load], vec![parse]]);
    }

    #[test]
    fn test_layered_topological_sort() {
        let mut graph = OpGraph::new();
        let load1 = graph.add_node(Operation::Load {
            id: "file1.md".into(),
            path: PathBuf::from("examples/file1.md"),
        });
        let parse1 = graph.add_node(Operation::Parse { id: "file1.md".into() });

        let load2 = graph.add_node(Operation::Load {
            id: "file2.md".into(),
            path: PathBuf::from("examples/file2.md"),
        });
        let parse2 = graph.add_node(Operation::Parse { id: "file2.md".into() });

        let finish = graph.add_node(Operation::Finish);
        graph.add_dependency(parse1.clone(), load1.clone());
        graph.add_dependency(parse2.clone(), load2.clone());
        graph.add_dependency(finish.clone(), parse1.clone());
        graph.add_dependency(finish.clone(), parse2.clone());

        let result = grouped_topological_sort(&graph).unwrap();
        assert_eq!(
            result,
            vec![vec![load1, load2], vec![parse1, parse2], vec![finish]]
        );
    }
}
