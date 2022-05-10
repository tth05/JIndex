use crate::class_index::ClassIndex;
use crate::class_index_members::{IndexedClass, IndexedField, IndexedMethod};
use crate::constant_pool::ClassIndexConstantPool;
use crate::package_index::PackageIndex;
use crate::signature::indexed_signature::ToIndexedType;
use crate::signature::{
    RawClassSignature, RawEnclosingTypeInfo, RawMethodSignature, RawSignatureType,
};
use anyhow::anyhow;
use ascii::{AsciiChar, AsciiStr, AsciiString};
use cafebabe::{FieldAccessFlags, MethodAccessFlags};
use rustc_hash::FxHashMap;
use std::time::Instant;

pub mod workers;

pub(crate) type ClassToIndexMap<'a> =
    FxHashMap<(&'a AsciiStr, &'a AsciiStr), (u32, &'a IndexedClass)>;

struct ClassIndexBuilder {
    expected_method_count: u32,
    average_class_name_size: u32,
    average_method_name_size: u32,
}

impl ClassIndexBuilder {
    fn new() -> Self {
        Self {
            expected_method_count: 0,
            average_class_name_size: 15,
            average_method_name_size: 8,
        }
    }

    fn with_expected_method_count(mut self, count: u32) -> Self {
        self.expected_method_count = count;
        self
    }

    fn build(self, vec: Vec<ClassInfo>) -> anyhow::Result<(BuildTimeInfo, ClassIndex)> {
        let start_time = Instant::now();
        let element_count = vec.len() as u32;

        let mut constant_pool = ClassIndexConstantPool::new(
            ((element_count * self.average_class_name_size
                + self.expected_method_count * self.average_method_name_size) as f32
                * 0.8) as u32,
        );

        let mut package_index = PackageIndex::new();
        let mut classes: Vec<((&AsciiStr, &AsciiStr), IndexedClass)> =
            Vec::with_capacity(vec.len());
        let mut constant_pool_map: FxHashMap<&AsciiStr, u32> = FxHashMap::with_capacity_and_hasher(
            vec.len() + self.expected_method_count as usize,
            Default::default(),
        );

        for class_info in vec.iter() {
            let package_index = package_index
                .get_or_add_package_index(&mut constant_pool, &class_info.package_name);
            let class_name_index = get_index_from_pool(
                &class_info.class_name,
                &mut constant_pool_map,
                &mut constant_pool,
            )?;

            let indexed_class = IndexedClass::new(
                package_index,
                class_name_index,
                class_info.class_name_start_index as u8, //Name can't be longer than u8::MAX
                class_info.access_flags,
            );

            classes.push((
                (&class_info.package_name, &class_info.class_name),
                indexed_class,
            ));
        }

        // Sort classes to finalize their order, this allows us to refer to them by index from now
        // on
        ClassIndexBuilder::sort_classes(&package_index, &constant_pool, &mut classes);
        let mut classes_map: ClassToIndexMap =
            FxHashMap::with_capacity_and_hasher(classes.len(), Default::default());
        // Build a name to index map and give each class its index
        classes.iter().enumerate().for_each(|(index, class)| {
            class.1.set_index(index as u32);
            classes_map.insert(class.0, (index as u32, &class.1));
        });

        //TODO: Multi thread this loop using dashmap/flurry?
        for class_info in vec.iter() {
            let (indexed_class_index, indexed_class) = classes_map
                .get(&(
                    class_info.package_name.as_ref(),
                    class_info.class_name.as_ref(),
                ))
                .ok_or_else(|| {
                    anyhow::anyhow!("Indexed class not found in map {:?}", class_info)
                })?;

            //Add class to its package
            package_index
                .package_at(indexed_class.package_index())
                .add_class(*indexed_class_index);

            //Signature
            indexed_class.set_signature(class_info.signature.to_indexed_type(
                &mut constant_pool,
                &mut constant_pool_map,
                &classes_map,
            )?);

            //Enclosing type
            if let Some(info) = &class_info.enclosing_type {
                indexed_class.set_enclosing_type_info(info.to_indexed_type(
                    &mut constant_pool,
                    &mut constant_pool_map,
                    &classes_map,
                )?);
            }

            //Member classes
            if let Some(members) = &class_info.member_classes {
                members
                    .iter()
                    .filter_map(|m| {
                        let split_parts = crate::rsplit_once(m, AsciiChar::Slash);
                        classes_map.get(&split_parts)
                    })
                    .for_each(|m| {
                        indexed_class.add_member_class(m.0);
                    })
            }

            //Fields
            let mut indexed_fields = Vec::with_capacity(class_info.fields.len());

            for field_info in class_info.fields.iter() {
                let field_name = &field_info.field_name;

                let field_name_index =
                    get_index_from_pool(field_name, &mut constant_pool_map, &mut constant_pool)?;

                indexed_fields.push(IndexedField::new(
                    field_name_index,
                    field_info.access_flags.bits(),
                    field_info.descriptor.to_indexed_type(
                        &mut constant_pool,
                        &mut constant_pool_map,
                        &classes_map,
                    )?,
                ));
            }

            indexed_class.set_fields(indexed_fields).map_err(|_| {
                anyhow!(
                    "Failed to set fields for class. Already visited. {:?}",
                    class_info
                )
            })?;

            //Methods
            let mut indexed_methods = Vec::with_capacity(class_info.methods.len());

            for method_info in class_info.methods.iter() {
                let method_name = &method_info.method_name;

                let method_name_index =
                    get_index_from_pool(method_name, &mut constant_pool_map, &mut constant_pool)?;

                indexed_methods.push(IndexedMethod::new(
                    method_name_index,
                    method_info.access_flags.bits(),
                    method_info.signature.to_indexed_type(
                        &mut constant_pool,
                        &mut constant_pool_map,
                        &classes_map,
                    )?,
                ));
            }

            indexed_class.set_methods(indexed_methods).map_err(|_| {
                anyhow!(
                    "Failed to set methods for class. Already visited. {:?}",
                    class_info
                )
            })?;
        }

        let classes = classes.into_iter().map(|class| class.1).collect();

        Ok((
            BuildTimeInfo {
                indexing_time: start_time.elapsed().as_millis(),
                ..Default::default()
            },
            ClassIndex::new(constant_pool, package_index, classes),
        ))
    }

    fn sort_classes(
        package_index: &PackageIndex,
        constant_pool: &ClassIndexConstantPool,
        classes: &mut [((&AsciiStr, &AsciiStr), IndexedClass)],
    ) {
        classes.sort_by(|a, b| {
            let a_name = a.1.class_name(constant_pool);
            let b_name = b.1.class_name(constant_pool);
            a_name.cmp(b_name).then_with(|| {
                package_index
                    .package_at(a.1.package_index())
                    .package_name_with_parents_cmp(
                        package_index,
                        constant_pool,
                        &package_index
                            .package_at(b.1.package_index())
                            .package_name_with_parents(package_index, constant_pool),
                    )
            })
        });
    }
}

pub fn get_index_from_pool<'a>(
    value: &'a AsciiStr,
    map: &mut FxHashMap<&'a AsciiStr, u32>,
    pool: &mut ClassIndexConstantPool,
) -> anyhow::Result<u32> {
    Ok(if let Some(i) = map.get(value) {
        *i
    } else {
        let index = pool.add_string(value.as_bytes())?;
        map.insert(value, index);
        index
    })
}

