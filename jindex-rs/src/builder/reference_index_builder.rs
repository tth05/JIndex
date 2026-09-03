use super::raw_references::{
    RawExtraSiteKind, RawReferenceData, RawReferenceMetadata, RawReferenceSite, RawReferenceTarget,
};
use super::{ClassInfo, ClassToIndexMap};
use crate::semantic_index::{
    DescriptorPool, ExtraReferenceSite, PackedLiteralSite, PackedReferenceSite, ReferenceIndexData,
    ReferenceSiteKind, Utf8Pool,
};
use anyhow::anyhow;
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};

#[cfg(test)]
mod audit_oracle;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum TargetKey {
    Class(u32),
    Field(u32, u32, u32),
    Method(u32, u32, u32),
}

enum ResolvedTarget {
    None,
    One(u32),
    Many(Box<[u32]>),
}

impl ResolvedTarget {
    fn as_slice(&self) -> &[u32] {
        match self {
            Self::None => &[],
            Self::One(target) => std::slice::from_ref(target),
            Self::Many(targets) => targets,
        }
    }
}

#[derive(Default)]
struct ResolutionScratch {
    visited: FxHashSet<u32>,
    interfaces: Vec<u32>,
    results: Vec<u32>,
}

struct TargetResolver<'a> {
    fields: &'a [MemberLookupEntry],
    methods: &'a [MemberLookupEntry],
    hierarchy: &'a [HierarchyInfo],
    constructor_name: Option<u32>,
    class_count: u32,
    field_count: u32,
}

impl TargetResolver<'_> {
    fn resolve(&self, key: TargetKey, scratch: &mut ResolutionScratch) -> ResolvedTarget {
        scratch.visited.clear();
        match key {
            TargetKey::Class(index) => ResolvedTarget::One(index),
            TargetKey::Field(owner, name, descriptor) => resolve_field_from(
                owner,
                name,
                descriptor,
                self.fields,
                self.hierarchy,
                &mut scratch.visited,
            )
            .map_or(ResolvedTarget::None, |index| {
                ResolvedTarget::One(self.class_count + index)
            }),
            TargetKey::Method(owner, name, descriptor) => {
                let base = self.class_count + self.field_count;
                if Some(name) == self.constructor_name {
                    return find_member(self.methods, owner, name, descriptor)
                        .map_or(ResolvedTarget::None, |index| {
                            ResolvedTarget::One(base + index)
                        });
                }
                scratch.interfaces.clear();
                let mut current = Some(owner);
                while let Some(class) = current {
                    if !scratch.visited.insert(class) {
                        break;
                    }
                    if let Some(method) = find_member(self.methods, class, name, descriptor) {
                        return ResolvedTarget::One(base + method);
                    }
                    scratch
                        .interfaces
                        .extend_from_slice(&self.hierarchy[class as usize].interfaces);
                    current = self.hierarchy[class as usize].super_class;
                }
                scratch.visited.clear();
                scratch.results.clear();
                for interface in &scratch.interfaces {
                    collect_interface_methods(
                        *interface,
                        name,
                        descriptor,
                        self.methods,
                        self.hierarchy,
                        &mut scratch.visited,
                        &mut scratch.results,
                    );
                }
                scratch.results.sort_unstable();
                scratch.results.dedup();
                match scratch.results.as_slice() {
                    [] => ResolvedTarget::None,
                    [method] => ResolvedTarget::One(base + method),
                    methods => {
                        ResolvedTarget::Many(methods.iter().map(|index| base + index).collect())
                    }
                }
            }
        }
    }
}

fn target_key(
    target: RawReferenceTarget,
    references: &RawReferenceData,
    classes: &ClassToIndexMap<'_>,
    names: &FxHashMap<&str, u32>,
    descriptors: &FxHashMap<&str, u32>,
) -> Option<TargetKey> {
    match target {
        RawReferenceTarget::Class { name } => {
            class_index(classes, references.string(name)).map(TargetKey::Class)
        }
        RawReferenceTarget::Field {
            owner,
            name,
            descriptor,
        } => Some(TargetKey::Field(
            class_index(classes, references.string(owner))?,
            *names.get(references.string(name))?,
            *descriptors.get(references.string(descriptor))?,
        )),
        RawReferenceTarget::Method {
            owner,
            name,
            descriptor,
        } => Some(TargetKey::Method(
            class_index(classes, references.string(owner))?,
            *names.get(references.string(name))?,
            *descriptors.get(references.string(descriptor))?,
        )),
    }
}

