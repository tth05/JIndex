use std::ops::Range;

use ascii::{AsAsciiStr, AsciiChar, AsciiStr};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::all_direct_super_types;
use crate::class_index_members::{IndexedClass, IndexedMethod};
use crate::constant_pool::{
    search_bytes, ClassIndexConstantPool, MatchMode, SearchMode, SearchOptions,
};
use crate::package_index::{IndexedPackage, PackageIndex};
use crate::rsplit_once;
use crate::semantic_index::SemanticIndex;
use crate::subtype_index::SubtypeIndex;

pub struct ClassIndex {
    constant_pool: ClassIndexConstantPool,
    class_prefix_range_map: FxHashMap<u8, Range<u32>>,
    package_index: PackageIndex,
    classes: Vec<IndexedClass>,
    semantic_index: SemanticIndex,
    subtypes: SubtypeIndex,
}

impl ClassIndex {
    pub(crate) fn new(
        constant_pool: ClassIndexConstantPool,
        mut package_index: PackageIndex,
        classes: Vec<IndexedClass>,
        semantic_index: SemanticIndex,
    ) -> Self {
        package_index.rebuild_full_names(&constant_pool);
        let subtypes = SubtypeIndex::new(
            classes.len(),
            classes.iter().flat_map(|class| {
                all_direct_super_types!(class)
                    .filter_map(|signature| signature.extract_base_object_type())
                    .map(|parent| (parent, class.index()))
            }),
        );
        //Construct prefix range map
        let mut prefix_count_map: FxHashMap<u8, u32> = FxHashMap::default();

        for class in classes.iter() {
            let first_byte = class
                .class_name_bytes(&constant_pool)
                .first()
                .copied()
                .expect("Class names are not empty");
            *prefix_count_map.entry(first_byte).or_insert(0) += 1;
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
            subtypes,
        }
    }

    pub fn find_classes(
        &self,
        name: &AsciiStr,
        options: SearchOptions,
        source_ids: Option<&[u32]>,
    ) -> (Vec<&IndexedClass>, bool) {
        if name.is_empty() {
            return (Vec::new(), false);
        }

        if name.chars().any(|character| character == AsciiChar::Slash) {
            let (package_name, class_name) = rsplit_once(name, AsciiChar::Slash);
            if class_name.is_empty() {
                return (Vec::new(), false);
            }
            let Some(package) = self.find_package(package_name) else {
                return (Vec::new(), false);
            };
            let class_indices = package.sub_classes_indices();
            return self.collect_class_matches(
                class_indices
                    .iter()
                    .map(|index| self.class_at_index(*index)),
                class_name,
                options,
                source_ids,
            );
        }

        match options.search_mode {
            SearchMode::Prefix => match options.match_mode {
                MatchMode::IgnoreCase => {
                    let lower = name.get_ascii(0).unwrap().to_ascii_lowercase().as_byte();
                    let upper = name.get_ascii(0).unwrap().to_ascii_uppercase().as_byte();
                    let first = self.class_iter_for_char(lower);
                    let second = if lower == upper {
                        &self.classes[0..0]
                    } else {
                        self.class_iter_for_char(upper)
                    };
                    self.collect_class_matches(
                        first.iter().chain(second.iter()),
                        name,
                        options,
                        source_ids,
                    )
                }
                MatchMode::MatchCase | MatchMode::MatchCaseFirstCharOnly => self
                    .collect_class_matches(
                        self.class_iter_for_char(name.get_ascii(0).unwrap().as_byte())
                            .iter(),
                        name,
                        options,
                        source_ids,
                    ),
            },
            SearchMode::Contains => {
                // A contains query cannot use the first-character ranges, so every class is a candidate.
                self.collect_class_matches(self.classes.iter(), name, options, source_ids)
            }
        }
    }

