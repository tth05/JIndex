use super::{ClassInfo, FieldInfo, MethodInfo};
use crate::rsplit_once;
use crate::signature::{
    InnerClassType, RawClassSignature, RawEnclosingTypeInfo, RawMethodSignature, RawSignatureType,
};
use anyhow::{anyhow, bail, Context};
use ascii::{AsAsciiStr, AsciiChar};
use compact_str::{CompactString, ToCompactString};
use jvmti_bindings::classfile::{AttributeInfo, ClassFile, ConstantPool, CpInfo};
use std::borrow::Cow;
use std::str::FromStr;

const ACC_SYNTHETIC: u16 = 0x1000;

pub(super) fn parse_class(
    bytes: &[u8],
    source_id: u32,
    input_order: u32,
) -> anyhow::Result<ClassInfo> {
    let class_file = ClassFile::parse(bytes)
        .map_err(|error| anyhow!(error))
        .with_context(|| "Failed to parse class file")?;
    let pool = &class_file.constant_pool;
    let this_class = class_name(pool, class_file.this_class)?;
    let converted = convert_enclosing_type_and_inner_classes(
        this_class,
        enclosing_method(&class_file.attributes, pool)?,
        inner_classes(&class_file.attributes, pool)?,
    )?;

    let fields = class_file
        .fields
        .iter()
        .filter_map(|field| {
            let name = match ascii_member_name(pool, field.name_index) {
                Ok(Some(name)) => name,
                Ok(None) => return None,
                Err(error) => return Some(Err(error)),
            };
            Some((|| {
                let descriptor = ascii(pool.get_utf8(field.descriptor_index)?, "field descriptor")?;
                let signature = signature(&field.attributes, pool)?.unwrap_or(descriptor);
                Ok(FieldInfo {
                    field_name: name.to_compact_string(),
                    jvm_descriptor: descriptor.to_compact_string(),
                    descriptor: RawSignatureType::from_str(signature)
                        .with_context(|| format!("Invalid field signature {signature}"))?,
                    access_flags: field.access_flags,
                })
            })())
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let methods = class_file
        .methods
        .iter()
        .filter(|method| method.access_flags & ACC_SYNTHETIC == 0)
        .filter_map(|method| {
            let name = match ascii_member_name(pool, method.name_index) {
                Ok(Some(name)) => name,
                Ok(None) => return None,
                Err(error) => return Some(Err(error)),
            };
            Some((|| {
                let descriptor =
                    ascii(pool.get_utf8(method.descriptor_index)?, "method descriptor")?;
                let signature = signature(&method.attributes, pool)?.unwrap_or(descriptor);
                let exceptions = exceptions(&method.attributes, pool)?;
                Ok(MethodInfo {
                    method_name: name.to_compact_string(),
                    jvm_descriptor: descriptor.to_compact_string(),
                    signature: RawMethodSignature::from_data(signature, &|| exceptions.as_ref())
                        .with_context(|| format!("Invalid method signature {signature}"))?,
                    access_flags: method.access_flags,
                })
            })())
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(ClassInfo {
        source_id,
        input_order,
        package_name: converted.package_name,
        class_name: converted.full_class_name,
        class_name_start_index: converted.class_name_start_index,
        access_flags: class_file.access_flags | converted.inner_class_access_flags,
        signature: class_signature(&class_file, pool)?,
        enclosing_type: converted.enclosing_type,
        member_classes: converted.member_classes,
        fields,
        methods,
    })
}

fn class_signature(
    class_file: &ClassFile,
    pool: &ConstantPool,
) -> anyhow::Result<RawClassSignature> {
    if let Some(signature) = signature(&class_file.attributes, pool)? {
        return RawClassSignature::from_str(signature).with_context(|| "Invalid class signature");
    }

    let super_class = if class_file.super_class == 0 {
        None
    } else {
        let name = ascii(
            class_name(pool, class_file.super_class)?,
            "super class name",
        )?;
        (name != "java/lang/Object").then(|| RawSignatureType::Object(name.to_compact_string()))
    };
    let interfaces = class_file
        .interfaces
        .iter()
        .map(|index| {
            ascii(class_name(pool, *index)?, "interface name")
                .map(|name| RawSignatureType::Object(name.to_compact_string()))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(RawClassSignature::new(
        super_class,
        (!interfaces.is_empty()).then_some(interfaces),
    ))
}

fn signature<'a>(
    attributes: &'a [AttributeInfo],
    pool: &'a ConstantPool,
) -> anyhow::Result<Option<&'a str>> {
    attributes
        .iter()
        .find_map(|attribute| match attribute {
            AttributeInfo::Signature { signature_index } => Some(pool.get_utf8(*signature_index)),
            _ => None,
        })
        .transpose()
        .map_err(Into::into)
}

fn exceptions<'a>(
    attributes: &'a [AttributeInfo],
    pool: &'a ConstantPool,
) -> anyhow::Result<Option<Vec<Cow<'a, str>>>> {
    let Some(classes) = attributes.iter().find_map(|attribute| match attribute {
        AttributeInfo::Exceptions {
            exception_index_table,
        } => Some(exception_index_table),
        _ => None,
    }) else {
        return Ok(None);
    };
    classes
        .iter()
        .map(|index| class_name(pool, *index).map(Cow::Borrowed))
        .collect::<anyhow::Result<Vec<_>>>()
        .map(Some)
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

fn ascii<'a>(value: &'a str, description: &str) -> anyhow::Result<&'a str> {
    value
        .is_ascii()
        .then_some(value)
        .ok_or_else(|| anyhow!("{description} is not ASCII"))
}

fn ascii_member_name(pool: &ConstantPool, index: u16) -> anyhow::Result<Option<&str>> {
    let name = pool.get_utf8(index)?;
    // Declaration names share JIndex's compact ASCII search pool. Preserve the class and exclude
    // only the unsupported declaration, matching the previous index format's boundary.
    Ok(name.is_ascii().then_some(name))
}

struct ResolvedEnclosingMethod<'a> {
    class_name: &'a str,
    method: Option<(&'a str, &'a str)>,
}