#[derive(Clone, Copy)]
struct MemberLookupEntry {
    owner: u32,
    name: u32,
    descriptor: u32,
    ordinal: u32,
}

#[derive(Default)]
struct HierarchyInfo {
    super_class: Option<u32>,
    interfaces: Vec<u32>,
}

#[derive(Clone, Copy)]
struct ResolvedReference {
    target: u32,
    site_identity: u32,
    metadata: RawReferenceMetadata,
}

#[derive(Clone, Copy)]
struct ResolvedLiteralReference {
    target: u32,
    site_identity: u32,
    occurrence_count: u32,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_reference_index<'a>(
    class_infos: &'a [ClassInfo],
    raw_references: Vec<RawReferenceData>,
    classes: &ClassToIndexMap<'a>,
    constant_pool_map: &FxHashMap<&'a str, u32>,
    descriptor_pool_map: &FxHashMap<&'a str, u32>,
    descriptor_pool: &mut DescriptorPool,
    field_offsets: &[u32],
    method_offsets: &[u32],
) -> anyhow::Result<ReferenceIndexData> {
    let class_count = class_infos.len();
    let mut class_infos_by_index = vec![None; class_count];
    let mut references_by_index: Vec<Option<RawReferenceData>> =
        (0..class_count).map(|_| None).collect();
    if raw_references.len() != class_count {
        return Err(anyhow!("Raw reference data does not match the class count"));
    }
    for (class_info, references) in class_infos.iter().zip(raw_references) {
        let class_index = classes
            .get(&(
                class_info.package_name.as_str(),
                class_info.class_name.as_str(),
            ))
            .map(|(index, _)| *index)
            .ok_or_else(|| anyhow!("Selected class is missing from the class lookup"))?;
        class_infos_by_index[class_index as usize] = Some(class_info);
        references_by_index[class_index as usize] = Some(references);
    }
    let class_infos_by_index = class_infos_by_index
        .into_iter()
        .map(|info| info.ok_or_else(|| anyhow!("Class lookup has no selected class info")))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let references_by_index = references_by_index
        .into_iter()
        .map(|references| {
            references.ok_or_else(|| anyhow!("Class lookup has no raw reference data"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let hierarchy = build_hierarchy(&class_infos_by_index, classes);
    let mut field_lookup = Vec::with_capacity(*field_offsets.last().unwrap_or(&0) as usize);
    let mut method_lookup = Vec::with_capacity(*method_offsets.last().unwrap_or(&0) as usize);
    for (owner, class_info) in class_infos_by_index.iter().enumerate() {
        for (member_index, field) in class_info.fields.iter().enumerate() {
            field_lookup.push(MemberLookupEntry {
                owner: owner as u32,
                name: *constant_pool_map
                    .get(field.field_name.as_str())
                    .expect("Indexed field name is missing from the constant pool map"),
                descriptor: *descriptor_pool_map
                    .get(field.jvm_descriptor.as_str())
                    .expect("Indexed field descriptor is missing from the descriptor pool map"),
                ordinal: field_offsets[owner] + member_index as u32,
            });
        }
        for (member_index, method) in class_info.methods.iter().enumerate() {
            method_lookup.push(MemberLookupEntry {
                owner: owner as u32,
                name: *constant_pool_map
                    .get(method.method_name.as_str())
                    .expect("Indexed method name is missing from the constant pool map"),
                descriptor: *descriptor_pool_map
                    .get(method.jvm_descriptor.as_str())
                    .expect("Indexed method descriptor is missing from the descriptor pool map"),
                ordinal: method_offsets[owner] + member_index as u32,
            });
        }
    }
    field_lookup.sort_unstable_by_key(member_key);
    method_lookup.sort_unstable_by_key(member_key);

    let mut extra_site_names = Utf8Pool::default();
    let mut extra_descriptor_map = FxHashMap::default();
    let mut extra_sites = Vec::new();
    let mut extra_site_offsets = Vec::with_capacity(class_count + 1);
    extra_site_offsets.push(0_u32);
    for (owner, references) in references_by_index.iter().enumerate() {
        for raw_site in &references.extra_sites {
            let name = references.string(raw_site.name);
            let descriptor = references.string(raw_site.descriptor);
            let descriptor_offset = if let Some(offset) = descriptor_pool_map.get(descriptor) {
                *offset
            } else if let Some(offset) = extra_descriptor_map.get(descriptor) {
                *offset
            } else {
                let offset = descriptor_pool.add(descriptor)?;
                extra_descriptor_map.insert(descriptor.to_owned(), offset);
                offset
            };
            extra_sites.push(ExtraReferenceSite {
                owner_class_index: owner as u32,
                kind: match raw_site.kind {
                    RawExtraSiteKind::Field => ReferenceSiteKind::Field as u8,
                    RawExtraSiteKind::Method => ReferenceSiteKind::Method as u8,
                    RawExtraSiteKind::RecordComponent => ReferenceSiteKind::RecordComponent as u8,
                },
                name_offset: extra_site_names.add(name)?,
                descriptor_offset,
            });
        }
        extra_site_offsets.push(
            u32::try_from(extra_sites.len())
                .map_err(|_| anyhow!("The index contains more than 4294967295 extra sites"))?,
        );
    }

    let field_count = *field_offsets.last().unwrap_or(&0);
    let method_count = *method_offsets.last().unwrap_or(&0);
    let total_targets = u32::try_from(class_count)?
        .checked_add(field_count)
        .and_then(|value| value.checked_add(method_count))
        .ok_or_else(|| anyhow!("The reference target table exceeds 4294967295 symbols"))?;
    let class_count = u32::try_from(class_count)?;
    let resolver = TargetResolver {
        fields: &field_lookup,
        methods: &method_lookup,
        hierarchy: &hierarchy,
        constructor_name: constant_pool_map.get("<init>").copied(),
        class_count,
        field_count,
    };
    // Normalize while the builder's !Sync class references are confined to this thread.
    // Rayon sees only immutable IDs, lookup tables and raw reference data.
    let mut keys = Vec::new();
    let mut key_ids = FxHashMap::default();
    let mut local_target_ids = Vec::with_capacity(references_by_index.len());
    for references in &references_by_index {
        let mut ids = Vec::with_capacity(references.targets.len());
        for target in &references.targets {
            let id = if let Some(key) = target_key(
                *target,
                references,
                classes,
                constant_pool_map,
                descriptor_pool_map,
            ) {
                Some(if let Some(id) = key_ids.get(&key) {
                    *id
                } else {
                    let id = u32::try_from(keys.len())
                        .map_err(|_| anyhow!("More than 4294967295 unique reference targets"))?;
                    keys.push(key);
                    key_ids.insert(key, id);
                    id
                })
            } else {
                None
            };
            ids.push(id);
        }
        local_target_ids.push(ids);
    }
    drop(key_ids);
    let resolved_targets: Vec<_> = keys
        .par_iter()
        .map_init(ResolutionScratch::default, |scratch, key| {
            resolver.resolve(*key, scratch)
        })
        .collect();
    drop(keys);

    let raw_literal_edge_count = references_by_index
        .iter()
        .map(|references| references.literal_edges.len())
        .sum();
    let mut literal_ids: FxHashMap<Vec<u16>, u32> = FxHashMap::default();
    let mut resolved_literals = Vec::with_capacity(raw_literal_edge_count);
    let mut references_by_index = references_by_index;
    for (owner, references) in references_by_index.iter_mut().enumerate() {
        let mut local_literal_ids = Vec::with_capacity(references.literals.len());
        for value in std::mem::take(&mut references.literals) {
            let literal_id = if let Some(id) = literal_ids.get(value.as_slice()) {
                *id
            } else {
                let id = u32::try_from(literal_ids.len())
                    .map_err(|_| anyhow!("The index contains more than 4294967295 literals"))?;
                literal_ids.insert(value, id);
                id
            };
            local_literal_ids.push(literal_id);
        }
        for edge in &references.literal_edges {
            let site_identity = resolve_source_site_identity(
                edge.site,
                owner as u32,
                field_offsets,
                method_offsets,
                &extra_site_offsets,
            )?;
            resolved_literals.push(ResolvedLiteralReference {
                target: local_literal_ids[edge.literal as usize],
                site_identity,
                occurrence_count: edge.occurrence_count,
            });
        }
    }

    #[cfg(test)]
    let expected = references_by_index
        .iter()
        .enumerate()
        .map(|(owner, references)| {
            let mut output = Vec::new();
            audit_oracle::visit_resolved_references(
                owner as u32,
                references,
                classes,
                constant_pool_map,
                descriptor_pool_map,
                &field_lookup,
                &method_lookup,
                &hierarchy,
                field_offsets,
                method_offsets,
                &extra_site_offsets,
                class_count,
                field_count,
                |target, site_identity, metadata| {
                    output.push((
                        target,
                        site_identity,
                        metadata.relation_mask(),
                        metadata.occurrence_count(),
                    ));
                    Ok(())
                },
            )
            .unwrap();
            output
        })
        .collect::<Vec<_>>();

    let resolved = references_by_index
        .into_par_iter()
        .zip(local_target_ids)
        .enumerate()
        .map(|(owner, (references, local_ids))| {
            let mut output = Vec::with_capacity(references.edges.len());
            for edge in references.edges {
                let Some(id) = local_ids[edge.target as usize] else {
                    continue;
                };
                let targets = resolved_targets[id as usize].as_slice();
                if targets.is_empty() {
                    continue;
                }
                let site_identity = resolve_source_site_identity(
                    edge.site,
                    owner as u32,
                    field_offsets,
                    method_offsets,
                    &extra_site_offsets,
                )?;
                output.extend(targets.iter().map(|target| ResolvedReference {
                    target: *target,
                    site_identity,
                    metadata: edge.metadata,
                }));
            }
            Ok(output)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    drop(resolved_targets);

    #[cfg(test)]
    for (owner, (actual, expected)) in resolved.iter().zip(expected).enumerate() {
        let actual = actual
            .iter()
            .map(|reference| {
                (
                    reference.target,
                    reference.site_identity,
                    reference.metadata.relation_mask(),
                    reference.metadata.occurrence_count(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            actual, expected,
            "Cached/parallel references differ from the per-edge oracle for class {owner}"
        );
    }

    let mut offsets = vec![0_u32; total_targets as usize + 1];
    let mut resolved_count = 0;
    for class_references in &resolved {
        resolved_count += class_references.len();
        for reference in class_references {
            offsets[reference.target as usize + 1] = offsets[reference.target as usize + 1]
                .checked_add(1)
                .ok_or_else(|| anyhow!("A symbol has more than 4294967295 reference sites"))?;
        }
    }
    let (reference_offsets, reference_sites) =
        build_reference_postings(offsets, resolved.into_iter().flatten(), resolved_count)?;
    let literal_count = literal_ids.len();
    let mut literal_values_by_id = literal_ids.into_iter().collect::<Vec<_>>();
    literal_values_by_id.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    let mut literal_remap = vec![0_u32; literal_count];
    for (new_id, (_, old_id)) in literal_values_by_id.iter().enumerate() {
        literal_remap[*old_id as usize] = new_id as u32;
    }
    for reference in &mut resolved_literals {
        reference.target = literal_remap[reference.target as usize];
    }
    let mut literal_offsets = Vec::with_capacity(literal_count + 1);
    let mut literal_values = Vec::new();
    literal_offsets.push(0_u32);
    for (value, _) in literal_values_by_id {
        literal_values.extend(value);
        literal_offsets.push(
            u32::try_from(literal_values.len())
                .map_err(|_| anyhow!("The literal pool exceeds 4294967295 UTF-16 code units"))?,
        );
    }

    let mut literal_posting_counts = vec![0_u32; literal_count + 1];
    for reference in &resolved_literals {
        let count = &mut literal_posting_counts[reference.target as usize + 1];
        *count = count
            .checked_add(1)
            .ok_or_else(|| anyhow!("A literal has more than 4294967295 reference sites"))?;
    }
    let (literal_posting_offsets, literal_sites) =
        build_literal_postings(literal_posting_counts, resolved_literals)?;

    Ok(ReferenceIndexData {
        extra_site_names,
        extra_sites,
        offsets: reference_offsets,
        sites: reference_sites,
        literal_offsets,
        literal_values,
        literal_posting_offsets,
        literal_sites,
    })
}

fn build_reference_postings(
    mut counts: Vec<u32>,
    resolved: impl IntoIterator<Item = ResolvedReference>,
    resolved_count: usize,
) -> anyhow::Result<(Vec<u32>, Vec<PackedReferenceSite>)> {
    for index in 1..counts.len() {
        counts[index] = counts[index]
            .checked_add(counts[index - 1])
            .ok_or_else(|| anyhow!("The index has more than 4294967295 reference sites"))?;
    }

    let placeholder = PackedReferenceSite::new(0, 0, 1, 1)?;
    let mut sites = vec![placeholder; resolved_count];
    let mut write_positions = counts[..counts.len() - 1].to_vec();
    for reference in resolved {
        let position = &mut write_positions[reference.target as usize];
        sites[*position as usize] = PackedReferenceSite::new(
            (reference.site_identity >> 30) as u8,
            reference.site_identity & ((1 << 30) - 1),
            reference.metadata.relation_mask(),
            reference.metadata.occurrence_count(),
        )?;
        *position += 1;
    }
    debug_assert_eq!(write_positions.as_slice(), &counts[1..]);

    let mut offsets = Vec::with_capacity(counts.len());
    offsets.push(0_u32);
    let mut write = 0_usize;
    for target in 0..counts.len() - 1 {
        let start = counts[target] as usize;
        let end = counts[target + 1] as usize;
        sites[start..end].sort_unstable_by_key(|site| site.identity());

        let mut read = start;
        while read < end {
            let first = sites[read];
            let identity = first.identity();
            let mut relation_mask = first.relation_mask();
            let mut occurrence_count = first.occurrence_count();
            read += 1;
            while read < end && sites[read].identity() == identity {
                occurrence_count = occurrence_count
                    .checked_add(sites[read].occurrence_count())
                    .filter(|count| *count < (1 << 24))
                    .ok_or_else(|| anyhow!("Merged reference count exceeds 16777215"))?;
                relation_mask |= sites[read].relation_mask();
                read += 1;
            }
            sites[write] = PackedReferenceSite::new(
                first.storage_kind(),
                first.ordinal(),
                relation_mask,
                occurrence_count,
            )?;
            write += 1;
        }
        offsets.push(
            u32::try_from(write)
                .map_err(|_| anyhow!("The index has more than 4294967295 reference sites"))?,
        );
    }
    sites.truncate(write);
    Ok((offsets, sites))
}

fn build_literal_postings(
    mut counts: Vec<u32>,
    resolved: Vec<ResolvedLiteralReference>,
) -> anyhow::Result<(Vec<u32>, Vec<PackedLiteralSite>)> {
    for index in 1..counts.len() {
        counts[index] = counts[index]
            .checked_add(counts[index - 1])
            .ok_or_else(|| anyhow!("The index has more than 4294967295 literal sites"))?;
    }

    let placeholder = PackedLiteralSite::new(0, 0, 1)?;
    let mut sites = vec![placeholder; resolved.len()];
    let mut write_positions = counts[..counts.len() - 1].to_vec();
    for reference in resolved {
        let position = &mut write_positions[reference.target as usize];
        sites[*position as usize] = PackedLiteralSite::new(
            (reference.site_identity >> 30) as u8,
            reference.site_identity & ((1 << 30) - 1),
            reference.occurrence_count,
        )?;
        *position += 1;
    }
    debug_assert_eq!(write_positions.as_slice(), &counts[1..]);

    let mut offsets = Vec::with_capacity(counts.len());
    offsets.push(0_u32);
    let mut write = 0_usize;
    for target in 0..counts.len() - 1 {
        let start = counts[target] as usize;
        let end = counts[target + 1] as usize;
        sites[start..end].sort_unstable_by_key(|site| site.identity());

        let mut read = start;
        while read < end {
            let first = sites[read];
            let identity = first.identity();
            let mut occurrence_count = first.occurrence_count();
            read += 1;
            while read < end && sites[read].identity() == identity {
                occurrence_count = occurrence_count
                    .checked_add(sites[read].occurrence_count())
                    .ok_or_else(|| anyhow!("Merged literal count exceeds 4294967295"))?;
                read += 1;
            }
            sites[write] =
                PackedLiteralSite::new(first.storage_kind(), first.ordinal(), occurrence_count)?;
            write += 1;
        }
        offsets.push(
            u32::try_from(write)
                .map_err(|_| anyhow!("The index has more than 4294967295 literal sites"))?,
        );
    }
    sites.truncate(write);
    Ok((offsets, sites))
}

fn build_hierarchy(
    class_infos: &[&ClassInfo],
    classes: &ClassToIndexMap<'_>,
) -> Vec<HierarchyInfo> {
    class_infos
        .iter()
        .map(|info| HierarchyInfo {
            super_class: info
                .super_class
                .as_deref()
                .and_then(|name| class_index(classes, name)),
            interfaces: info
                .interfaces
                .iter()
                .filter_map(|name| class_index(classes, name))
                .collect(),
        })
        .collect()
}

fn resolve_source_site_identity(
    site: RawReferenceSite,
    owner: u32,
    field_offsets: &[u32],
    method_offsets: &[u32],
    extra_offsets: &[u32],
) -> anyhow::Result<u32> {
    let ordinal = match site.kind() {
        0 => owner,
        1 => field_offsets[owner as usize]
            .checked_add(site.ordinal())
            .ok_or_else(|| anyhow!("Field reference-site ordinal overflow"))?,
        2 => method_offsets[owner as usize]
            .checked_add(site.ordinal())
            .ok_or_else(|| anyhow!("Method reference-site ordinal overflow"))?,
        3 => extra_offsets[owner as usize]
            .checked_add(site.ordinal())
            .ok_or_else(|| anyhow!("Extra reference-site ordinal overflow"))?,
        kind => return Err(anyhow!("Unknown raw reference-site kind {kind}")),
    };
    Ok((site.kind() << 30) | ordinal)
}

fn resolve_field_from(
    owner: u32,
    name: u32,
    descriptor: u32,
    fields: &[MemberLookupEntry],
    hierarchy: &[HierarchyInfo],
    visiting: &mut FxHashSet<u32>,
) -> Option<u32> {
    if !visiting.insert(owner) {
        return None;
    }
    if let Some(field) = find_member(fields, owner, name, descriptor) {
        return Some(field);
    }
    for interface in &hierarchy[owner as usize].interfaces {
        if let Some(field) =
            resolve_field_from(*interface, name, descriptor, fields, hierarchy, visiting)
        {
            return Some(field);
        }
    }
    hierarchy[owner as usize].super_class.and_then(|parent| {
        resolve_field_from(parent, name, descriptor, fields, hierarchy, visiting)
    })
}

fn collect_interface_methods(
    interface: u32,
    name: u32,
    descriptor: u32,
    methods: &[MemberLookupEntry],
    hierarchy: &[HierarchyInfo],
    visited: &mut FxHashSet<u32>,
    output: &mut Vec<u32>,
) {
    if !visited.insert(interface) {
        return;
    }
    if let Some(method) = find_member(methods, interface, name, descriptor) {
        output.push(method);
        return;
    }
    for parent in &hierarchy[interface as usize].interfaces {
        collect_interface_methods(
            *parent, name, descriptor, methods, hierarchy, visited, output,
        );
    }
}

fn find_member(
    members: &[MemberLookupEntry],
    owner: u32,
    name: u32,
    descriptor: u32,
) -> Option<u32> {
    members
        .binary_search_by_key(&(owner, name, descriptor), member_key)
        .ok()
        .map(|index| members[index].ordinal)
}

fn member_key(member: &MemberLookupEntry) -> (u32, u32, u32) {
    (member.owner, member.name, member.descriptor)
}

fn class_index(classes: &ClassToIndexMap<'_>, internal_name: &str) -> Option<u32> {
    let (package_name, class_name) = internal_name
        .rsplit_once('/')
        .unwrap_or(("", internal_name));
    classes
        .get(&(package_name, class_name))
        .map(|(index, _)| *index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use speedy::Writable;

    #[test]
    fn cached_parallel_resolution_matches_oracle_and_is_deterministic() {
        let build = |threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    let (_, index) = crate::builder::workers::create_class_index_from_jars(
                        vec!["../src/test/resources/Samples.jar".to_owned()],
                        21,
                    )
                    .unwrap();
                    assert!(index.semantic_index().reference_site_count() > 1_000);
                    index.write_to_vec().unwrap()
                })
        };
        assert_eq!(build(1), build(4));
    }

    #[test]
    #[ignore = "Requires JINDEX_AUDIT_MANIFEST and JINDEX_AUDIT_JDK_ARCHIVE exported by RuntimeCorpusBenchmark"]
    fn audit_runtime_corpus_reference_equivalence() {
        use crate::builder::workers::{
            create_class_index_from_sources, ArchiveSource, DirectSource,
        };
        use std::io::Read;

        let manifest =
            std::fs::read_to_string(std::env::var("JINDEX_AUDIT_MANIFEST").unwrap()).unwrap();
        assert_eq!(
            manifest.lines().next(),
            Some("totaldebug-runtime-sources-v1")
        );
        let mut archives: Vec<_> = manifest
            .lines()
            .skip(1)
            .enumerate()
            .map(|(index, uri)| {
                let path = uri
                    .strip_prefix("file:///")
                    .expect("Expected absolute file URI");
                let mut bytes = Vec::new();
                let mut characters = path.as_bytes().iter().copied();
                while let Some(byte) = characters.next() {
                    bytes.push(if byte == b'%' {
                        let high = char::from(characters.next().unwrap()).to_digit(16).unwrap();
                        let low = char::from(characters.next().unwrap()).to_digit(16).unwrap();
                        ((high << 4) | low) as u8
                    } else {
                        byte
                    });
                }
                let path = String::from_utf8(bytes).unwrap();
                let path = if cfg!(windows) {
                    path
                } else {
                    format!("/{path}")
                };
                ArchiveSource {
                    source_id: index as u32,
                    input_order: index as u32,
                    target_java_release: 21,
                    file_name: path,
                }
            })
            .collect();
        let archive_count = archives.len();
        let jdk_archive = std::env::var("JINDEX_AUDIT_JDK_ARCHIVE").unwrap();
        let mut direct = Vec::new();
        {
            let mut zip = zip::ZipArchive::new(std::fs::File::open(jdk_archive).unwrap()).unwrap();
            for entry in 0..zip.len() {
                let selected = {
                    let file = zip.by_index_raw(entry).unwrap();
                    file.name().ends_with(".class")
                };
                if !selected {
                    continue;
                }
                let mut bytes = Vec::new();
                zip.by_index(entry)
                    .unwrap()
                    .read_to_end(&mut bytes)
                    .unwrap();
                let index = (archive_count + direct.len()) as u32;
                direct.push(DirectSource {
                    source_id: index,
                    input_order: direct.len() as u32,
                    bytes,
                });
            }
        }
        eprintln!(
            "Oracle corpus: {archive_count} archives, {} JDK classes",
            direct.len()
        );
        // The mixed Java overload puts direct JDK classes before archives in precedence order.
        for archive in &mut archives {
            archive.input_order += direct.len() as u32;
        }
        let (_, index) = create_class_index_from_sources(archives, direct).unwrap();
        eprintln!(
            "Exact per-edge oracle comparison passed: {} classes, {} reference sites",
            index.class_count(),
            index.semantic_index().reference_site_count()
        );
    }

    #[test]
    fn packed_reference_site_stays_eight_bytes() {
        assert_eq!(std::mem::size_of::<PackedReferenceSite>(), 8);
        assert_eq!(std::mem::size_of::<PackedLiteralSite>(), 8);
        assert_eq!(std::mem::size_of::<ResolvedReference>(), 12);
        assert_eq!(std::mem::size_of::<ResolvedLiteralReference>(), 12);
    }
}
