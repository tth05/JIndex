use super::raw_references::{
    RawExtraSiteKind, RawReferenceData, RawReferenceSite, RawReferenceTarget,
};
use super::{ClassInfo, ClassToIndexMap};
use crate::semantic_index::{
    DescriptorPool, ExtraReferenceSite, PackedReferenceSite, ReferenceIndexData, ReferenceSiteKind,
    Utf8Pool,
};
use anyhow::anyhow;
use rustc_hash::{FxHashMap, FxHashSet};

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
    let mut offsets = vec![0_u32; total_targets as usize + 1];
    let raw_edge_count = references_by_index
        .iter()
        .map(|references| references.edges.len())
        .sum();
    let mut resolved = Vec::with_capacity(raw_edge_count);
    let raw_literal_edge_count = references_by_index
        .iter()
        .map(|references| references.literal_edges.len())
        .sum();
    let mut literal_ids: FxHashMap<Vec<u16>, u32> = FxHashMap::default();
    let mut resolved_literals = Vec::with_capacity(raw_literal_edge_count);
    let class_count = u32::try_from(class_count)?;
    for (owner, references) in references_by_index.into_iter().enumerate() {
        let mut local_literal_ids = Vec::with_capacity(references.literals.len());
        for value in &references.literals {
            let literal_id = if let Some(id) = literal_ids.get(value.as_slice()) {
                *id
            } else {
                let id = u32::try_from(literal_ids.len())
                    .map_err(|_| anyhow!("The index contains more than 4294967295 literals"))?;
                literal_ids.insert(value.clone(), id);
                id
            };
            local_literal_ids.push(literal_id);
        }
        for edge in &references.literal_edges {
            let site = resolve_source_site(
                edge.site,
                owner as u32,
                edge.occurrence_count,
                field_offsets,
                method_offsets,
                &extra_site_offsets,
            )?;
            resolved_literals.push(ResolvedReference {
                target: local_literal_ids[edge.literal as usize],
                site_identity: site.identity(),
                occurrence_count: site.occurrence_count(),
            });
        }
        visit_resolved_references(
            owner as u32,
            &references,
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
            |target, site| {
                offsets[target as usize + 1] = offsets[target as usize + 1]
                    .checked_add(1)
                    .ok_or_else(|| anyhow!("A symbol has more than 4294967295 reference sites"))?;
                resolved.push(ResolvedReference {
                    target,
                    site_identity: site.identity(),
                    occurrence_count: site.occurrence_count(),
                });
                Ok(())
            },
        )?;
    }
    let (reference_offsets, reference_sites) = build_postings(offsets, resolved)?;

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
        build_postings(literal_posting_counts, resolved_literals)?;

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