fn enclosing_method<'a>(
    attributes: &'a [AttributeInfo],
    pool: &'a ConstantPool,
) -> anyhow::Result<Option<ResolvedEnclosingMethod<'a>>> {
    let Some((class_index, method_index)) =
        attributes.iter().find_map(|attribute| match attribute {
            AttributeInfo::EnclosingMethod {
                class_index,
                method_index,
            } => Some((*class_index, *method_index)),
            _ => None,
        })
    else {
        return Ok(None);
    };
    Ok(Some(ResolvedEnclosingMethod {
        class_name: class_name(pool, class_index)?,
        method: (method_index != 0)
            .then(|| name_and_type(pool, method_index))
            .transpose()?,
    }))
}

struct ResolvedInnerClass<'a> {
    inner_class_name: &'a str,
    outer_class_name: Option<&'a str>,
    inner_name: Option<&'a str>,
    access_flags: u16,
}

fn inner_classes<'a>(
    attributes: &'a [AttributeInfo],
    pool: &'a ConstantPool,
) -> anyhow::Result<Vec<ResolvedInnerClass<'a>>> {
    let Some(classes) = attributes.iter().find_map(|attribute| match attribute {
        AttributeInfo::InnerClasses { classes } => Some(classes),
        _ => None,
    }) else {
        return Ok(Vec::new());
    };
    classes
        .iter()
        .map(|entry| {
            Ok(ResolvedInnerClass {
                inner_class_name: class_name(pool, entry.inner_class_info_index)?,
                outer_class_name: (entry.outer_class_info_index != 0)
                    .then(|| class_name(pool, entry.outer_class_info_index))
                    .transpose()?,
                inner_name: (entry.inner_name_index != 0)
                    .then(|| pool.get_utf8(entry.inner_name_index))
                    .transpose()?,
                access_flags: entry.inner_class_access_flags,
            })
        })
        .collect()
}

struct ConvertedInnerClassInfo {
    package_name: CompactString,
    full_class_name: CompactString,
    class_name_start_index: usize,
    inner_class_access_flags: u16,
    enclosing_type: Option<RawEnclosingTypeInfo>,
    member_classes: Option<Vec<CompactString>>,
}

