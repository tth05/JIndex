// Original per-edge resolution, retained only to check the cached/parallel implementation.
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn visit_resolved_references(
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
    mut visitor: impl FnMut(u32, u32, RawReferenceMetadata) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    for edge in &references.edges {
        let site_identity = resolve_source_site_identity(
            edge.site,
            owner,
            field_offsets,
            method_offsets,
            extra_site_offsets,
        )?;
        match references.targets[edge.target as usize] {
            RawReferenceTarget::Class { name } => {
                if let Some(target) = class_index(classes, references.string(name)) {
                    visitor(target, site_identity, edge.metadata)?;
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
                    visitor(class_count + target, site_identity, edge.metadata)?;
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
                    visitor(
                        class_count + field_count + target,
                        site_identity,
                        edge.metadata,
                    )?;
                }
            }
        }
    }
    Ok(())
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
