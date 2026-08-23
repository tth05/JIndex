use crate::class_index::ClassIndex;
use crate::class_index_members::IndexedClass;
use crate::constant_pool::{ClassIndexConstantPool, MatchMode, SearchMode, SearchOptions};
use anyhow::{anyhow, ensure};
use ascii::AsciiStr;
use speedy::{Readable, Writable};
use std::cmp::Ordering;

const SYMBOL_KIND_SHIFT: u64 = 62;
const SYMBOL_ORDINAL_MASK: u64 = (1 << SYMBOL_KIND_SHIFT) - 1;
const REFERENCE_SITE_KIND_SHIFT: u64 = 62;
const REFERENCE_SITE_ORDINAL_SHIFT: u64 = 32;
const REFERENCE_SITE_ORDINAL_MASK: u64 = (1 << 30) - 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SymbolKind {
    Class = 0,
    Field = 1,
    Method = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SymbolId(u64);

impl SymbolId {
    pub fn new(kind: SymbolKind, ordinal: u64) -> anyhow::Result<Self> {
        ensure!(
            ordinal <= SYMBOL_ORDINAL_MASK,
            "Symbol ordinal {} exceeds the supported range",
            ordinal
        );
        Ok(Self(((kind as u64) << SYMBOL_KIND_SHIFT) | ordinal))
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }

    pub fn from_u64(value: u64) -> anyhow::Result<Self> {
        let kind = value >> SYMBOL_KIND_SHIFT;
        ensure!(
            kind <= SymbolKind::Method as u64,
            "Unknown symbol kind {kind}"
        );
        Ok(Self(value))
    }

    pub fn kind(self) -> SymbolKind {
        match self.0 >> SYMBOL_KIND_SHIFT {
            0 => SymbolKind::Class,
            1 => SymbolKind::Field,
            2 => SymbolKind::Method,
            _ => unreachable!("Symbol IDs are validated when constructed"),
        }
    }

    pub fn ordinal(self) -> u64 {
        self.0 & SYMBOL_ORDINAL_MASK
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ReferenceSiteKind {
    Class = 0,
    Field = 1,
    Method = 2,
    RecordComponent = 3,
}

#[derive(Clone, Copy, Debug, Readable, Writable)]
pub struct PackedReferenceSite(u64);

impl PackedReferenceSite {
    pub(crate) fn new(kind: u8, ordinal: u32, occurrence_count: u32) -> anyhow::Result<Self> {
        ensure!(kind <= 3, "Reference-site storage kind exceeds 2 bits");
        ensure!(
            u64::from(ordinal) <= REFERENCE_SITE_ORDINAL_MASK,
            "Reference-site ordinal exceeds 30 bits"
        );
        ensure!(
            occurrence_count > 0,
            "Reference occurrence count must be positive"
        );
        Ok(Self(
            (u64::from(kind) << REFERENCE_SITE_KIND_SHIFT)
                | (u64::from(ordinal) << REFERENCE_SITE_ORDINAL_SHIFT)
                | u64::from(occurrence_count),
        ))
    }

    pub fn storage_kind(self) -> u8 {
        (self.0 >> REFERENCE_SITE_KIND_SHIFT) as u8
    }

    pub fn ordinal(self) -> u32 {
        ((self.0 >> REFERENCE_SITE_ORDINAL_SHIFT) & REFERENCE_SITE_ORDINAL_MASK) as u32
    }

    pub fn occurrence_count(self) -> u32 {
        self.0 as u32
    }

    pub(crate) fn identity(self) -> u32 {
        (self.0 >> REFERENCE_SITE_ORDINAL_SHIFT) as u32
    }
}

#[derive(Clone, Copy, Debug, Readable, Writable)]
pub struct ExtraReferenceSite {
    pub owner_class_index: u32,
    pub kind: u8,
    pub name_offset: u32,
    pub descriptor_offset: u32,
}

#[derive(Default, Readable, Writable)]
pub struct Utf8Pool {
    data: Vec<u8>,
}

impl Utf8Pool {
    pub(crate) fn add(&mut self, value: &str) -> anyhow::Result<u32> {
        let length = u16::try_from(value.len())
            .map_err(|_| anyhow!("UTF-8 pool value exceeds 65535 bytes"))?;
        let offset =
            u32::try_from(self.data.len()).map_err(|_| anyhow!("UTF-8 pool exceeds 4 GiB"))?;
        self.data.extend_from_slice(&length.to_le_bytes());
        self.data.extend_from_slice(value.as_bytes());
        Ok(offset)
    }

    pub fn get(&self, offset: u32) -> &str {
        let offset = offset as usize;
        let length = u16::from_le_bytes([self.data[offset], self.data[offset + 1]]) as usize;
        std::str::from_utf8(&self.data[offset + 2..offset + 2 + length])
            .expect("Persisted UTF-8 pool contains invalid data")
    }
}

#[derive(Default)]
pub(crate) struct ReferenceIndexData {
    pub extra_site_names: Utf8Pool,
    pub extra_sites: Vec<ExtraReferenceSite>,
    pub offsets: Vec<u32>,
    pub sites: Vec<PackedReferenceSite>,
    pub literal_offsets: Vec<u32>,
    pub literal_values: Vec<u16>,
    pub literal_posting_offsets: Vec<u32>,
    pub literal_sites: Vec<PackedReferenceSite>,
}

#[derive(Clone, Copy, Debug, Readable, Writable)]
pub struct PackedMemberId(u64);

impl PackedMemberId {
    pub fn new(class_index: u32, member_index: usize) -> anyhow::Result<Self> {
        let member_index = u16::try_from(member_index)
            .map_err(|_| anyhow!("A class has more than 65535 members of one kind"))?;
        Ok(Self(
            (u64::from(class_index) << 16) | u64::from(member_index),
        ))
    }

    pub fn class_index(self) -> u32 {
        (self.0 >> 16) as u32
    }

    pub fn member_index(self) -> u16 {
        self.0 as u16
    }
}

#[derive(Default, Readable, Writable)]
pub struct DescriptorPool {
    data: Vec<u8>,
}

impl DescriptorPool {
    pub fn add(&mut self, descriptor: &str) -> anyhow::Result<u32> {
        ensure!(descriptor.is_ascii(), "JVM descriptor is not ASCII");
        let length = u16::try_from(descriptor.len())
            .map_err(|_| anyhow!("JVM descriptor exceeds 65535 bytes"))?;
        let offset =
            u32::try_from(self.data.len()).map_err(|_| anyhow!("Descriptor pool exceeds 4 GiB"))?;
        self.data.reserve(2 + descriptor.len());
        self.data.extend_from_slice(&length.to_le_bytes());
        self.data.extend_from_slice(descriptor.as_bytes());
        Ok(offset)
    }

    pub fn get(&self, offset: u32) -> &AsciiStr {
        let offset = offset as usize;
        let length = u16::from_le_bytes([
            *self.data.get(offset).expect("Invalid descriptor offset"),
            *self
                .data
                .get(offset + 1)
                .expect("Invalid descriptor length"),
        ]) as usize;
        let bytes = &self.data[offset + 2..offset + 2 + length];
        unsafe { AsciiStr::from_ascii_unchecked(bytes) }
    }
}

#[derive(Default, Readable, Writable)]
pub struct SemanticIndex {
    class_source_ids: Vec<u32>,
    descriptor_pool: DescriptorPool,
    field_offsets: Vec<u32>,
    method_offsets: Vec<u32>,
    field_descriptors: Vec<u32>,
    method_descriptors: Vec<u32>,
    field_search: Vec<PackedMemberId>,
    method_search: Vec<PackedMemberId>,
    extra_site_names: Utf8Pool,
    extra_sites: Vec<ExtraReferenceSite>,
    reference_offsets: Vec<u32>,
    reference_sites: Vec<PackedReferenceSite>,
    literal_offsets: Vec<u32>,
    literal_values: Vec<u16>,
    literal_posting_offsets: Vec<u32>,
    literal_sites: Vec<PackedReferenceSite>,
}

impl SemanticIndex {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        class_source_ids: Vec<u32>,
        descriptor_pool: DescriptorPool,
        field_offsets: Vec<u32>,
        method_offsets: Vec<u32>,
        field_descriptors: Vec<u32>,
        method_descriptors: Vec<u32>,
        field_search: Vec<PackedMemberId>,
        method_search: Vec<PackedMemberId>,
        references: ReferenceIndexData,
    ) -> Self {
        Self {
            class_source_ids,
            descriptor_pool,
            field_offsets,
            method_offsets,
            field_descriptors,
            method_descriptors,
            field_search,
            method_search,
            extra_site_names: references.extra_site_names,
            extra_sites: references.extra_sites,
            reference_offsets: references.offsets,
            reference_sites: references.sites,
            literal_offsets: references.literal_offsets,
            literal_values: references.literal_values,
            literal_posting_offsets: references.literal_posting_offsets,
            literal_sites: references.literal_sites,
        }
    }

    pub fn class_source_id(&self, class_index: u32) -> u32 {
        self.class_source_ids[class_index as usize]
    }

    pub fn field_count(&self) -> usize {
        self.field_descriptors.len()
    }

    pub fn method_count(&self) -> usize {
        self.method_descriptors.len()
    }

    pub fn reference_site_count(&self) -> usize {
        self.reference_sites.len()
    }

    pub fn literal_count(&self) -> usize {
        self.literal_offsets.len().saturating_sub(1)
    }

    pub fn literal_occurrence_count(&self) -> u64 {
        self.literal_sites
            .iter()
            .map(|site| u64::from(site.occurrence_count()))
            .sum()
    }

    pub fn find_literal(&self, value: &[u16]) -> Option<u32> {
        let mut low = 0_usize;
        let mut high = self.literal_count();
        while low < high {
            let middle = low + (high - low) / 2;
            match self.literal(middle as u32).cmp(value) {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => return Some(middle as u32),
            }
        }
        None
    }

    pub fn literal(&self, literal: u32) -> &[u16] {
        let start = self.literal_offsets[literal as usize] as usize;
        let end = self.literal_offsets[literal as usize + 1] as usize;
        &self.literal_values[start..end]
    }

    pub fn literal_references(&self, literal: u32) -> &[PackedReferenceSite] {
        let start = self.literal_posting_offsets[literal as usize] as usize;
        let end = self.literal_posting_offsets[literal as usize + 1] as usize;
        &self.literal_sites[start..end]
    }

    pub fn find_literals_containing(&self, query: &[u16], limit: usize) -> (Vec<u32>, bool) {
        if query.is_empty() || limit == 0 {
            return (Vec::new(), false);
        }
        let mut matches = Vec::with_capacity(limit.min(256));
        for literal in 0..self.literal_count() {
            if !self
                .literal(literal as u32)
                .windows(query.len())
                .any(|window| window == query)
            {
                continue;
            }
            if matches.len() == limit {
                return (matches, true);
            }
            matches.push(literal as u32);
        }
        (matches, false)
    }

    pub fn references_to(&self, target: SymbolId) -> anyhow::Result<&[PackedReferenceSite]> {
        let target_index = self.target_storage_index(target)?;
        let start = self.reference_offsets[target_index] as usize;
        let end = self.reference_offsets[target_index + 1] as usize;
        Ok(&self.reference_sites[start..end])
    }

    pub fn extra_site(&self, ordinal: u32) -> &ExtraReferenceSite {
        &self.extra_sites[ordinal as usize]
    }

    pub fn extra_site_name(&self, site: &ExtraReferenceSite) -> &str {
        self.extra_site_names.get(site.name_offset)
    }

    pub fn extra_site_descriptor(&self, site: &ExtraReferenceSite) -> &AsciiStr {
        self.descriptor_pool.get(site.descriptor_offset)
    }

    pub fn member_from_ordinal(
        &self,
        kind: SymbolKind,
        ordinal: u32,
    ) -> anyhow::Result<PackedMemberId> {
        let offsets = match kind {
            SymbolKind::Field => &self.field_offsets,
            SymbolKind::Method => &self.method_offsets,
            SymbolKind::Class => return Err(anyhow!("Classes are not member ordinals")),
        };
        let total = *offsets.last().unwrap_or(&0);
        ensure!(ordinal < total, "Member ordinal is out of range");
        let class_index = offsets.partition_point(|offset| *offset <= ordinal) - 1;
        PackedMemberId::new(
            u32::try_from(class_index)?,
            (ordinal - offsets[class_index]) as usize,
        )
    }

    fn target_storage_index(&self, target: SymbolId) -> anyhow::Result<usize> {
        let ordinal = usize::try_from(target.ordinal())?;
        let class_count = self.class_source_ids.len();
        match target.kind() {
            SymbolKind::Class => {
                ensure!(
                    ordinal < class_count,
                    "Class symbol ordinal is out of range"
                );
                Ok(ordinal)
            }
            SymbolKind::Field => {
                ensure!(
                    ordinal < self.field_count(),
                    "Field symbol ordinal is out of range"
                );
                Ok(class_count + ordinal)
            }
            SymbolKind::Method => {
                ensure!(
                    ordinal < self.method_count(),
                    "Method symbol ordinal is out of range"
                );
                Ok(class_count + self.field_count() + ordinal)
            }
        }
    }

    pub fn descriptor(&self, kind: SymbolKind, class_index: u32, member_index: u16) -> &AsciiStr {
        let (offsets, descriptors) = match kind {
            SymbolKind::Field => (&self.field_offsets, &self.field_descriptors),
            SymbolKind::Method => (&self.method_offsets, &self.method_descriptors),
            SymbolKind::Class => panic!("Classes do not have JVM descriptors"),
        };
        let global_index = offsets[class_index as usize] as usize + member_index as usize;
        self.descriptor_pool.get(descriptors[global_index])
    }

    pub fn symbol_id(
        &self,
        kind: SymbolKind,
        class_index: u32,
        member_index: u16,
    ) -> anyhow::Result<SymbolId> {
        let ordinal = match kind {
            SymbolKind::Class => u64::from(class_index),
            SymbolKind::Field => {
                u64::from(self.field_offsets[class_index as usize]) + u64::from(member_index)
            }
            SymbolKind::Method => {
                u64::from(self.method_offsets[class_index as usize]) + u64::from(member_index)
            }
        };
        SymbolId::new(kind, ordinal)
    }

    pub fn find_members(
        &self,
        class_index: &ClassIndex,
        query: &AsciiStr,
        options: SearchOptions,
        include_fields: bool,
        include_methods: bool,
    ) -> Vec<MemberSearchResult> {
        if query.is_empty() || options.limit == 0 {
            return Vec::new();
        }

        let included_kind_count = usize::from(include_fields) + usize::from(include_methods);
        let mut results =
            Vec::with_capacity(options.limit.min(256).saturating_mul(included_kind_count));
        if include_fields {
            self.collect_matches(
                class_index,
                query,
                options,
                SymbolKind::Field,
                &self.field_search,
                &mut results,
            );
        }
        if include_methods {
            self.collect_matches(
                class_index,
                query,
                options,
                SymbolKind::Method,
                &self.method_search,
                &mut results,
            );
        }

        results.sort_unstable_by(|left, right| {
            left.match_offset
                .cmp(&right.match_offset)
                .then_with(|| compare_member_names(class_index, *left, *right))
                .then_with(|| (left.kind as u8).cmp(&(right.kind as u8)))
                .then_with(|| left.member.class_index().cmp(&right.member.class_index()))
                .then_with(|| left.member.member_index().cmp(&right.member.member_index()))
        });
        results.truncate(options.limit);
        results
    }

    fn collect_matches(
        &self,
        class_index: &ClassIndex,
        query: &AsciiStr,
        options: SearchOptions,
        kind: SymbolKind,
        members: &[PackedMemberId],
        output: &mut Vec<MemberSearchResult>,
    ) {
        if matches!(options.search_mode, SearchMode::Contains) {
            self.collect_contains_matches(class_index, query, options, kind, output);
            return;
        }

        let start = members.partition_point(|member| {
            compare_ascii_folded(member_name(class_index, kind, *member), query).is_lt()
        });
        let mut match_count = 0;
        for member in &members[start..] {
            let name = member_name(class_index, kind, *member);
            if !starts_with_ascii_ignore_case(name, query) {
                break;
            }
            if let Some(match_offset) = search_ascii(name, query, options) {
                output.push(MemberSearchResult {
                    kind,
                    member: *member,
                    match_offset,
                });
                match_count += 1;
                if match_count >= options.limit {
                    break;
                }
            }
        }
    }

    fn collect_contains_matches(
        &self,
        class_index: &ClassIndex,
        query: &AsciiStr,
        options: SearchOptions,
        kind: SymbolKind,
        output: &mut Vec<MemberSearchResult>,
    ) {
        let constant_pool = class_index.constant_pool();
        for (class_ordinal, class) in class_index.classes().iter().enumerate() {
            match kind {
                SymbolKind::Field => {
                    for (member_ordinal, field) in class.fields().iter().enumerate() {
                        let name = field.field_name(constant_pool);
                        if let Some(match_offset) = search_ascii(name, query, options) {
                            output.push(MemberSearchResult {
                                kind,
                                member: PackedMemberId::new(class_ordinal as u32, member_ordinal)
                                    .expect("Indexed field no longer fits its stored identifier"),
                                match_offset,
                            });
                        }
                    }
                }
                SymbolKind::Method => {
                    for (member_ordinal, method) in class.methods().iter().enumerate() {
                        let name = method.method_name(constant_pool);
                        if let Some(match_offset) = search_ascii(name, query, options) {
                            output.push(MemberSearchResult {
                                kind,
                                member: PackedMemberId::new(class_ordinal as u32, member_ordinal)
                                    .expect("Indexed method no longer fits its stored identifier"),
                                match_offset,
                            });
                        }
                    }
                }
                SymbolKind::Class => panic!("Class is not a member kind"),
            }
        }
    }
}

pub(crate) fn sort_members_for_search(
    classes: &[IndexedClass],
    constant_pool: &ClassIndexConstantPool,
    kind: SymbolKind,
    members: &mut [PackedMemberId],
) {
    members.sort_unstable_by(|left, right| {
        let left_name = member_name_from_parts(classes, constant_pool, kind, *left);
        let right_name = member_name_from_parts(classes, constant_pool, kind, *right);
        compare_ascii_folded(left_name, right_name)
            .then_with(|| left_name.cmp(right_name))
            .then_with(|| left.class_index().cmp(&right.class_index()))
            .then_with(|| left.member_index().cmp(&right.member_index()))
    });
}

#[derive(Clone, Copy, Debug)]
pub struct MemberSearchResult {
    pub kind: SymbolKind,
    pub member: PackedMemberId,
    pub match_offset: usize,
}

fn member_name<'a>(
    index: &'a ClassIndex,
    kind: SymbolKind,
    member: PackedMemberId,
) -> &'a AsciiStr {
    member_name_from_parts(index.classes(), index.constant_pool(), kind, member)
}