impl Default for ClassIndexBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
struct ClassInfo {
    pub package_name: AsciiString,
    pub class_name: AsciiString,
    pub class_name_start_index: usize,
    pub access_flags: u16,
    pub enclosing_type: Option<RawEnclosingTypeInfo>,
    pub member_classes: Option<Vec<AsciiString>>,
    pub signature: RawClassSignature,
    pub fields: Vec<FieldInfo>,
    pub methods: Vec<MethodInfo>,
}

#[derive(Debug)]
struct FieldInfo {
    pub field_name: AsciiString,
    pub descriptor: RawSignatureType,
    pub access_flags: FieldAccessFlags,
}

#[derive(Debug)]
struct MethodInfo {
    pub method_name: AsciiString,
    pub signature: RawMethodSignature,
    pub access_flags: MethodAccessFlags,
}

#[derive(Default)]
pub struct BuildTimeInfo {
    pub deserialization_time: u128,
    pub file_reading_time: u128,
    pub class_reading_time: u128,
    pub indexing_time: u128,
}

impl BuildTimeInfo {
    fn merge(&mut self, other: BuildTimeInfo) {
        self.deserialization_time += other.deserialization_time;
        self.file_reading_time += other.file_reading_time;
        self.class_reading_time += other.class_reading_time;
        self.indexing_time += other.indexing_time;
    }

    pub fn total_time_millis(&self) -> u128 {
        self.deserialization_time
            + self.file_reading_time
            + self.class_reading_time
            + self.indexing_time
    }
}

impl ToString for BuildTimeInfo {
    fn to_string(&self) -> String {
        format!(
            "Deserialization: {}ms\nFile reading: {}ms\nClass reading: {}ms\nIndexing: {}ms\nTotal: {}ms",
            self.deserialization_time,
            self.file_reading_time,
            self.class_reading_time,
            self.indexing_time,
            self.total_time_millis()
        )
    }
}
