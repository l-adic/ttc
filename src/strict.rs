use petgraph::{
    graph::{DiGraph, NodeIndex},
    visit::{depth_first_search, Control, DfsEvent},
    Direction, Graph,
};
use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    hash::Hash,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TTCError {
    #[error("Graph is empty")]
    EmptyGraph,
    #[error("Graph contains an invalid edge: {}", _0)]
    InvalidEdge(String),
    #[error("Graph will always have a cycle")]
    AlwaysCycles,
}

#[derive(Debug, Error)]
pub enum PrefsError<V: Debug + Display> {
    #[error("{} has preferences for options that don't exist", _0)]
    InvalidChoice(V),
}

#[derive(Debug)]
pub struct PreferenceGraph<V> {
    graph: DiGraph<V, ()>,
    prefs: Preferences<V>,
}

#[derive(Debug)]
pub struct Preferences<V> {
    prefs: HashMap<V, Vec<V>>,
}

#[derive(Debug)]
pub struct Cycle<V> {
    pub values: Vec<V>,
}

impl<V: Eq + Clone + std::hash::Hash> PartialEq for Cycle<V> {
    fn eq(&self, other: &Self) -> bool {
        if self.values.len() != other.values.len() {
            return false;
        }

        let len = self.values.len();
        for i in 0..len {
            if self
                .values
                .iter()
                .cycle()
                .skip(i)
                .take(len)
                .eq(other.values.iter())
            {
                return true;
            }
        }

        false
    }
}

pub struct Solution<V> {
    pub res: Vec<Cycle<V>>,
}

impl<V: Debug> std::fmt::Display for Solution<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.res)
    }
}

impl<V> Preferences<V>
where
    V: Debug + Display + Eq + Hash + Clone,
{
    pub fn new(prefs: HashMap<V, Vec<V>>) -> Result<Self, PrefsError<V>> {
        for (k, vs) in prefs.iter() {
            if !vs.iter().all(|a| prefs.contains_key(a)) {
                return Err(PrefsError::InvalidChoice(k.clone()));
            }
        }
        Ok(Self { prefs })
    }

    pub fn preferred_item(&self, v: V) -> V {
        self.prefs
            .get(&v)
            .and_then(|vp| vp.first().map(Clone::clone))
            .unwrap_or(v)
    }

    pub fn remove_prefs(&mut self, chosen: &Vec<V>) {
        for v in chosen {
            self.prefs.remove_entry(v);
        }
        for leftover_vs in self.prefs.values_mut() {
            leftover_vs.retain(|x| !chosen.contains(x))
        }
    }
}

impl<V> PreferenceGraph<V>
where
    V: PartialEq + Eq + Display + Hash + Copy + Debug,
{
    pub fn new(prefs: Preferences<V>) -> Result<Self, TTCError> {
        let v: Vec<V> = prefs.prefs.keys().cloned().collect();
        if v.is_empty() {
            return Err(TTCError::EmptyGraph);
        }
        let e: Vec<(V, V)> = v
            .iter()
            .map(|x| (x.clone(), prefs.preferred_item(*x)))
            .collect();

        let graph = Self::setup(&v, &e);
        Ok(Self { graph, prefs })
    }

    fn setup(v: &Vec<V>, e: &Vec<(V, V)>) -> DiGraph<V, ()> {
        let mut g = Graph::new();
        let mut m: HashMap<V, NodeIndex> = HashMap::new();

        for &node in v {
            let index = g.add_node(node);
            m.insert(node, index);
        }

        for &(v1, v2) in e {
            let n1 = m.get(&v1);
            let n2 = m.get(&v2);
            match (n1, n2) {
                (Some(_n1), Some(_n2)) => {
                    let _ = g.add_edge(*_n1, *_n2, ());
                }
                _ => (),
            }
        }
        g
    }

    /// https://www.cis.upenn.edu/~aaroth/courses/slides/agt17/lect11.pdf
    fn find_cycle(&self) -> Result<Cycle<V>, TTCError> {
        let mut predecessors = vec![NodeIndex::end(); self.graph.node_count()];
        let start = self.graph.node_indices().next();
        let cycle_end = depth_first_search(&self.graph, start, |event| match event {
            DfsEvent::TreeEdge(u, v) => {
                predecessors[v.index()] = u;
                return Control::Continue;
            }
            DfsEvent::BackEdge(u, v) => {
                predecessors[v.index()] = u;
                return Control::Break(u);
            }
            _ => return Control::Continue,
        })
        .break_value()
        .ok_or(TTCError::AlwaysCycles)?;

        let mut cycle = vec![self.graph[cycle_end]];
        let mut current_node = predecessors[cycle_end.index()];

        while current_node != cycle_end {
            let pred = predecessors[current_node.index()];
            cycle.push(self.graph[current_node]);
            current_node = pred;
        }
        cycle.reverse();
        Ok(Cycle { values: cycle })
    }

    pub fn solve_preferences(&mut self) -> Result<Solution<V>, TTCError> {
        let mut res = Vec::new();
        while self.graph.node_count() > 0 {
            let cycle = self.find_cycle()?;
            self.prefs.remove_prefs(&cycle.values);
            for v in &cycle.values {
                let ix = self
                    .graph
                    .node_indices()
                    .find(|&index| self.graph[index] == *v)
                    .unwrap();
                let mut edges_to_add = vec![];
                for n in self.graph.neighbors_directed(ix, Direction::Incoming) {
                    let preferred_item = self.prefs.preferred_item(self.graph[n]);
                    let preferred_ix = self
                        .graph
                        .node_indices()
                        .find(|&index| self.graph[index] == preferred_item)
                        .unwrap();
                    edges_to_add.push((n, preferred_ix));
                }
                for e in edges_to_add {
                    self.graph.add_edge(e.0, e.1, ());
                }
                self.graph.remove_node(ix);
            }
            res.push(cycle);
        }
        Ok(Solution { res })
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn basic_test() {
        let prefs = vec![
            ("S1", vec!["S3", "S2", "S4", "S1"]),
            ("S2", vec!["S3", "S5", "S6"]),
            ("S3", vec!["S3", "S1"]),
            ("S4", vec!["S2", "S5", "S6", "S4"]),
            ("S5", vec!["S1", "S3", "S2"]),
            ("S6", vec!["S2", "S4", "S5", "S6"]),
        ];
        let prefs = Preferences::new(prefs.into_iter().collect()).unwrap();

        let mut g = PreferenceGraph::new(prefs).unwrap();
        let ps = g.solve_preferences().unwrap().res;
        assert_eq!(
            vec![
                Cycle { values: vec!["S3"] },
                Cycle {
                    values: vec!["S1", "S2", "S5"]
                },
                Cycle {
                    values: vec!["S4", "S6"]
                }
            ],
            ps
        );
    }
}
