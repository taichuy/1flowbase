use super::node_depends_on;
use crate::_tests::graph_algorithms::node;
use std::collections::BTreeMap;

#[test]
fn borrowed_reachability_matches_non_reflexive_transitive_closure() {
    let mut checks = 0;
    for n in 0_usize..=4 {
        for bits in 0..(1_u64 << (n * n)) {
            let nodes: BTreeMap<_, _> = (0..n)
                .map(|i| {
                    let mut node = node(&i.to_string());
                    node.dependency_node_ids = (0..n)
                        .filter(|j| bits & (1 << (i * n + j)) != 0)
                        .map(|j| j.to_string())
                        .collect();
                    (node.node_id.clone(), node)
                })
                .collect();
            // Independent Floyd-Warshall oracle. A node reaches itself only through
            // a non-empty cycle; missing nodes are included in every query matrix.
            let mut closure = vec![vec![false; n]; n];
            for (i, row) in closure.iter_mut().enumerate() {
                for (j, reachable) in row.iter_mut().enumerate() {
                    *reachable = bits & (1 << (i * n + j)) != 0;
                }
            }
            for k in 0..n {
                for i in 0..n {
                    for j in 0..n {
                        closure[i][j] |= closure[i][k] && closure[k][j];
                    }
                }
            }
            let missing_row = vec![false; n];
            for (i, row) in closure
                .iter()
                .chain(std::iter::once(&missing_row))
                .enumerate()
            {
                for (j, reachable) in row
                    .iter()
                    .copied()
                    .chain(std::iter::once(false))
                    .enumerate()
                {
                    assert_eq!(
                        node_depends_on(&nodes, &i.to_string(), &j.to_string()),
                        reachable,
                        "n={n} edges={bits:b} query={i}->{j}",
                    );
                    checks += 1;
                }
            }
        }
    }
    assert_eq!(checks, 1_646_745);
}
