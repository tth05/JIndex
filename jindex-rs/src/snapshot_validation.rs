use crate::class_index_members::IndexedClass;
use crate::constant_pool::ClassIndexConstantPool;
use crate::package_index::PackageIndex;
use crate::semantic_index::SemanticIndex;
use anyhow::ensure;

pub(crate) fn validate(
    pool: &ClassIndexConstantPool,
    packages: &PackageIndex,
    classes: &[IndexedClass],
    semantic: &SemanticIndex,
) -> anyhow::Result<()> {
    let entries = pool.entry_offsets();
    packages.validate_snapshot(pool, &entries, classes)?;
    let mut previous = None;
    for (index, class) in classes.iter().enumerate() {
        ensure!(
            class.index() as usize == index,
            "Class ordinal does not match its position"
        );
        ensure!(
            entries.contains(&class.class_name_index()),
            "Invalid class name offset"
        );
        let name = class.class_name_bytes(pool);
        ensure!(
            !name.is_empty() && usize::from(class.class_name_start_index()) < name.len(),
            "Invalid simple class name range"
        );
        ensure!(
            previous.is_none_or(|previous| previous <= name),
            "Classes are not sorted by name"
        );
        previous = Some(name);
        for member in class.member_classes().iter().copied() {
            ensure!(
                (member as usize) < classes.len(),
                "Invalid member class index"
            );
        }
        class
            .signature()
            .validate_snapshot(&entries, classes.len())?;
        if let Some(enclosing) = class.enclosing_type_info() {
            enclosing.validate_snapshot(&entries, classes.len())?;
        }
        ensure!(
            class.fields().len() <= u16::MAX as usize && class.methods().len() <= u16::MAX as usize,
            "Too many members in a class"
        );
        for field in class.fields() {
            ensure!(
                entries.contains(&field.field_name_index()),
                "Invalid field name offset"
            );
            field
                .field_signature()
                .validate_snapshot(&entries, classes.len())?;
        }
        for method in class.methods() {
            ensure!(
                entries.contains(&method.method_name_index()),
                "Invalid method name offset"
            );
            method
                .method_signature()
                .validate_snapshot(&entries, classes.len())?;
        }
    }
    semantic.validate_snapshot(classes, pool)
}