fn member_name_from_parts<'a>(
    classes: &'a [IndexedClass],
    constant_pool: &'a ClassIndexConstantPool,
    kind: SymbolKind,
    member: PackedMemberId,
) -> &'a AsciiStr {
    let class = &classes[member.class_index() as usize];
    match kind {
        SymbolKind::Field => {
            class.fields()[member.member_index() as usize].field_name(constant_pool)
        }
        SymbolKind::Method => {
            class.methods()[member.member_index() as usize].method_name(constant_pool)
        }
        SymbolKind::Class => panic!("Class is not a member kind"),
    }
}

fn compare_member_names(
    index: &ClassIndex,
    left: MemberSearchResult,
    right: MemberSearchResult,
) -> Ordering {
    let left_name = member_name(index, left.kind, left.member);
    let right_name = member_name(index, right.kind, right.member);
    compare_ascii_folded(left_name, right_name).then_with(|| left_name.cmp(right_name))
}

fn compare_ascii_folded(left: &AsciiStr, right: &AsciiStr) -> Ordering {
    left.as_bytes()
        .iter()
        .map(u8::to_ascii_lowercase)
        .cmp(right.as_bytes().iter().map(u8::to_ascii_lowercase))
}

fn starts_with_ascii_ignore_case(value: &AsciiStr, prefix: &AsciiStr) -> bool {
    value.len() >= prefix.len()
        && value.as_bytes()[..prefix.len()]
            .iter()
            .zip(prefix.as_bytes())
            .all(|(actual, expected)| actual.eq_ignore_ascii_case(expected))
}

