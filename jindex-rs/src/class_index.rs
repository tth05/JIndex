use std::ops::Range;
use std::time::Instant;

use ascii::{AsAsciiStr, AsciiChar, AsciiStr};
use rustc_hash::FxHashMap;

use crate::all_direct_super_types;
use crate::class_index_members::{IndexedClass, IndexedMethod};
use crate::constant_pool::{ClassIndexConstantPool, MatchMode, SearchMode, SearchOptions};
use crate::package_index::{IndexedPackage, PackageIndex};
use crate::rsplit_once;

pub struct ClassIndex {
    constant_pool: ClassIndexConstantPool,
    class_prefix_range_map: FxHashMap<u8, Range<u32>>,
    package_index: PackageIndex,
    classes: Vec<IndexedClass>,
}

impl ClassIndex {
    pub(crate) fn new(
        constant_pool: ClassIndexConstantPool,
        package_index: PackageIndex,
        classes: Vec<IndexedClass>,
    ) -> Self {
        //Construct prefix range map
        let mut prefix_count_map: FxHashMap<u8, u32> = FxHashMap::default();

        let time = Instant::now();
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
        println!("Sort {}", time.elapsed().as_millis());

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
        }
    }

    pub fn find_classes(&self, name: &AsciiStr, options: SearchOptions) -> Vec<&IndexedClass> {
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
    /// 3. Utilize find_package (which uses that new type) and then a binary search on the found package class_indices to make this whole find_class even faster
    /// For example, when searching for 'java/lang/S', we perform a binary search on a slice with 12k elements.
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
        self.classes
            .iter()
            .filter(|class| {
                if direct_sub_types_only {
                    class.is_direct_sub_type_of(index)
                } else {
                    let mut optional_parent = Some(*class);
                    while let Some(parent) = optional_parent {
                        if parent.is_direct_sub_type_of(index) {
                            return true;
                        }
                        optional_parent = parent
                            .signature()
                            .super_class()
                            .and_then(|s| s.extract_base_object_type())
                            .map(|i| self.class_at_index(i));
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
    ) -> Vec<(&IndexedClass, &IndexedMethod)> {
        self.find_implementations_of_class(defining_class_index, false)
            .iter()
            .flat_map(|class| {
                class
                    .methods()
                    .iter()
                    .filter(|m| m.overrides(target_method))
                    .map(|m| (*class, m))
            })
            .collect()
    }

    pub fn find_base_methods_of_method(
        &self,
        class: &IndexedClass,
        target_method: &IndexedMethod,
    ) -> Vec<(&IndexedClass, &IndexedMethod)> {
        // Check all super types of the given class
        all_direct_super_types!(class)
            .filter_map(|c| c.extract_base_object_type())
            .map(|i| self.class_at_index(i))
            .flat_map(|c| {
                c.methods()
                    .iter()
                    // Collect all methods from the current class
                    .filter(|m| target_method.overrides(m))
                    // We don't use `c` here directly to satisfy the borrow checker
                    .map(|m| (self.class_at_index(c.index()), m))
                    .chain(
                        // Recursively search super types of current class
                        self.find_base_methods_of_method(c, target_method)
                            .into_iter(),
                    )
            })
            .collect()
    }

    pub fn classes(&self) -> &Vec<IndexedClass> {
        &self.classes
    }

    pub fn package_index(&self) -> &PackageIndex {
        &self.package_index
    }

    pub fn constant_pool(&self) -> &ClassIndexConstantPool {
        &self.constant_pool
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
