use super::bytecode::visit_constant_pool_operands;
use crate::reference_relations::{
    ClassReferenceKind, FieldReferenceKind, MemberReferenceKind, MethodReferenceKind,
};
use anyhow::{anyhow, bail, ensure, Context};
use compact_str::{CompactString, ToCompactString};
use jvmti_bindings::classfile::{
    Annotation, AttributeInfo, BootstrapMethod, ClassFile, ConstantPool, CpInfo, ElementValue,
    RecordComponent, TypeAnnotation,
};
use rustc_hash::FxHashMap;
use std::collections::hash_map::Entry;

const SITE_KIND_SHIFT: u32 = 30;
const SITE_ORDINAL_MASK: u32 = (1 << SITE_KIND_SHIFT) - 1;
const MAX_CONSTANT_DEPTH: usize = 64;
const RELATION_SHIFT: u32 = 24;
const OCCURRENCE_MASK: u32 = (1 << RELATION_SHIFT) - 1;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct RawReferenceSite(u32);

impl RawReferenceSite {
    pub(crate) fn class() -> Self {
        Self(0)
    }

    pub(crate) fn field(ordinal: usize) -> anyhow::Result<Self> {
        Self::new(1, ordinal)
    }

    pub(crate) fn method(ordinal: usize) -> anyhow::Result<Self> {
        Self::new(2, ordinal)
    }

    fn extra(ordinal: usize) -> anyhow::Result<Self> {
        Self::new(3, ordinal)
    }

    fn new(kind: u32, ordinal: usize) -> anyhow::Result<Self> {
        let ordinal = u32::try_from(ordinal)?;
        ensure!(
            ordinal <= SITE_ORDINAL_MASK,
            "Reference-site ordinal exceeds 30 bits"
        );
        Ok(Self((kind << SITE_KIND_SHIFT) | ordinal))
    }

    pub(crate) fn kind(self) -> u32 {
        self.0 >> SITE_KIND_SHIFT
    }

