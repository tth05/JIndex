use std::hash::{Hash, Hasher};
use std::ops::Range;

use ascii::{AsAsciiStr, AsciiChar, AsciiStr};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::all_direct_super_types;
use crate::class_index_members::{IndexedClass, IndexedMethod};
use crate::constant_pool::{ClassIndexConstantPool, MatchMode, SearchMode, SearchOptions};
use crate::package_index::{IndexedPackage, PackageIndex};
use crate::rsplit_once;
use crate::semantic_index::SemanticIndex;

pub struct ClassIndex {
    constant_pool: ClassIndexConstantPool,
    class_prefix_range_map: FxHashMap<u8, Range<u32>>,
    package_index: PackageIndex,
    classes: Vec<IndexedClass>,
    semantic_index: SemanticIndex,
}

impl ClassIndex {
    pub(crate) fn new(
        constant_pool: ClassIndexConstantPool,
        package_index: PackageIndex,
        classes: Vec<IndexedClass>,
        semantic_index: SemanticIndex,
    ) -> Self {
        //Construct prefix range map
        let mut prefix_count_map: FxHashMap<u8, u32> = FxHashMap::default();

        for class in classes.iter() {
            let count = prefix_count_map
                .entry(
                    class
                        .class_name(&constant_pool)
                        .get_ascii(0)
                        .unwrap()
                        .as_byte(),
                )
                .or_insert(0);
            *count += 1;
        }

        let mut range_map: FxHashMap<u8, Range<u32>> = FxHashMap::default();
        let mut total = 0u32;
        for i in 0..=127u8 {
            let prefix_count = prefix_count_map.get(&i);
            if prefix_count.is_none() {
                continue;
            }

            let prefix_count = prefix_count.unwrap();
            range_map.insert(i, total..(total + prefix_count));
            total += prefix_count;
        }

        range_map.shrink_to_fit();
        Self {
            constant_pool,
            classes,
            package_index,
            class_prefix_range_map: range_map,
            semantic_index,
        }
    }

    pub fn find_classes(
        &self,
        name: &AsciiStr,
        options: SearchOptions,
        source_ids: Option<&[u32]>,
    ) -> Vec<&IndexedClass> {
        if name.is_empty() {
            return Vec::default();
        }

        let mut iters = Vec::with_capacity(2);
        match options.search_mode {
            SearchMode::Prefix => match options.match_mode {
                MatchMode::IgnoreCase => {
                    iters.push(self.class_iter_for_char(
                        name.get_ascii(0).unwrap().to_ascii_lowercase().as_byte(),
                    ));
                    iters.push(self.class_iter_for_char(
                        name.get_ascii(0).unwrap().to_ascii_uppercase().as_byte(),
                    ));
                }
                MatchMode::MatchCase | MatchMode::MatchCaseFirstCharOnly => {
                    iters.push(self.class_iter_for_char(name.get_ascii(0).unwrap().as_byte()));
                }
            },
            SearchMode::Contains => {
                //We have to search all classes in contains mode
                iters.push(&self.classes[..]);
            }
        }

        let mut result: Vec<(usize, &IndexedClass)> = Vec::new();

        for x in iters {
            let mut index = 0;
            x.iter()
                .filter(|class| {
                    source_ids.is_none_or(|source_ids| {
                        source_ids
                            .binary_search(&self.semantic_index().class_source_id(class.index()))
                            .is_ok()
                    })
                })
                .filter_map(|class| {
                    let result = self
                        .constant_pool()
                        .string_view_at(class.class_name_index())
                        .search(self.constant_pool(), name, options)
                        .map(|r| (r, class));

                    index += 1;
                    result
                })
                .take(options.limit.saturating_sub(result.len()))
                .for_each(|el| result.push(el))
        }

        result.sort_by_key(|el| el.0);
        result.into_iter().map(|el| el.1).collect()
    }

    ///TODO:
    /// 0. Benchmark if this could actually be faster
    /// 1. Abstract the prefix_range_map into its own type
    /// 2. Use that type to fast access all root packages
    /// 3. Utilize find_package (which uses that new type) and then a binary search on the found
    /// package class_indices to make this whole find_class even faster For example, when
    /// searching for 'java/lang/S', we perform a binary search on a slice with 12k elements.
    /// Instead we could find java/lang extremely fast and then binary search ~200 classes.
    pub fn find_class(
        &self,
        package_name: &AsciiStr,
        class_name: &AsciiStr,
    ) -> Option<&IndexedClass> {
        if class_name.is_empty() {
            return Option::None;
        }

        let class_iter = self.class_iter_for_char(class_name.get_ascii(0).unwrap().as_byte());

        let index = class_iter.binary_search_by(|a| {
            a.class_name(&self.constant_pool)
                .cmp(class_name)
                .then_with(|| {
                    self.package_index
                        .package_at(a.package_index())
                        .package_name_with_parents_cmp(
                            &self.package_index,
                            &self.constant_pool,
                            package_name,
                        )
                })
        });
        if let Ok(i) = index {
            return Some(class_iter.get(i).unwrap());
        }

        None
    }

