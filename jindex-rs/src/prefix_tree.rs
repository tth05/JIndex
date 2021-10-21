use crate::constant_pool::{ClassIndexConstantPool, ConstantPoolStringView};
use anyhow::anyhow;
use ascii::{AsAsciiStr, AsciiStr};

enum Node<T> {
    Normal(Normal<T>),
    Tail(Tail<T>),
}

struct Normal<T> {
    mask: u64,
    depth: u8,
    children: Vec<Node<T>>,
    values: Vec<T>,
}

impl<T> Normal<T> {
    fn new(depth: u8) -> Self {
        Self {
            mask: 0,
            depth,
            children: Vec::new(),
            values: Vec::new(),
        }
    }

    fn put(
        &mut self,
        constant_pool: &ClassIndexConstantPool,
        key: ConstantPoolStringView,
        value: T,
    ) {
        if key.is_empty() {
            self.values.push(value);
            return;
        }

        let current_byte = key.byte_at(constant_pool, 0);
        let index = self.get_index_for_byte(current_byte);

        let node;
        if !self.has_child_for_byte(current_byte) {
            node = self.insert_new_node(index, self.byte_as_mask_byte(current_byte));
        } else {
            node = self.children.get_mut(index as usize).unwrap();
        }

        let new_key = key.substring_to_end(1).unwrap();
        if let Node::Normal(ref mut normal) = node {
            normal.put(constant_pool, new_key, value);
        } else if let Node::Tail(ref mut tail) = node {
            tail.values.push((new_key, value));
        }
    }

    fn collect_all_starting_with<'a>(
        &'a self,
        constant_pool: &ClassIndexConstantPool,
        sequence: &AsciiStr,
        results: &mut Vec<&'a T>,
        limit: &mut u32,
    ) -> anyhow::Result<()> {
        fn collect_from_node<'a, T>(
            node: &'a Node<T>,
            constant_pool: &ClassIndexConstantPool,
            sequence: &AsciiStr,
            results: &mut Vec<&'a T>,
            limit: &mut u32,
        ) -> anyhow::Result<()> {
            if let Node::Normal(normal) = node {
                Ok(normal.collect_all_starting_with(constant_pool, sequence, results, limit)?)
            } else if let Node::Tail(tail) = node {
                Ok(tail.collect_all_starting_with(constant_pool, sequence, results, limit)?)
            } else {
                Err(anyhow!("Unknown node type"))
            }
        }

        if sequence.is_empty() {
            results.try_reserve(self.values.len())?;
            for t in &self.values {
                if *limit == 0 {
                    return Ok(());
                }

                results.push(t);
                *limit -= 1;
            }

            for child in &self.children {
                collect_from_node(child, constant_pool, sequence, results, limit)?;
            }
        } else {
            let first_byte = sequence.first().unwrap().as_byte();
            if self.has_child_for_byte(first_byte) {
                let node = self
                    .children
                    .get(self.get_index_for_byte(first_byte))
                    .unwrap();
                collect_from_node(
                    node,
                    constant_pool,
                    sequence.slice_ascii(1..).unwrap(),
                    results,
                    limit,
                )?;
            }
        }

        Ok(())
    }

    fn insert_new_node(&mut self, index: usize, mask_byte: u8) -> &mut Node<T> {
        if index == self.children.len() {
            self.children.push(self.create_new_node());
        } else {
            self.children.insert(index, self.create_new_node());
        }

        self.mask |= 1 << mask_byte;

        return self.children.get_mut(index).unwrap();
    }

    fn create_new_node(&self) -> Node<T> {
        match self.depth {
            1 => Node::Tail(Tail::new()),
            _ => Node::Normal(Normal::new(self.depth - 1)),
        }
    }

    fn has_child_for_byte(&self, byte: u8) -> bool {
        ((self.mask >> self.byte_as_mask_byte(byte)) & 1) != 0
    }

    fn get_index_for_byte(&self, byte: u8) -> usize {
        let mask_byte = self.byte_as_mask_byte(byte);
        if mask_byte == 0 {
            0
        } else {
            ((0xFFFFFFFFFFFFFFFF >> (64 - mask_byte)) & self.mask).count_ones() as usize
        }
    }

    fn byte_as_mask_byte(&self, byte: u8) -> u8 {
        if byte >= 97 && byte <= 122 {
            byte - 97
        } else if byte >= 65 && byte <= 90 {
            byte - 65 + 26
        } else if byte >= 48 && byte <= 57 {
            byte - 48 + 26 * 2
        } else if byte == 36 {
            26 * 2 + 10
        } else if byte == 95 {
            26 * 2 + 10 + 1
        } else {
            panic!("Invalid byte");
        }
    }
}

struct Tail<T> {
    values: Vec<(ConstantPoolStringView, T)>,
}

impl<T> Tail<T> {
    fn new() -> Self {
        Self { values: Vec::new() }
    }

    fn collect_all_starting_with<'a>(
        &'a self,
        constant_pool: &ClassIndexConstantPool,
        sequence: &AsciiStr,
        results: &mut Vec<&'a T>,
        limit: &mut u32,
    ) -> anyhow::Result<()> {
        println!("Hi");

        for (key, value) in &self.values {
            if *limit == 0 {
                return Ok(());
            }

            if key.starts_with(constant_pool, sequence) {
                results.push(value);
                *limit -= 1;
            }
        }

        Ok(())
    }
}

pub struct PrefixTree<T> {
    root_node: Node<T>,
}

impl<T> PrefixTree<T> {
    pub fn new(max_depth: u8) -> Self {
        Self {
            root_node: Node::Normal(Normal::new(max_depth)),
        }
    }

    pub fn put(
        &mut self,
        constant_pool: &ClassIndexConstantPool,
        str: ConstantPoolStringView,
        value: T,
    ) {
        if let Node::Normal(root) = &mut self.root_node {
            root.put(constant_pool, str, value);
        } else {
            panic!("Wrong root node");
        }
    }

    pub fn find_all_starting_with(
        &self,
        constant_pool: &ClassIndexConstantPool,
        sequence: &AsciiStr,
        limit: &mut u32,
    ) -> anyhow::Result<Vec<&T>> {
        let mut results = Vec::new();
        if let Node::Normal(root) = &self.root_node {
            root.collect_all_starting_with(constant_pool, sequence, &mut results, limit)?;
        } else {
            panic!("Wrong root node");
        }

        Ok(results)
    }
}
