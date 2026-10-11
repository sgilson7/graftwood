//! The path through the woods: floors of nodes, each joined to one or two
//! nodes on the floor above, rolled from the run's seed. The first floor is
//! all battles, the floor before the last is all rests, and the last is the
//! boss alone.

use crate::content::RunRules;
use crate::rng::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Battle,
    Elite,
    Encounter,
    Shop,
    Rest,
    Boss,
}

impl Kind {
    pub fn key(self) -> &'static str {
        match self {
            Kind::Battle => "battle",
            Kind::Elite => "elite",
            Kind::Encounter => "encounter",
            Kind::Shop => "shop",
            Kind::Rest => "rest",
            Kind::Boss => "boss",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub floor: usize,
    /// 0 to columns − 1, across the path
    pub col: i32,
    pub kind: Kind,
    /// nodes on the next floor this one leads to
    pub next: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Map {
    pub nodes: Vec<Node>,
    pub floors: Vec<Vec<usize>>,
}

fn roll_kind(r: &RunRules, floor: usize, rng: &mut Rng) -> Kind {
    let w = &r.weights;
    let elite = if floor >= r.elite_from_floor { w.elite } else { 0 };
    let rest = if floor >= r.rest_from_floor { w.rest } else { 0 };
    let table = [
        (Kind::Battle, w.battle),
        (Kind::Encounter, w.encounter),
        (Kind::Shop, w.shop),
        (Kind::Elite, elite),
        (Kind::Rest, rest),
    ];
    let total: i32 = table.iter().map(|t| t.1).sum();
    let mut roll = rng.range(0, total.max(1) - 1);
    for (k, weight) in table {
        if roll < weight {
            return k;
        }
        roll -= weight;
    }
    Kind::Battle
}

impl Map {
    pub fn generate(r: &RunRules, rng: &mut Rng) -> Map {
        let mut nodes: Vec<Node> = Vec::new();
        let mut floors: Vec<Vec<usize>> = Vec::new();
        let last = r.floors - 1;
        for floor in 0..r.floors {
            let mut cols: Vec<i32> = if floor == last {
                vec![(r.columns - 1) / 2]
            } else {
                let n = rng.range(r.min_nodes, r.max_nodes.min(r.columns));
                let mut all: Vec<i32> = (0..r.columns).collect();
                // a partial shuffle picks n distinct columns
                for i in 0..n as usize {
                    let j = rng.range(i as i32, r.columns - 1) as usize;
                    all.swap(i, j);
                }
                all.truncate(n as usize);
                all
            };
            cols.sort();
            let mut ids = Vec::new();
            for col in cols {
                let kind = if floor == last {
                    Kind::Boss
                } else if floor == 0 {
                    Kind::Battle
                } else if floor == last - 1 {
                    Kind::Rest
                } else {
                    roll_kind(r, floor, rng)
                };
                ids.push(nodes.len());
                nodes.push(Node {
                    floor,
                    col,
                    kind,
                    next: Vec::new(),
                });
            }
            floors.push(ids);
        }
        // join each floor to the next: every node to its nearest column above,
        // and every node above that nothing reaches from its nearest below
        for floor in 0..last {
            let (below, above) = (floors[floor].clone(), floors[floor + 1].clone());
            let nearest = |from: i32, among: &[usize], nodes: &[Node]| -> usize {
                *among
                    .iter()
                    .min_by_key(|&&k| ((nodes[k].col - from).abs(), nodes[k].col))
                    .unwrap_or(&among[0])
            };
            for &b in &below {
                let to = nearest(nodes[b].col, &above, &nodes);
                nodes[b].next.push(to);
            }
            for &a in &above {
                if !below.iter().any(|&b| nodes[b].next.contains(&a)) {
                    let from = nearest(nodes[a].col, &below, &nodes);
                    nodes[from].next.push(a);
                }
            }
            // a second way up from some nodes, so choices branch
            for &b in &below {
                if rng.chance(40) {
                    let col = nodes[b].col;
                    if let Some(&extra) = above
                        .iter()
                        .filter(|&&a| (nodes[a].col - col).abs() == 1 && !nodes[b].next.contains(&a))
                        .min_by_key(|&&a| nodes[a].col)
                    {
                        nodes[b].next.push(extra);
                    }
                }
            }
            for &b in &below {
                let mut n = nodes[b].next.clone();
                n.sort_by_key(|&k| nodes[k].col);
                n.dedup();
                nodes[b].next = n;
            }
        }
        Map { nodes, floors }
    }

    /// The nodes the player may go to next from `at` (the start: floor one).
    pub fn choices(&self, at: Option<usize>) -> Vec<usize> {
        match at {
            None => self.floors[0].clone(),
            Some(k) => self.nodes[k].next.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::tests::content;

    #[test]
    fn every_node_is_reachable_and_every_node_but_the_boss_leads_on() {
        let c = content();
        for seed in 0..200 {
            let m = Map::generate(&c.rules, &mut Rng::new(seed));
            let last = c.rules.floors - 1;
            for (k, n) in m.nodes.iter().enumerate() {
                if n.floor < last {
                    assert!(!n.next.is_empty(), "seed {seed}: node {k} leads nowhere");
                    for &t in &n.next {
                        assert_eq!(m.nodes[t].floor, n.floor + 1);
                    }
                }
                if n.floor > 0 {
                    assert!(
                        m.nodes.iter().any(|p| p.next.contains(&k)),
                        "seed {seed}: node {k} unreachable"
                    );
                }
            }
        }
    }

    #[test]
    fn the_floors_have_their_fixed_kinds() {
        let c = content();
        let m = Map::generate(&c.rules, &mut Rng::new(5));
        let n = c.rules.floors;
        assert_eq!(m.floors.len(), n);
        assert!(m.floors[0].iter().all(|&k| m.nodes[k].kind == Kind::Battle));
        assert!(m.floors[n - 2].iter().all(|&k| m.nodes[k].kind == Kind::Rest));
        assert_eq!(m.floors[n - 1].len(), 1);
        assert_eq!(m.nodes[m.floors[n - 1][0]].kind, Kind::Boss);
    }

    #[test]
    fn no_elite_or_rest_comes_before_its_floor() {
        let c = content();
        for seed in 0..100 {
            let m = Map::generate(&c.rules, &mut Rng::new(seed));
            for n in &m.nodes {
                if n.kind == Kind::Elite {
                    assert!(n.floor >= c.rules.elite_from_floor);
                }
                if n.kind == Kind::Rest {
                    assert!(n.floor >= c.rules.rest_from_floor);
                }
            }
        }
    }

    #[test]
    fn some_paths_branch() {
        let c = content();
        let m = Map::generate(&c.rules, &mut Rng::new(11));
        assert!(m.nodes.iter().any(|n| n.next.len() > 1));
    }
}