fn convert_enclosing_type_and_inner_classes(
    this_name: &str,
    enclosing_method: Option<ResolvedEnclosingMethod<'_>>,
    inner_classes: Vec<ResolvedInnerClass<'_>>,
) -> anyhow::Result<ConvertedInnerClassInfo> {
    let this_name = this_name.as_ascii_str()?;
    let (package_name, class_name) = rsplit_once(this_name, AsciiChar::Slash);
    let mut class_name_start_index = 0;
    let mut access_flags = 0;
    let mut enclosing_type_info = None;
    let mut self_inner_class_index = None;

    if let Some((index, entry)) = inner_classes
        .iter()
        .enumerate()
        .find(|(_, entry)| entry.inner_class_name == this_name)
    {
        access_flags = entry.access_flags;
        let inner_class_type = if entry.inner_name.is_none() {
            InnerClassType::Anonymous
        } else if entry.outer_class_name.is_none() {
            InnerClassType::Local
        } else {
            InnerClassType::Member
        };

        if let Some(enclosing) = enclosing_method {
            let (method_name, method_descriptor) = match enclosing.method {
                Some((name, descriptor)) => (
                    Some(ascii(name, "enclosing method name")?.to_compact_string()),
                    Some(RawMethodSignature::from_data(descriptor, &|| None)?),
                ),
                None => (None, None),
            };
            enclosing_type_info = Some(RawEnclosingTypeInfo::new(
                Some(ascii(enclosing.class_name, "enclosing class name")?.to_compact_string()),
                inner_class_type,
                method_name,
                method_descriptor,
            ));
        } else {
            let (outer_name, inner_name_start) = extract_outer_and_inner_name(class_name, entry)?;
            class_name_start_index = inner_name_start;
            enclosing_type_info = Some(RawEnclosingTypeInfo::new(
                Some(outer_name),
                inner_class_type,
                None,
                None,
            ));
        }
        self_inner_class_index = Some(index);
    }

    let member_classes = inner_classes
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            (Some(index) != self_inner_class_index
                && entry.outer_class_name == Some(this_name.as_str())
                && entry.inner_name.is_some())
            .then(|| {
                ascii(entry.inner_class_name, "member class name")
                    .map(|name| name.to_compact_string())
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(ConvertedInnerClassInfo {
        package_name: package_name.to_compact_string(),
        full_class_name: class_name.to_compact_string(),
        class_name_start_index,
        inner_class_access_flags: access_flags,
        enclosing_type: enclosing_type_info,
        member_classes: (!member_classes.is_empty()).then_some(member_classes),
    })
}

fn extract_outer_and_inner_name(
    original_class_name: &ascii::AsciiStr,
    entry: &ResolvedInnerClass<'_>,
) -> anyhow::Result<(CompactString, usize)> {
    if let (Some(inner_name), Some(outer_name)) = (entry.inner_name, entry.outer_class_name) {
        if !inner_name.is_empty() {
            return Ok((
                ascii(outer_name, "outer class name")?.to_compact_string(),
                checked_name_start(original_class_name.len(), inner_name.len())?,
            ));
        }
    }

    if let Some(outer_name) = entry.outer_class_name {
        let inner_name_length = entry
            .inner_class_name
            .len()
            .checked_sub(outer_name.len() + 1)
            .ok_or_else(|| anyhow!("Inner class name is shorter than its outer class name"))?;
        return Ok((
            ascii(outer_name, "outer class name")?.to_compact_string(),
            checked_name_start(original_class_name.len(), inner_name_length)?,
        ));
    }

    let index = entry
        .inner_class_name
        .rfind('$')
        .ok_or_else(|| anyhow!("No '$' found in inner class name"))?;
    Ok((
        ascii(&entry.inner_class_name[..index], "derived outer class name")?.to_compact_string(),
        checked_name_start(
            original_class_name.len(),
            entry.inner_class_name.len() - (index + 1),
        )?,
    ))
}

fn checked_name_start(class_name_length: usize, inner_name_length: usize) -> anyhow::Result<usize> {
    class_name_length
        .checked_sub(inner_name_length)
        .ok_or_else(|| anyhow!("Inner class name is longer than the class name"))
}