fn build_postings(
    mut counts: Vec<u32>,
    resolved: Vec<ResolvedReference>,
) -> anyhow::Result<(Vec<u32>, Vec<PackedReferenceSite>)> {
    for index in 1..counts.len() {
        counts[index] = counts[index]
            .checked_add(counts[index - 1])
            .ok_or_else(|| anyhow!("The index has more than 4294967295 reference sites"))?;
    }

    let placeholder = PackedReferenceSite::new(0, 0, 1)?;
    let mut sites = vec![placeholder; resolved.len()];
    let mut write_positions = counts[..counts.len() - 1].to_vec();
    for reference in resolved {
        let position = &mut write_positions[reference.target as usize];
        sites[*position as usize] = PackedReferenceSite::new(
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
                    .ok_or_else(|| anyhow!("Merged reference count exceeds 4294967295"))?;
                read += 1;
            }
            sites[write] =
                PackedReferenceSite::new(first.storage_kind(), first.ordinal(), occurrence_count)?;
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

#[allow(clippy::too_many_arguments)]
fn visit_resolved_references(
    owner: u32,
    references: &RawReferenceData,
    classes: &ClassToIndexMap<'_>,
    names: &FxHashMap<&str, u32>,
    descriptors: &FxHashMap<&str, u32>,
    fields: &[MemberLookupEntry],
    methods: &[MemberLookupEntry],
    hierarchy: &[HierarchyInfo],
    field_offsets: &[u32],
    method_offsets: &[u32],
    extra_site_offsets: &[u32],
    class_count: u32,
    field_count: u32,
    mut visitor: impl FnMut(u32, PackedReferenceSite) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    for edge in &references.edges {
        let site = resolve_source_site(
            edge.site,
            owner,
            edge.occurrence_count,
            field_offsets,
            method_offsets,
            extra_site_offsets,
        )?;
        match references.targets[edge.target as usize] {
            RawReferenceTarget::Class { name } => {
                if let Some(target) = class_index(classes, references.string(name)) {
                    visitor(target, site)?;
                }
            }
            RawReferenceTarget::Field {
                owner,
                name,
                descriptor,
            } => {
                if let Some(target) = resolve_field(
                    references.string(owner),
                    references.string(name),
                    references.string(descriptor),
                    classes,
                    names,
                    descriptors,
                    fields,
                    hierarchy,
                ) {
                    visitor(class_count + target, site)?;
                }
            }
            RawReferenceTarget::Method {
                owner,
                name,
                descriptor,
            } => {
                for target in resolve_methods(
                    references.string(owner),
                    references.string(name),
                    references.string(descriptor),
                    classes,
                    names,
                    descriptors,
                    methods,
                    hierarchy,
                ) {
                    visitor(class_count + field_count + target, site)?;
                }
            }
        }
    }
    Ok(())
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

fn resolve_source_site(
    site: RawReferenceSite,
    owner: u32,
    occurrence_count: u32,
    field_offsets: &[u32],
    method_offsets: &[u32],
    extra_offsets: &[u32],
) -> anyhow::Result<PackedReferenceSite> {
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
    PackedReferenceSite::new(site.kind() as u8, ordinal, occurrence_count)
}

#[allow(clippy::too_many_arguments)]
fn resolve_field(
    owner: &str,
    name: &str,
    descriptor: &str,
    classes: &ClassToIndexMap<'_>,
    names: &FxHashMap<&str, u32>,
    descriptors: &FxHashMap<&str, u32>,
    fields: &[MemberLookupEntry],
    hierarchy: &[HierarchyInfo],
) -> Option<u32> {
    let owner = class_index(classes, owner)?;
    let name = *names.get(name)?;
    let descriptor = *descriptors.get(descriptor)?;
    resolve_field_from(
        owner,
        name,
        descriptor,
        fields,
        hierarchy,
        &mut FxHashSet::default(),
    )
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

#[allow(clippy::too_many_arguments)]
fn resolve_methods(
    owner: &str,
    name: &str,
    descriptor: &str,
    classes: &ClassToIndexMap<'_>,
    names: &FxHashMap<&str, u32>,
    descriptors: &FxHashMap<&str, u32>,
    methods: &[MemberLookupEntry],
    hierarchy: &[HierarchyInfo],
) -> Vec<u32> {
    let Some(owner) = class_index(classes, owner) else {
        return Vec::new();
    };
    let Some(name_index) = names.get(name).copied() else {
        return Vec::new();
    };
    let Some(descriptor_index) = descriptors.get(descriptor).copied() else {
        return Vec::new();
    };
    if name == "<init>" {
        return find_member(methods, owner, name_index, descriptor_index)
            .into_iter()
            .collect();
    }

    let mut interface_roots = Vec::new();
    let mut current = Some(owner);
    let mut visited_classes = FxHashSet::default();
    while let Some(class) = current {
        if !visited_classes.insert(class) {
            break;
        }
        if let Some(method) = find_member(methods, class, name_index, descriptor_index) {
            return vec![method];
        }
        interface_roots.extend_from_slice(&hierarchy[class as usize].interfaces);
        current = hierarchy[class as usize].super_class;
    }

    let mut results = Vec::new();
    let mut visited_interfaces = FxHashSet::default();
    for interface in interface_roots {
        collect_interface_methods(
            interface,
            name_index,
            descriptor_index,
            methods,
            hierarchy,
            &mut visited_interfaces,
            &mut results,
        );
    }
    results.sort_unstable();
    results.dedup();
    results
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

    #[test]
    fn packed_reference_site_stays_eight_bytes() {
        assert_eq!(std::mem::size_of::<PackedReferenceSite>(), 8);
        assert_eq!(std::mem::size_of::<ResolvedReference>(), 12);
    }
}