    fn collect_class_matches<'a>(
        &'a self,
        classes: impl Iterator<Item = &'a IndexedClass>,
        name: &AsciiStr,
        options: SearchOptions,
        source_ids: Option<&[u32]>,
    ) -> (Vec<&'a IndexedClass>, bool) {
        let probe_limit = options.limit.saturating_add(1);
        let mut matches = Vec::with_capacity(probe_limit.min(1_024));
        for class in classes {
            if !self.includes_source(class, source_ids) {
                continue;
            }
            let Some(position) = self
                .constant_pool
                .string_view_at(class.class_name_index())
                .search(&self.constant_pool, name, options)
            else {
                continue;
            };
            matches.push((position, class));
            if matches.len() == probe_limit {
                break;
            }
        }

        matches.sort_by_key(|(position, class)| (*position, class.index()));
        let truncated = matches.len() > options.limit;
        matches.truncate(options.limit);
        (
            matches.into_iter().map(|(_, class)| class).collect(),
            truncated,
        )
    }

    pub fn find_classes_by_binary_name(
        &self,
        name: &AsciiStr,
        options: SearchOptions,
        source_ids: Option<&[u32]>,
    ) -> (Vec<&IndexedClass>, bool) {
        if name.is_empty() {
            return (Vec::new(), false);
        }

        let mut matches = Vec::new();
        let mut binary_name = Vec::new();
        for class in &self.classes {
            if !self.includes_source(class, source_ids) {
                continue;
            }

            let package = self
                .package_index
                .package_at(class.package_index())
                .full_name()
                .as_bytes();
            binary_name.clear();
            binary_name.extend_from_slice(package);
            if !package.is_empty() {
                binary_name.push(b'/');
            }
            binary_name.extend_from_slice(class.class_name_bytes(&self.constant_pool));

            let Some(position) = search_bytes(&binary_name, name, options) else {
                continue;
            };
            matches.push((position, class));
        }

        let truncated = matches.len() > options.limit;
        matches.sort_by_key(|(position, class)| (*position, class.index()));
        matches.truncate(options.limit);
        (
            matches.into_iter().map(|(_, class)| class).collect(),
            truncated,
        )
    }

    fn includes_source(&self, class: &IndexedClass, source_ids: Option<&[u32]>) -> bool {
        source_ids.is_none_or(|selected| {
            selected
                .binary_search(&self.semantic_index.class_source_id(class.index()))
                .is_ok()
        })
    }

    /// Finds a class with an exact package and class name.
    ///
    /// Classes are sorted by `(class name, full package name)`, so the lookup binary-searches the
    /// first-character range of the class name.
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
            a.class_name_bytes(&self.constant_pool)
                .cmp(class_name.as_bytes())
                .then_with(|| {
                    self.package_index
                        .package_at(a.package_index())
                        .full_name()
                        .cmp(package_name)
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

    pub fn find_implementations_of_class(
        &self,
        index: u32,
        direct_sub_types_only: bool,
    ) -> Vec<&IndexedClass> {
        self.subtypes
            .implementations(index, direct_sub_types_only)
            .into_iter()
            .map(|index| self.class_at_index(index))
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
        let mut visited = FxHashSet::default();
        visited.insert(class.index());
        let mut queue = all_direct_super_types!(class)
            .filter_map(|signature| signature.extract_base_object_type())
            .collect::<Vec<_>>();
        let mut results = Vec::new();
        while let Some(index) = queue.pop() {
            if !visited.insert(index) {
                continue;
            }
            let ancestor = self.class_at_index(index);
            for method in ancestor.methods() {
                if self.method_overrides(class, target_method, ancestor, method) {
                    results.push(MethodWithClass {
                        class: ancestor,
                        method,
                    });
                }
            }
            queue.extend(
                all_direct_super_types!(ancestor)
                    .filter_map(|signature| signature.extract_base_object_type()),
            );
        }
        results.sort_by_key(|result| (result.class.index(), result.method.method_name_index()));
        results
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