    pub fn find_packages(&self, name: &AsciiStr) -> Vec<&IndexedPackage> {
        if name.is_empty() {
            return Vec::default();
        }

        let pool = self.constant_pool();
        let split_index = rsplit_once(name, AsciiChar::Slash);

        let base_package = if split_index.0.is_empty() {
            Some(self.package_index.package_at(0))
        } else {
            self.find_package(split_index.0)
        };

        match base_package {
            Some(p) => {
                let mut results = Vec::new();
                for sub_index in p.sub_packages_indices() {
                    let sub_package = self.package_index.package_at(*sub_index);
                    if pool
                        .string_view_at(sub_package.package_name_index())
                        .starts_with(pool, split_index.1, MatchMode::IgnoreCase)
                    {
                        results.push(sub_package);
                    }
                }

                results
            }
            None => Vec::default(),
        }
    }

    pub fn find_package(&self, name: &AsciiStr) -> Option<&IndexedPackage> {
        for sub_index in self.package_index.package_at(0).sub_packages_indices() {
            let result = self.find_package_starting_at(name, *sub_index);
            if result.is_some() {
                return result.map(|p| p.1);
            }
        }

        None
    }

    fn find_package_starting_at(
        &self,
        name: &AsciiStr,
        start_package_index: u32,
    ) -> Option<(u32, &IndexedPackage)> {
        let package = self.package_index.package_at(start_package_index);
        let split_index = name
            .chars()
            .position(|ch| ch == AsciiChar::Slash)
            .unwrap_or(name.len());
        let part = &name[0..split_index];

        if package.package_name(&self.constant_pool) != part {
            return None;
        }

        if split_index >= name.len() {
            //We've found it!
            Some((start_package_index, package))
        } else {
            let remaining_name = &name[split_index + 1..];
            for sub_index in package.sub_packages_indices() {
                let result = self.find_package_starting_at(remaining_name, *sub_index);
                if result.is_some() {
                    return result;
                }
            }

            None
        }
    }

    pub fn find_methods(
        &self,
        name: &AsciiStr,
        limit: usize,
    ) -> anyhow::Result<Vec<&IndexedMethod>> {
        let res = self
            .classes
            .iter()
            .flat_map(|class| class.methods())
            .filter(|method| {
                self.constant_pool()
                    .string_view_at(method.method_name_index())
                    .starts_with(self.constant_pool(), name, MatchMode::MatchCase)
            })
            .take(limit)
            .collect();
        Ok(res)
    }

    pub fn find_implementations_of_class(
        &self,
        index: u32,
        direct_sub_types_only: bool,
    ) -> Vec<&IndexedClass> {
        let mut queue = Vec::new();
        self.classes
            .iter()
            .filter(|class| {
                if direct_sub_types_only {
                    class.is_direct_sub_type_of(index)
                } else {
                    queue.push(*class);
                    while let Some(current) = queue.pop() {
                        if current.is_direct_sub_type_of(index) {
                            return true;
                        }

                        if let Some(super_class) = current
                            .signature()
                            .super_class()
                            .and_then(|s| s.extract_base_object_type())
                            .map(|i| self.class_at_index(i))
                        {
                            queue.push(super_class);
                        }

                        if let Some(interfaces) =
                            current.signature().interfaces().map(|interfaces| {
                                interfaces.iter().filter_map(|i| {
                                    i.extract_base_object_type().map(|i| self.class_at_index(i))
                                })
                            })
                        {
                            interfaces.for_each(|i| queue.push(i));
                        }
                    }

                    false
                }
            })
            .collect()
    }

