use crate::constant_pool::{ClassIndexConstantPool, ConstantPoolStringView};
use anyhow::anyhow;
use ascii::{AsAsciiStr, AsciiStr};
use speedy::{Context, Readable, Reader, Writable, Writer};

#[derive(Readable, Writable)]
enum Node<T> {
    Normal(Normal<T>),
    Tail(Tail<T>),
}

struct Normal<T> {
    //TODO: Change to [u8]
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
            panic!("Invalid byte {}", byte);
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
        for (key, value) in &self.values {
            if *limit == 0 {
                return Ok(());
            }

            if sequence.len() <= key.len() as usize
                && key.starts_with(constant_pool, sequence, false)
            {
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

/*
Serialization
*/

impl<'a, C, Q> Readable<'a, C> for PrefixTree<Q>
where
    C: Context,
    Q: Readable<'a, C>,
{
    fn read_from<R: Reader<'a, C>>(reader: &mut R) -> Result<Self, C::Error> {
        Ok(PrefixTree {
            root_node: Node::read_from(reader)?,
        })
    }
}

impl<C, Q> Writable<C> for PrefixTree<Q>
where
    C: Context,
    Q: Writable<C>,
{
    fn write_to<T: ?Sized + Writer<C>>(&self, writer: &mut T) -> Result<(), C::Error> {
        self.root_node.write_to(writer)?;
        Ok(())
    }
}

impl<'a, C, Q> Readable<'a, C> for Tail<Q>
where
    C: Context,
    Q: Readable<'a, C>,
{
    fn read_from<R: Reader<'a, C>>(reader: &mut R) -> Result<Self, C::Error> {
        let length = reader.read_u32()?;
        let mut values = Vec::with_capacity(length as usize);

        for _ in 0..length {
            values.push((
                ConstantPoolStringView::new(
                    reader.read_u32()?,
                    reader.read_u8()?,
                    reader.read_u8()?,
                ),
                reader.read_value()?,
            ));
        }
        Ok(Tail { values })
    }
}

impl<C, Q> Writable<C> for Tail<Q>
where
    C: Context,
    Q: Writable<C>,
{
    fn write_to<T: ?Sized + Writer<C>>(&self, writer: &mut T) -> Result<(), C::Error> {
        writer.write_u32(self.values.len() as u32)?;
        for (view, value) in &self.values {
            writer.write_u32(view.index())?;
            writer.write_u8(view.start())?;
            writer.write_u8(view.end())?;
            writer.write_value(value)?;
        }
        Ok(())
    }
}

impl<'a, C, Q> Readable<'a, C> for Normal<Q>
where
    C: Context,
    Q: Readable<'a, C>,
{
    fn read_from<R: Reader<'a, C>>(reader: &mut R) -> Result<Self, C::Error> {
        let mask = reader.read_u64()?;
        let depth = reader.read_u8()?;

        let length = reader.read_u32()? as usize;
        let children: Vec<Node<Q>> = reader.read_vec(length)?;

        let length = reader.read_u32()? as usize;
        let values: Vec<Q> = reader.read_vec(length)?;

        Ok(Normal {
            mask,
            depth,
            children,
            values,
        })
    }
}

impl<C, Q> Writable<C> for Normal<Q>
where
    C: Context,
    Q: Writable<C>,
{
    fn write_to<T: ?Sized + Writer<C>>(&self, writer: &mut T) -> Result<(), C::Error> {
        writer.write_u64(self.mask)?;
        writer.write_u8(self.depth)?;

        writer.write_u32(self.children.len() as u32)?;
        writer.write_collection(&self.children)?;

        writer.write_u32(self.values.len() as u32)?;
        writer.write_collection(&self.values)?;
        Ok(())
    }
}
