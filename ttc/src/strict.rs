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

#[derive(Debug, Clone)]
pub struct Cycle<V> {
    values: Vec<V>,
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

#[derive(Debug)]
pub struct Allocation<V> {
    pub allocation: HashMap<V, V>,
}

impl<V: Clone + Eq + Hash> From<Vec<Cycle<V>>> for Allocation<V> {
    fn from(cycles: Vec<Cycle<V>>) -> Self {
        let mut allocation = HashMap::new();
        cycles.into_iter().for_each(|cycle| {
            cycle
                .values
                .iter()
                .zip(cycle.values.iter().cycle().skip(1))
                .for_each(|(a, b)| {
                    allocation.insert(a.clone(), b.clone());
                });
        });
        Allocation { allocation }
    }
}

#[derive(Debug, Error)]
pub enum PrefsError<V: Display> {
    #[error("{} has preferences for options that don't exist", _0)]
    InvalidChoice(V),
}

#[derive(Debug, Clone)]
pub struct Preferences<V> {
    pub prefs: HashMap<V, Vec<V>>,
}

impl<V> Preferences<V> {
    pub fn participants(&self) -> Vec<&V> {
        self.prefs.keys().collect()
    }
}

impl<V: Eq + Hash> Preferences<V> {
    pub fn rank(&self, participant: V, value: V) -> Option<usize> {
        self.prefs
            .get(&participant)
            .map(|prefs| prefs.iter().position(|v| v == &value).unwrap_or(usize::MAX))
    }

    pub fn get(&self, v: &V) -> Option<&Vec<V>> {
        self.prefs.get(v)
    }

