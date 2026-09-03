// Derived reverse adjacency. Rebuilt after loading; not part of the snapshot payload.
pub(crate) struct SubtypeIndex {
    offsets: Vec<usize>,
    children: Vec<u32>,
}

impl SubtypeIndex {
    pub(crate) fn new(class_count: usize, edges: impl Iterator<Item = (u32, u32)> + Clone) -> Self {
        let mut offsets = vec![0; class_count + 1];
        for (parent, _) in edges.clone() {
            offsets[parent as usize + 1] += 1;
        }
        for index in 1..offsets.len() {
            offsets[index] += offsets[index - 1];
        }
        let mut children = vec![0; offsets[class_count]];
        let mut write_positions = offsets[..class_count].to_vec();
        for (parent, child) in edges {
            children[write_positions[parent as usize]] = child;
            write_positions[parent as usize] += 1;
        }
        Self { offsets, children }
    }

    pub(crate) fn implementations(&self, target: u32, direct_only: bool) -> Vec<u32> {
        let mut visited = vec![false; self.offsets.len() - 1];
        visited[target as usize] = true;
        let mut queue = vec![target];
        let mut results = Vec::new();
        while let Some(parent) = queue.pop() {
            for child in
                &self.children[self.offsets[parent as usize]..self.offsets[parent as usize + 1]]
            {
                if std::mem::replace(&mut visited[*child as usize], true) {
                    continue;
                }
                results.push(*child);
                if !direct_only {
                    queue.push(*child);
                }
            }
        }
        results.sort_unstable();
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverse_queries_match_forward_walks_including_cycles_and_diamonds() {
        let mut seed = 0x5eed_u64;
        for _ in 0..256 {
            let mut parents = vec![Vec::new(); 24];
            let mut edges = Vec::new();
            for child in 0..24_u32 {
                for parent in 0..24_u32 {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    if seed >> 60 == 0 {
                        parents[child as usize].push(parent);
                        edges.push((parent, child));
                    }
                }
            }
            let index = SubtypeIndex::new(24, edges.iter().copied());
            for target in 0..24_u32 {
                for direct_only in [true, false] {
                    let expected = (0..24_u32)
                        .filter(|candidate| {
                            if *candidate == target {
                                return false;
                            }
                            let mut queue = parents[*candidate as usize].clone();
                            let mut visited = [false; 24];
                            while let Some(parent) = queue.pop() {
                                if parent == target {
                                    return true;
                                }
                                if direct_only
                                    || std::mem::replace(&mut visited[parent as usize], true)
                                {
                                    continue;
                                }
                                queue.extend(&parents[parent as usize]);
                            }
                            false
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(expected, index.implementations(target, direct_only));
                }
            }
        }
    }
}