    pub(crate) fn ordinal(self) -> u32 {
        self.0 & SITE_ORDINAL_MASK
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RawExtraSiteKind {
    Field,
    Method,
    RecordComponent,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RawExtraSite {
    pub kind: RawExtraSiteKind,
    pub name: u32,
    pub descriptor: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum RawReferenceTarget {
    Class {
        name: u32,
    },
    Field {
        owner: u32,
        name: u32,
        descriptor: u32,
    },
    Method {
        owner: u32,
        name: u32,
        descriptor: u32,
    },
}

#[derive(Clone, Copy, Debug)]
enum ConstantContext {
    Metadata,
    Annotation,
    Bootstrap,
    Instruction(u8),
    Handle,
}

impl ConstantContext {
    fn class_kind(self) -> ClassReferenceKind {
        match self {
            Self::Annotation => ClassReferenceKind::AnnotationOrMetadata,
            Self::Bootstrap | Self::Instruction(_) | Self::Handle => {
                ClassReferenceKind::RuntimeType
            }
            Self::Metadata => ClassReferenceKind::AnnotationOrMetadata,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RawReferenceEdge {
    pub site: RawReferenceSite,
    pub target: u32,
    pub metadata: RawReferenceMetadata,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RawReferenceMetadata(u32);

impl RawReferenceMetadata {
    fn first(relation_mask: u8) -> anyhow::Result<Self> {
        ensure!(
            relation_mask > 0,
            "Reference relation mask must not be empty"
        );
        Ok(Self((u32::from(relation_mask) << RELATION_SHIFT) | 1))
    }

    fn add(&mut self, relation_mask: u8) -> anyhow::Result<()> {
        ensure!(
            relation_mask > 0,
            "Reference relation mask must not be empty"
        );
        let occurrence_count = self
            .occurrence_count()
            .checked_add(1)
            .filter(|count| *count <= OCCURRENCE_MASK)
            .ok_or_else(|| anyhow!("Reference occurrence count exceeds {OCCURRENCE_MASK}"))?;
        self.0 =
            (u32::from(self.relation_mask() | relation_mask) << RELATION_SHIFT) | occurrence_count;
        Ok(())
    }

    pub(crate) fn relation_mask(self) -> u8 {
        (self.0 >> RELATION_SHIFT) as u8
    }

    pub(crate) fn occurrence_count(self) -> u32 {
        self.0 & OCCURRENCE_MASK
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RawLiteralEdge {
    pub site: RawReferenceSite,
    pub literal: u32,
    pub occurrence_count: u32,
}

#[derive(Debug, Default)]
pub(crate) struct RawReferenceData {
    strings: Vec<u8>,
    pub targets: Vec<RawReferenceTarget>,
    pub edges: Vec<RawReferenceEdge>,
    pub extra_sites: Vec<RawExtraSite>,
    pub literals: Vec<Vec<u16>>,
    pub literal_edges: Vec<RawLiteralEdge>,
}

impl RawReferenceData {
    pub(crate) fn string(&self, offset: u32) -> &str {
        let offset = offset as usize;
        let length = u16::from_le_bytes([self.strings[offset], self.strings[offset + 1]]) as usize;
        std::str::from_utf8(&self.strings[offset + 2..offset + 2 + length])
            .expect("Raw reference strings were validated as UTF-8")
    }
}

#[derive(Default)]
pub(crate) struct RawReferenceBuilder {
    string_pool: RawStringPoolBuilder,
    targets: FxHashMap<RawReferenceTarget, u32>,
    edges: FxHashMap<(RawReferenceSite, u32), RawReferenceMetadata>,
    extra_sites: Vec<RawExtraSite>,
    literals: FxHashMap<Vec<u16>, u32>,
    literal_edges: FxHashMap<(RawReferenceSite, u32), u32>,
}

impl RawReferenceBuilder {
    pub(crate) fn add_extra_site(
        &mut self,
        kind: RawExtraSiteKind,
        name: &str,
        descriptor: &str,
    ) -> anyhow::Result<RawReferenceSite> {
        let name = self.string_pool.add(name)?;
        let descriptor = self.string_pool.add(descriptor)?;
        let site = RawReferenceSite::extra(self.extra_sites.len())?;
        self.extra_sites.push(RawExtraSite {
            kind,
            name,
            descriptor,
        });
        Ok(site)
    }

    pub(crate) fn collect(
        &mut self,
        class_file: &ClassFile,
        field_sites: &[RawReferenceSite],
        method_sites: &[RawReferenceSite],
    ) -> anyhow::Result<()> {
        ensure!(
            field_sites.len() == class_file.fields.len(),
            "Field reference-site count does not match the class file"
        );
        ensure!(
            method_sites.len() == class_file.methods.len(),
            "Method reference-site count does not match the class file"
        );
        let pool = &class_file.constant_pool;
        let bootstrap_methods = bootstrap_methods(&class_file.attributes);
        let class_site = RawReferenceSite::class();

        if class_file.super_class != 0 {
            self.add_class_index(
                class_site,
                pool,
                class_file.super_class,
                ClassReferenceKind::Hierarchy,
            )?;
        }
        for class_index in &class_file.interfaces {
            self.add_class_index(
                class_site,
                pool,
                *class_index,
                ClassReferenceKind::Hierarchy,
            )?;
        }
        self.collect_attributes(
            class_site,
            &class_file.attributes,
            pool,
            bootstrap_methods,
            false,
        )?;

        for (field, site) in class_file.fields.iter().zip(field_sites) {
            self.add_descriptor(
                *site,
                pool.get_utf8(field.descriptor_index)?,
                ClassReferenceKind::Declaration,
            )?;
            self.collect_attributes(*site, &field.attributes, pool, bootstrap_methods, false)?;
        }
        for (method, site) in class_file.methods.iter().zip(method_sites) {
            self.add_descriptor(
                *site,
                pool.get_utf8(method.descriptor_index)?,
                ClassReferenceKind::Declaration,
            )?;
            self.collect_attributes(*site, &method.attributes, pool, bootstrap_methods, true)?;
        }
        Ok(())
    }

    pub(crate) fn finish(self) -> RawReferenceData {
        let mut targets = vec![None; self.targets.len()];
        for (target, index) in self.targets {
            targets[index as usize] = Some(target);
        }
        let mut literals = self
            .literals
            .into_iter()
            .map(|(value, old_id)| (value, old_id))
            .collect::<Vec<_>>();
        literals.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        let mut literal_remap = vec![0_u32; literals.len()];
        for (new_id, (_, old_id)) in literals.iter().enumerate() {
            literal_remap[*old_id as usize] = new_id as u32;
        }
        let mut literal_edges = self
            .literal_edges
            .into_iter()
            .map(|((site, literal), occurrence_count)| RawLiteralEdge {
                site,
                literal: literal_remap[literal as usize],
                occurrence_count,
            })
            .collect::<Vec<_>>();
        literal_edges.sort_unstable_by_key(|edge| (edge.literal, edge.site.0));

        RawReferenceData {
            strings: self.string_pool.data,
            targets: targets
                .into_iter()
                .map(|target| target.expect("Raw reference target IDs are contiguous"))
                .collect(),
            edges: self
                .edges
                .into_iter()
                .map(|((site, target), metadata)| RawReferenceEdge {
                    site,
                    target,
                    metadata,
                })
                .collect(),
            extra_sites: self.extra_sites,
            literals: literals.into_iter().map(|(value, _)| value).collect(),
            literal_edges,
        }
    }

    fn collect_attributes(
        &mut self,
        site: RawReferenceSite,
        attributes: &[AttributeInfo],
        pool: &ConstantPool,
        bootstrap_methods: &[BootstrapMethod],
        scan_code: bool,
    ) -> anyhow::Result<()> {
        for attribute in attributes {
            match attribute {
                AttributeInfo::Signature { signature_index } => {
                    self.add_signature(
                        site,
                        pool.get_utf8(*signature_index)?,
                        ClassReferenceKind::Declaration,
                    )?;
                }
                AttributeInfo::ConstantValue {
                    constantvalue_index,
                } => {
                    self.collect_constant(
                        site,
                        pool,
                        bootstrap_methods,
                        *constantvalue_index,
                        ConstantContext::Metadata,
                        0,
                    )?;
                }
                AttributeInfo::Exceptions {
                    exception_index_table,
                } => {
                    for class_index in exception_index_table {
                        self.add_class_index(
                            site,
                            pool,
                            *class_index,
                            ClassReferenceKind::Declaration,
                        )?;
                    }
                }
                AttributeInfo::RuntimeVisibleAnnotations { annotations }
                | AttributeInfo::RuntimeInvisibleAnnotations { annotations } => {
                    for annotation in annotations {
                        self.collect_annotation(site, annotation, pool, bootstrap_methods)?;
                    }
                }
                AttributeInfo::RuntimeVisibleParameterAnnotations {
                    parameter_annotations,
                }
                | AttributeInfo::RuntimeInvisibleParameterAnnotations {
                    parameter_annotations,
                } => {
                    for annotations in parameter_annotations {
                        for annotation in annotations {
                            self.collect_annotation(site, annotation, pool, bootstrap_methods)?;
                        }
                    }
                }
                AttributeInfo::RuntimeVisibleTypeAnnotations { annotations }
                | AttributeInfo::RuntimeInvisibleTypeAnnotations { annotations } => {
                    for annotation in annotations {
                        self.collect_type_annotation(site, annotation, pool, bootstrap_methods)?;
                    }
                }
                AttributeInfo::AnnotationDefault { default_value } => {
                    self.collect_element_value(site, default_value, pool, bootstrap_methods)?;
                }
                AttributeInfo::PermittedSubclasses { classes } => {
                    for class_index in classes {
                        self.add_class_index(
                            site,
                            pool,
                            *class_index,
                            ClassReferenceKind::Hierarchy,
                        )?;
                    }
                }
                AttributeInfo::Record { components } => {
                    for component in components {
                        self.collect_record_component(component, pool, bootstrap_methods)?;
                    }
                }
                AttributeInfo::Code(code) if scan_code => {
                    for entry in &code.exception_table {
                        if entry.catch_type != 0 {
                            self.add_class_index(
                                site,
                                pool,
                                entry.catch_type,
                                ClassReferenceKind::RuntimeType,
                            )?;
                        }
                    }
                    visit_constant_pool_operands(&code.code, |opcode, index| {
                        self.collect_constant(
                            site,
                            pool,
                            bootstrap_methods,
                            index,
                            ConstantContext::Instruction(opcode),
                            0,
                        )
                    })?;
                    self.collect_attributes(
                        site,
                        &code.attributes,
                        pool,
                        bootstrap_methods,
                        false,
                    )?;
                }
                AttributeInfo::LocalVariableTable { entries } => {
                    for entry in entries {
                        self.add_descriptor(
                            site,
                            pool.get_utf8(entry.descriptor_index)?,
                            ClassReferenceKind::Declaration,
                        )?;
                    }
                }
                AttributeInfo::LocalVariableTypeTable { entries } => {
                    for entry in entries {
                        self.add_signature(
                            site,
                            pool.get_utf8(entry.signature_index)?,
                            ClassReferenceKind::Declaration,
                        )?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn collect_record_component(
        &mut self,
        component: &RecordComponent,
        pool: &ConstantPool,
        bootstrap_methods: &[BootstrapMethod],
    ) -> anyhow::Result<()> {
        let name = pool.get_utf8(component.name_index)?;
        let descriptor = pool.get_utf8(component.descriptor_index)?;
        let site = self.add_extra_site(RawExtraSiteKind::RecordComponent, name, descriptor)?;
        self.add_descriptor(site, descriptor, ClassReferenceKind::Declaration)?;
        self.collect_attributes(site, &component.attributes, pool, bootstrap_methods, false)
    }

    fn collect_annotation(
        &mut self,
        site: RawReferenceSite,
        annotation: &Annotation,
        pool: &ConstantPool,
        bootstrap_methods: &[BootstrapMethod],
    ) -> anyhow::Result<()> {
        self.add_descriptor(
            site,
            pool.get_utf8(annotation.type_index)?,
            ClassReferenceKind::AnnotationOrMetadata,
        )?;
        for pair in &annotation.element_value_pairs {
            self.collect_element_value(site, &pair.value, pool, bootstrap_methods)?;
        }
        Ok(())
    }

    fn collect_type_annotation(
        &mut self,
        site: RawReferenceSite,
        annotation: &TypeAnnotation,
        pool: &ConstantPool,
        bootstrap_methods: &[BootstrapMethod],
    ) -> anyhow::Result<()> {
        self.add_descriptor(
            site,
            pool.get_utf8(annotation.type_index)?,
            ClassReferenceKind::AnnotationOrMetadata,
        )?;
        for pair in &annotation.element_value_pairs {
            self.collect_element_value(site, &pair.value, pool, bootstrap_methods)?;
        }
        Ok(())
    }

    fn collect_element_value(
        &mut self,
        site: RawReferenceSite,
        value: &ElementValue,
        pool: &ConstantPool,
        bootstrap_methods: &[BootstrapMethod],
    ) -> anyhow::Result<()> {
        match value {
            ElementValue::Const {
                tag: b's',
                const_value_index,
            } => self.add_literal(site, pool, *const_value_index),
            ElementValue::Const {
                const_value_index, ..
            } => self.collect_constant(
                site,
                pool,
                bootstrap_methods,
                *const_value_index,
                ConstantContext::Annotation,
                0,
            ),
            ElementValue::EnumConst {
                type_name_index, ..
            } => self.add_descriptor(
                site,
                pool.get_utf8(*type_name_index)?,
                ClassReferenceKind::AnnotationOrMetadata,
            ),
            ElementValue::ClassInfo { class_info_index } => self.add_descriptor(
                site,
                pool.get_utf8(*class_info_index)?,
                ClassReferenceKind::AnnotationOrMetadata,
            ),
            ElementValue::AnnotationValue(annotation) => {
                self.collect_annotation(site, annotation, pool, bootstrap_methods)
            }
            ElementValue::ArrayValue(values) => {
                for value in values {
                    self.collect_element_value(site, value, pool, bootstrap_methods)?;
                }
                Ok(())
            }
            other => bail!("Unsupported annotation element value {other:?}"),
        }
    }

    fn collect_constant(
        &mut self,
        site: RawReferenceSite,
        pool: &ConstantPool,
        bootstrap_methods: &[BootstrapMethod],
        index: u16,
        context: ConstantContext,
        depth: usize,
    ) -> anyhow::Result<()> {
        ensure!(
            depth < MAX_CONSTANT_DEPTH,
            "Constant-pool reference nesting exceeds 64"
        );
        match pool.get(index)? {
            CpInfo::String { string_index } => self.add_literal(site, pool, *string_index),
            CpInfo::Class { .. } => self.add_class_index(site, pool, index, context.class_kind()),
            CpInfo::Fieldref {
                class_index,
                name_and_type_index,
            } => self.add_member_reference(
                site,
                pool,
                *class_index,
                *name_and_type_index,
                match context {
                    ConstantContext::Instruction(0xb2 | 0xb4) => {
                        MemberReferenceKind::Field(FieldReferenceKind::Read)
                    }
                    ConstantContext::Instruction(0xb3 | 0xb5) => {
                        MemberReferenceKind::Field(FieldReferenceKind::Write)
                    }
                    ConstantContext::Handle | ConstantContext::Bootstrap => {
                        MemberReferenceKind::Field(FieldReferenceKind::Handle)
                    }
                    other => bail!("Field reference has incompatible context {other:?}"),
                },
            ),
            CpInfo::Methodref {
                class_index,
                name_and_type_index,
            }
            | CpInfo::InterfaceMethodref {
                class_index,
                name_and_type_index,
            } => self.add_member_reference(
                site,
                pool,
                *class_index,
                *name_and_type_index,
                match context {
                    ConstantContext::Instruction(0xb6..=0xb9) => {
                        MemberReferenceKind::Method(MethodReferenceKind::Invoke)
                    }
                    ConstantContext::Handle | ConstantContext::Bootstrap => {
                        MemberReferenceKind::Method(MethodReferenceKind::Handle)
                    }
                    other => bail!("Method reference has incompatible context {other:?}"),
                },
            ),
            CpInfo::MethodHandle {
                reference_index, ..
            } => self.collect_constant(
                site,
                pool,
                bootstrap_methods,
                *reference_index,
                ConstantContext::Handle,
                depth + 1,
            ),
            CpInfo::MethodType { descriptor_index } => self.add_descriptor(
                site,
                pool.get_utf8(*descriptor_index)?,
                ClassReferenceKind::RuntimeType,
            ),
            CpInfo::Dynamic {
                bootstrap_method_attr_index,
                name_and_type_index,
            }
            | CpInfo::InvokeDynamic {
                bootstrap_method_attr_index,
                name_and_type_index,
            } => {
                let (_, descriptor) = name_and_type(pool, *name_and_type_index)?;
                self.add_descriptor(site, descriptor, ClassReferenceKind::MemberUsage)?;
                let bootstrap = bootstrap_methods
                    .get(*bootstrap_method_attr_index as usize)
                    .with_context(|| {
                        format!(
                            "Invalid bootstrap method index {}",
                            bootstrap_method_attr_index
                        )
                    })?;
                self.collect_constant(
                    site,
                    pool,
                    bootstrap_methods,
                    bootstrap.bootstrap_method_ref,
                    ConstantContext::Bootstrap,
                    depth + 1,
                )?;
                let skip_concat_recipe = is_string_concat_with_constants(pool, bootstrap)?;
                for (argument_index, argument) in bootstrap.bootstrap_arguments.iter().enumerate() {
                    if skip_concat_recipe && argument_index == 0 {
                        continue;
                    }
                    self.collect_constant(
                        site,
                        pool,
                        bootstrap_methods,
                        *argument,
                        ConstantContext::Bootstrap,
                        depth + 1,
                    )?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn add_member_reference(
        &mut self,
        site: RawReferenceSite,
        pool: &ConstantPool,
        class_index: u16,
        name_and_type_index: u16,
        kind: MemberReferenceKind,
    ) -> anyhow::Result<()> {
        let owner = class_name(pool, class_index)?;
        let (name, descriptor) = name_and_type(pool, name_and_type_index)?;
        self.add_class_name(site, owner, ClassReferenceKind::MemberUsage)?;
        self.add_descriptor(site, descriptor, ClassReferenceKind::MemberUsage)?;
        if !owner.is_ascii() || !name.is_ascii() || !descriptor.is_ascii() {
            return Ok(());
        }
        let owner = self.string_pool.add(owner)?;
        let name = self.string_pool.add(name)?;
        let descriptor = self.string_pool.add(descriptor)?;
        let target = match kind {
            MemberReferenceKind::Method(_) => RawReferenceTarget::Method {
                owner,
                name,
                descriptor,
            },
            MemberReferenceKind::Field(_) => RawReferenceTarget::Field {
                owner,
                name,
                descriptor,
            },
        };
        self.add_target(site, target, kind.mask())
    }

    fn add_class_index(
        &mut self,
        site: RawReferenceSite,
        pool: &ConstantPool,
        class_index: u16,
        kind: ClassReferenceKind,
    ) -> anyhow::Result<()> {
        self.add_class_name(site, class_name(pool, class_index)?, kind)
    }

    fn add_class_name(
        &mut self,
        site: RawReferenceSite,
        name: &str,
        kind: ClassReferenceKind,
    ) -> anyhow::Result<()> {
        if name.starts_with('[') {
            return self.add_descriptor(site, name, kind);
        }
        if !name.is_ascii() {
            return Ok(());
        }
        let name = self.string_pool.add(name)?;
        self.add_target(site, RawReferenceTarget::Class { name }, kind.mask())
    }

    fn add_descriptor(
        &mut self,
        site: RawReferenceSite,
        descriptor: &str,
        kind: ClassReferenceKind,
    ) -> anyhow::Result<()> {
        let bytes = descriptor.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] != b'L' {
                index += 1;
                continue;
            }
            let start = index + 1;
            let relative_end = bytes[start..]
                .iter()
                .position(|byte| *byte == b';')
                .ok_or_else(|| anyhow!("Unterminated object type in descriptor {descriptor}"))?;
            let end = start + relative_end;
            self.add_class_name(site, &descriptor[start..end], kind)?;
            index = end + 1;
        }
        Ok(())
    }

    fn add_signature(
        &mut self,
        site: RawReferenceSite,
        signature: &str,
        kind: ClassReferenceKind,
    ) -> anyhow::Result<()> {
        let mut index = 0;
        collect_signature_types(signature, &mut index, None, &mut |name| {
            self.add_class_name(site, name, kind)
        })
    }

    fn add_target(
        &mut self,
        site: RawReferenceSite,
        target: RawReferenceTarget,
        relation_mask: u8,
    ) -> anyhow::Result<()> {
        let next_id = u32::try_from(self.targets.len())?;
        let target_id = match self.targets.entry(target) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                entry.insert(next_id);
                next_id
            }
        };
        match self.edges.entry((site, target_id)) {
            Entry::Occupied(mut entry) => entry.get_mut().add(relation_mask)?,
            Entry::Vacant(entry) => {
                entry.insert(RawReferenceMetadata::first(relation_mask)?);
            }
        }
        Ok(())
    }

    fn add_literal(
        &mut self,
        site: RawReferenceSite,
        pool: &ConstantPool,
        string_index: u16,
    ) -> anyhow::Result<()> {
        let value = pool.get_java_string(string_index)?.to_utf16().into_owned();
        let next_id = u32::try_from(self.literals.len())?;
        let literal_id = match self.literals.entry(value) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                entry.insert(next_id);
                next_id
            }
        };
        let count = self.literal_edges.entry((site, literal_id)).or_default();
        *count = count
            .checked_add(1)
            .ok_or_else(|| anyhow!("Literal occurrence count exceeds 4294967295"))?;
        Ok(())
    }
}

#[derive(Default)]
struct RawStringPoolBuilder {
    data: Vec<u8>,
    offsets: FxHashMap<CompactString, u32>,
}

impl RawStringPoolBuilder {
    fn add(&mut self, value: &str) -> anyhow::Result<u32> {
        if let Some(offset) = self.offsets.get(value) {
            return Ok(*offset);
        }
        let length = u16::try_from(value.len())
            .map_err(|_| anyhow!("Raw reference string exceeds 65535 bytes"))?;
        let offset = u32::try_from(self.data.len())
            .map_err(|_| anyhow!("Raw reference string pool exceeds 4 GiB"))?;
        self.data.extend_from_slice(&length.to_le_bytes());
        self.data.extend_from_slice(value.as_bytes());
        self.offsets.insert(value.to_compact_string(), offset);
        Ok(offset)
    }
}

fn bootstrap_methods(attributes: &[AttributeInfo]) -> &[BootstrapMethod] {
    attributes
        .iter()
        .find_map(|attribute| match attribute {
            AttributeInfo::BootstrapMethods { methods } => Some(methods.as_slice()),
            _ => None,
        })
        .unwrap_or_default()
}

fn is_string_concat_with_constants(
    pool: &ConstantPool,
    bootstrap: &BootstrapMethod,
) -> anyhow::Result<bool> {
    let reference_index = match pool.get(bootstrap.bootstrap_method_ref)? {
        CpInfo::MethodHandle {
            reference_index, ..
        } => *reference_index,
        _ => return Ok(false),
    };
    let (class_index, name_and_type_index) = match pool.get(reference_index)? {
        CpInfo::Methodref {
            class_index,
            name_and_type_index,
        }
        | CpInfo::InterfaceMethodref {
            class_index,
            name_and_type_index,
        } => (*class_index, *name_and_type_index),
        _ => return Ok(false),
    };
    let (name, _) = name_and_type(pool, name_and_type_index)?;
    Ok(
        class_name(pool, class_index)? == "java/lang/invoke/StringConcatFactory"
            && name == "makeConcatWithConstants",
    )
}

fn class_name(pool: &ConstantPool, class_index: u16) -> anyhow::Result<&str> {
    match pool.get(class_index)? {
        CpInfo::Class { name_index } => Ok(pool.get_utf8(*name_index)?),
        _ => bail!("Constant-pool entry {class_index} is not a class"),
    }
}

fn name_and_type(pool: &ConstantPool, index: u16) -> anyhow::Result<(&str, &str)> {
    match pool.get(index)? {
        CpInfo::NameAndType {
            name_index,
            descriptor_index,
        } => Ok((
            pool.get_utf8(*name_index)?,
            pool.get_utf8(*descriptor_index)?,
        )),
        _ => bail!("Constant-pool entry {index} is not a name and type"),
    }
}

fn collect_signature_types(
    signature: &str,
    index: &mut usize,
    terminator: Option<u8>,
    visitor: &mut impl FnMut(&str) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let bytes = signature.as_bytes();
    if terminator.is_none() && *index == 0 && bytes.first() == Some(&b'<') {
        collect_formal_type_parameters(signature, index, visitor)?;
    }
    while *index < bytes.len() {
        if Some(bytes[*index]) == terminator {
            *index += 1;
            return Ok(());
        }
        match bytes[*index] {
            b'L' => collect_class_signature_type(signature, index, visitor)?,
            b'T' => skip_type_variable(signature, index)?,
            _ => *index += 1,
        }
    }
    ensure!(
        terminator.is_none(),
        "Unterminated nested signature in {signature}"
    );
    Ok(())
}

fn collect_formal_type_parameters(
    signature: &str,
    index: &mut usize,
    visitor: &mut impl FnMut(&str) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let bytes = signature.as_bytes();
    *index += 1;
    while *index < bytes.len() && bytes[*index] != b'>' {
        while *index < bytes.len() && bytes[*index] != b':' {
            *index += 1;
        }
        ensure!(
            *index < bytes.len(),
            "Unterminated formal type parameter in {signature}"
        );
        while *index < bytes.len() && bytes[*index] == b':' {
            *index += 1;
            if *index < bytes.len() && bytes[*index] != b':' {
                collect_reference_type(signature, index, visitor)?;
            }
        }
    }
    ensure!(
        *index < bytes.len() && bytes[*index] == b'>',
        "Unterminated formal type parameters in {signature}"
    );
    *index += 1;
    Ok(())
}

fn collect_reference_type(
    signature: &str,
    index: &mut usize,
    visitor: &mut impl FnMut(&str) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let bytes = signature.as_bytes();
    ensure!(
        *index < bytes.len(),
        "Missing reference type in {signature}"
    );
    match bytes[*index] {
        b'L' => collect_class_signature_type(signature, index, visitor),
        b'T' => skip_type_variable(signature, index),
        b'[' => {
            while *index < bytes.len() && bytes[*index] == b'[' {
                *index += 1;
            }
            ensure!(
                *index < bytes.len(),
                "Missing array component in {signature}"
            );
            if matches!(
                bytes[*index],
                b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z'
            ) {
                *index += 1;
                Ok(())
            } else {
                collect_reference_type(signature, index, visitor)
            }
        }
        byte => bail!(
            "Unexpected reference type byte 0x{byte:02x} at offset {} in {signature}",
            *index
        ),
    }
}

fn skip_type_variable(signature: &str, index: &mut usize) -> anyhow::Result<()> {
    let bytes = signature.as_bytes();
    let end = bytes[*index..]
        .iter()
        .position(|byte| *byte == b';')
        .map(|relative| *index + relative)
        .ok_or_else(|| anyhow!("Unterminated type variable in signature {signature}"))?;
    *index = end + 1;
    Ok(())
}

fn collect_class_signature_type(
    signature: &str,
    index: &mut usize,
    visitor: &mut impl FnMut(&str) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let bytes = signature.as_bytes();
    debug_assert_eq!(bytes[*index], b'L');

    *index += 1;
    let start = *index;
    while *index < bytes.len() && !matches!(bytes[*index], b'<' | b'.' | b';') {
        *index += 1;
    }
    ensure!(*index > start, "Empty class type in signature {signature}");
    let mut current_name = signature[start..*index].to_owned();
    visitor(&current_name)?;

    loop {
        ensure!(
            *index < bytes.len(),
            "Unterminated class type in signature {signature}"
        );
        match bytes[*index] {
            b'<' => {
                *index += 1;
                collect_signature_types(signature, index, Some(b'>'), visitor)?;
            }
            b'.' => {
                *index += 1;
                let inner_start = *index;
                while *index < bytes.len() && !matches!(bytes[*index], b'<' | b'.' | b';') {
                    *index += 1;
                }
                ensure!(
                    *index > inner_start,
                    "Empty inner class type in signature {signature}"
                );
                current_name.push('$');
                current_name.push_str(&signature[inner_start..*index]);
                visitor(&current_name)?;
            }
            b';' => {
                *index += 1;
                return Ok(());
            }
            other => bail!(
                "Unexpected signature byte 0x{other:02x} at offset {} in {signature}",
                *index
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_scanner_reports_outer_inner_and_nested_types() {
        let signature = "Ljava/util/Map<Lpkg/Key;Lpkg/Outer<TT;>.Inner<Lpkg/Value;>;>;";
        let mut index = 0;
        let mut names = Vec::new();
        collect_signature_types(signature, &mut index, None, &mut |name| {
            names.push(name.to_owned());
            Ok(())
        })
        .unwrap();
        assert_eq!(
            names,
            [
                "java/util/Map",
                "pkg/Key",
                "pkg/Outer",
                "pkg/Outer$Inner",
                "pkg/Value"
            ]
        );
    }

    #[test]
    fn signature_scanner_skips_type_variable_names() {
        let signature = "<L:Ljava/lang/Object;>(TL;)TL;";
        let mut index = 0;
        let mut names = Vec::new();
        collect_signature_types(signature, &mut index, None, &mut |name| {
            names.push(name.to_owned());
            Ok(())
        })
        .unwrap();
        assert_eq!(names, ["java/lang/Object"]);
    }

    #[test]
    fn signature_scanner_accepts_primitive_array_bounds() {
        let signature = "<T:[I>Ljava/lang/Object;";
        let mut index = 0;
        let mut names = Vec::new();
        collect_signature_types(signature, &mut index, None, &mut |name| {
            names.push(name.to_owned());
            Ok(())
        })
        .unwrap();
        assert_eq!(names, ["java/lang/Object"]);
    }
}