    pub fn map<F, W>(self, mut f: F) -> Preferences<W>
    where
        F: FnMut(V) -> W,
        W: Eq + Hash,
    {
        Preferences {
            prefs: self
                .prefs
                .into_iter()
                .map(|(k, vs)| (f(k), vs.into_iter().map(&mut f).collect()))
                .collect(),
        }
    }
}

impl<V> Preferences<V>
where
    V: Display + Eq + Hash + Clone,
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
            .and_then(|vp| vp.first().cloned())
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

#[derive(Debug, Error)]
pub enum TTCError {
    #[error("Graph is empty")]
    EmptyGraph,
    #[error("Graph contains an invalid edge: {}", _0)]
    InvalidEdge(String),
    #[error("Graph will always have a cycle")]
    AlwaysCycles,
}

pub struct PreferenceGraph<V> {
    graph: DiGraph<V, ()>,
    prefs: Preferences<V>,
}

impl<V> PreferenceGraph<V>
where
    V: Eq + Display + Hash + Copy,
{
    pub fn new(prefs: Preferences<V>) -> Result<Self, TTCError> {
        let v: Vec<V> = prefs.prefs.keys().cloned().collect();
        if v.is_empty() {
            return Err(TTCError::EmptyGraph);
        }
        let e: Vec<(V, V)> = v.iter().map(|x| (*x, prefs.preferred_item(*x))).collect();

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

            if let (Some(_n1), Some(_n2)) = (n1, n2) {
                let _ = g.add_edge(*_n1, *_n2, ());
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
                Control::Continue
            }
            DfsEvent::BackEdge(u, v) => {
                predecessors[v.index()] = u;
                Control::Break(u)
            }
            _ => Control::Continue,
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

    pub fn solve_preferences(&mut self) -> Result<Vec<Cycle<V>>, TTCError> {
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
        Ok(res)
    }
}

#[cfg(any(test, feature = "test"))]
pub mod test_utils {
    use std::collections::HashSet;

    use super::*;
    use proptest::prelude::*;

    impl<V> Arbitrary for Preferences<V>
    where
        V: Clone + Eq + std::hash::Hash + Arbitrary,
        V::Strategy: 'static,
    {
        type Parameters = Option<std::ops::RangeInclusive<usize>>;
        type Strategy = BoxedStrategy<Self>;

        fn arbitrary_with(params: Self::Parameters) -> Self::Strategy {
            let size_range = params.unwrap_or(2..=32);

            prop::collection::hash_set(any::<V>(), size_range)
                .prop_flat_map(|vertices| {
                    let vertices: Vec<V> = vertices.into_iter().collect();
                    let len = vertices.len();
                    let m = (3 * len) / 2;
                    // we include more indices in order to increase the likelihood of trades
                    prop::collection::vec(prop::collection::vec(0..len, 0..=m), len).prop_map(
                        move |subsets| Preferences {
                            prefs: vertices
                                .iter()
                                .zip(subsets)
                                .map(|(v, indices)| {
                                    let mut subset: Vec<V> =
                                        indices.into_iter().map(|i| vertices[i].clone()).collect();
                                    let mut seen = HashSet::new();
                                    subset.retain(|item| seen.insert(item.clone()));
                                    // it's technically legal to put this anywhere, but it really only makes sense to either
                                    // put it at the end or don't put it anywhere
                                    if let Some(idx) = subset.iter().position(|x| x == v) {
                                        subset.remove(idx);
                                        subset.push(v.clone());
                                    }
                                    (v.clone(), subset)
                                })
                                .collect(),
                        },
                    )
                })
                .boxed()
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use itertools::Itertools;
    use proptest::prelude::*;
    use std::collections::HashSet;

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
        let ps = g.solve_preferences().unwrap();
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

    impl<V: PartialEq> Cycle<V> {
        fn intersection(&self, other: &Self) -> Vec<&V> {
            self.values
                .iter()
                .filter(|v| other.values.contains(v))
                .collect()
        }

        fn elems(&self) -> Vec<&V> {
            self.values.iter().collect()
        }
    }

    fn should_exchange<V: Debug + Eq + Hash + Copy>(
        prefs: &Preferences<V>,
        alloc: &Allocation<V>,
        a: V,
        b: V,
    ) -> bool {
        let a_pref = prefs
            .rank(a, alloc.allocation[&a])
            .expect(format!("Failed to find key {:?}", a).as_str());
        let b_pref = prefs
            .rank(b, alloc.allocation[&b])
            .expect(format!("Failed to find key {:?}", b).as_str());
        let a_better = a_pref < b_pref;
        let b_better = b_pref < a_pref;
        a_better && b_better
    }

    proptest! {
    #[test]
    fn test_can_solve_random_graph(p in Preferences::<u32>::arbitrary())
      { let mut g = PreferenceGraph::new(p.clone()).unwrap();
        let solution = g.solve_preferences();
        let participants: HashSet<u32> = p.participants().into_iter().cloned().collect();

        // Check that the graph is solvable
        prop_assert!(solution.is_ok(), "Unsolvable graph");

        let cycles = solution.unwrap();

        // Check that the cycles are disjoint
        cycles.iter()
              .combinations(2)
              .try_for_each(|v| {
                let intersection = v[0].intersection(&v[1]);
                prop_assert!(intersection.is_empty(), "Cycles {:?} and {:?} intersect", v[0], v[1]);
                Ok(())
              })?;

        // Check that all participants are assigned
        {
            let mut assigned: HashSet<u32> = HashSet::new();
            for cycle in cycles.clone() {
                assigned.extend(cycle.elems());
            }
            prop_assert_eq!(participants.clone(), assigned, "Not all participants were assigned");
        }

        let alloc = Allocation::from(cycles);

        //Check that the allocation accounts for all of the preferences
        {
            let participants_allocated : HashSet<u32> = alloc.allocation.keys().cloned().collect();
            prop_assert_eq!(
                  participants
                , participants_allocated
                , "Allocations don't account for all of the preferences!"
            );
        }

        // check that the allocation respects the preferences
        alloc.allocation.iter().try_for_each(|(k,v)| {
            if k != v {
              let k_prefs = p.prefs.get(k).expect(format!("Failed to find key {:?} in preferences", k).as_str());
              prop_assert!(k_prefs.contains(v), "Preferences for {:?} don't contain {:?}", k, v);
            }
            Ok(())
        })?;

        // Check that the allocation is stable
        p.prefs.keys().combinations(2).try_for_each(|x| {
            let exchange = should_exchange(&p, &alloc, x[0].clone(), x[1].clone());
            prop_assert!(!exchange, "No exchange should increase satisfaction with allocation!");
            Ok(())

        })?;
      }
    }
}