fn search_ascii(value: &AsciiStr, query: &AsciiStr, options: SearchOptions) -> Option<usize> {
    if query.len() > value.len() {
        return None;
    }
    let last_start = match options.search_mode {
        SearchMode::Prefix => 0,
        SearchMode::Contains => value.len() - query.len(),
    };
    for start in 0..=last_start {
        let matches = value.as_bytes()[start..start + query.len()]
            .iter()
            .zip(query.as_bytes().iter())
            .enumerate()
            .all(|(offset, (actual, expected))| match options.match_mode {
                MatchMode::MatchCase => actual == expected,
                MatchMode::IgnoreCase => actual.eq_ignore_ascii_case(expected),
                MatchMode::MatchCaseFirstCharOnly if start == 0 && offset == 0 => {
                    actual == expected
                }
                MatchMode::MatchCaseFirstCharOnly => actual.eq_ignore_ascii_case(expected),
            });
        if matches {
            return Some(start);
        }
        if matches!(options.search_mode, SearchMode::Prefix) {
            break;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_member_id_is_eight_bytes() {
        assert_eq!(std::mem::size_of::<PackedMemberId>(), 8);
    }

    #[test]
    fn descriptor_pool_accepts_descriptors_longer_than_legacy_names() {
        let descriptor = format!("({})V", "Ljava/lang/String;".repeat(20));
        let mut pool = DescriptorPool::default();
        let offset = pool.add(&descriptor).unwrap();
        assert_eq!(pool.get(offset).as_str(), descriptor);
    }

    #[test]
    fn symbol_ids_are_tagged_without_losing_the_ordinal() {
        let id = SymbolId::new(SymbolKind::Method, 123_456).unwrap();
        assert_eq!(id.as_u64(), (2_u64 << SYMBOL_KIND_SHIFT) | 123_456);
    }
}
