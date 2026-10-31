use std::collections::BTreeMap;

pub(super) fn parents(records: &[Vec<u8>], header: usize) -> Vec<Option<usize>> {
    let mut groups: BTreeMap<u8, Vec<usize>> = BTreeMap::new();
    for (i, record) in records.iter().enumerate() {
        groups.entry(record[0]).or_default().push(i);
    }
    let mut parents = vec![None; records.len()];
    for group in groups.values() {
        let n = group.len();
        if n < 2 {
            continue;
        }
        let full = records[group[0]].len() as i32;
        let mut distance = vec![0_u8; n * n];
        let mut gain = vec![0_i32; n];
        for row in 0..n {
            for column in 0..n {
                let cost = if row == column {
                    full
                } else {
                    header as i32
                        + records[group[row]][1..]
                            .iter()
                            .zip(&records[group[column]][1..])
                            .filter(|(a, b)| a != b)
                            .count() as i32
                };
                distance[row * n + column] = cost as u8;
                gain[column] += (full - cost).max(0);
            }
        }
        let mut costs = vec![full; n];
        let mut selected = vec![false; n];
        loop {
            let best = (0..n)
                .filter(|&i| !selected[i])
                .max_by_key(|&i| (gain[i], std::cmp::Reverse(i)));
            let Some(best) = best.filter(|&i| gain[i] > 0) else {
                break;
            };
            selected[best] = true;
            for row in 0..n {
                if selected[row] && row != best {
                    continue;
                }
                let old = costs[row];
                let new = old.min(i32::from(distance[row * n + best]));
                if row != best && new == old {
                    continue;
                }
                for column in 0..n {
                    if selected[column] {
                        continue;
                    }
                    let cost = i32::from(distance[row * n + column]);
                    let before = (old - cost).max(0);
                    let after = if row == best { 0 } else { (new - cost).max(0) };
                    gain[column] -= before - after;
                }
                if row != best {
                    gain[row] += new - old;
                    costs[row] = new;
                }
            }
            costs[best] = full;
        }
        let fulls: Vec<_> = (0..n)
            .filter(|&i| selected[i] || costs[i] == full)
            .collect();
        for row in 0..n {
            if fulls.contains(&row) {
                continue;
            }
            let parent = fulls
                .iter()
                .copied()
                .min_by_key(|&column| (distance[row * n + column], column));
            if let Some(column) =
                parent.filter(|&column| i32::from(distance[row * n + column]) < full)
            {
                parents[group[row]] = Some(group[column]);
            }
        }
    }
    parents
}