    pub fn find_implementations_of_method<'b>(
        &'b self,
        defining_class_index: u32,
        target_method: &'b IndexedMethod,
    ) -> Vec<(&'b IndexedClass, &'b IndexedMethod)> {
        let defining_class = self.class_at_index(defining_class_index);
        self.find_implementations_of_class(defining_class_index, false)
            .iter()
            .flat_map(|class| {
                class
                    .methods()
                    .iter()
                    .filter(|method| {
                        self.method_overrides(class, method, defining_class, target_method)
                    })
                    .map(|m| (*class, m))
            })
            .collect()
    }

    pub fn summarize_hierarchy_of_class(
        &self,
        class_index: u32,
    ) -> (usize, Vec<usize>, Vec<usize>) {
        let target_class = self.class_at_index(class_index);
        let implementations = self.find_implementations_of_class(class_index, false);
        let mut method_implementation_counts = vec![0; target_class.methods().len()];

        for implementation in &implementations {
            for candidate in implementation.methods() {
                for (target_index, target) in target_class.methods().iter().enumerate() {
                    if self.method_overrides(implementation, candidate, target_class, target) {
                        method_implementation_counts[target_index] += 1;
                    }
                }
            }
        }

        let method_base_counts = target_class
            .methods()
            .iter()
            .map(|method| self.find_base_methods_of_method(target_class, method).len())
            .collect();

        (
            implementations.len(),
            method_implementation_counts,
            method_base_counts,
        )
    }

    pub fn find_base_methods_of_method<'a>(
        &'a self,
        class: &'a IndexedClass,
        target_method: &'a IndexedMethod,
    ) -> Vec<MethodWithClass<'a>> {
        self.find_base_methods_starting_at(class, class, target_method)
    }

    fn find_base_methods_starting_at<'a>(
        &'a self,
        current_class: &'a IndexedClass,
        declaring_class: &'a IndexedClass,
        target_method: &'a IndexedMethod,
    ) -> Vec<MethodWithClass<'a>> {
        // Check all super types of the given class
        all_direct_super_types!(current_class)
            .filter_map(|c| c.extract_base_object_type())
            .map(|i| self.class_at_index(i))
            .flat_map(|c| {
                c.methods()
                    .iter()
                    // Collect all methods from the current class
                    .filter(|method| {
                        self.method_overrides(declaring_class, target_method, c, method)
                    })
                    // We don't use `c` here directly to satisfy the borrow checker
                    .map(|m| MethodWithClass {
                        class: self.class_at_index(c.index()),
                        method: m,
                    })
                    .chain(
                        // Recursively search super types of current class
                        self.find_base_methods_starting_at(c, declaring_class, target_method)
                            .into_iter(),
                    )
            })
            .collect::<FxHashSet<MethodWithClass>>() // Remove duplicates
            .into_iter()
            .collect()
    }

    fn method_overrides(
        &self,
        candidate_class: &IndexedClass,
        candidate_method: &IndexedMethod,
        base_class: &IndexedClass,
        base_method: &IndexedMethod,
    ) -> bool {
        const ACC_PUBLIC: u16 = 0x0001;
        const ACC_PRIVATE: u16 = 0x0002;
        const ACC_PROTECTED: u16 = 0x0004;
        let base_is_package_private =
            base_method.access_flags() & (ACC_PUBLIC | ACC_PRIVATE | ACC_PROTECTED) == 0;
        (!base_is_package_private || candidate_class.package_index() == base_class.package_index())
            && candidate_method.overrides(base_method, &self.constant_pool)
    }

    pub fn classes(&self) -> &Vec<IndexedClass> {
        &self.classes
    }

    pub fn class_count(&self) -> usize {
        self.classes.len()
    }

    pub fn package_index(&self) -> &PackageIndex {
        &self.package_index
    }

    pub fn constant_pool(&self) -> &ClassIndexConstantPool {
        &self.constant_pool
    }

    pub fn semantic_index(&self) -> &SemanticIndex {
        &self.semantic_index
    }

    pub fn class_at_index(&self, index: u32) -> &IndexedClass {
        self.classes().get(index as usize).unwrap()
    }

    fn class_iter_for_char(&self, char: u8) -> &[IndexedClass] {
        self.class_prefix_range_map.get(&char).map_or_else(
            || &self.classes[0..0],
            |r| &self.classes[r.start as usize..r.end as usize],
        )
    }
}

pub struct MethodWithClass<'a> {
    pub class: &'a IndexedClass,
    pub method: &'a IndexedMethod,
}

impl PartialEq<Self> for MethodWithClass<'_> {
    fn eq(&self, other: &Self) -> bool {
        // NOTE: This implementation ignores method overloading, but it's fine for our use case
        self.class.index() == other.class.index()
            && self.method.method_name_index() == other.method.method_name_index()
    }
}

impl Eq for MethodWithClass<'_> {}

impl Hash for MethodWithClass<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.class.index().hash(state);
        self.method.method_name_index().hash(state);
    }
}
