use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::time::Instant;

use crate::builder::BuildTimeInfo;
use crate::class_index::ClassIndex;
use crate::class_index_members::IndexedClass;
use crate::package_index::IndexedPackage;
use anyhow::Context as AnyhowContext;
use speedy::{Context, Readable, Reader, Writable, Writer};
use zip::write::FileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::signature::{IndexedEnclosingTypeInfo, IndexedMethodSignature, IndexedSignatureType};

pub fn load_class_index_from_file(path: String) -> anyhow::Result<(BuildTimeInfo, ClassIndex)> {
    let now = Instant::now();
    let mut archive = ZipArchive::new(OpenOptions::new().read(true).open(path)?)?;
    let mut file = archive
        .by_index(0)
        .with_context(|| "File with index 0 not found")?;
    let file_size = file.size();

    let mut output_buf = Vec::with_capacity(file_size as usize);
    file.read_to_end(&mut output_buf)
        .with_context(|| "Failed to read first file")?;

    let mut info = BuildTimeInfo {
        file_reading_time: now.elapsed().as_millis(),
        ..Default::default()
    };

    let now = Instant::now();
    let result = ClassIndex::read_from_buffer(&output_buf)
        .with_context(|| "Failed to deserialize ClassIndex")?;
    info.deserialization_time = now.elapsed().as_millis();

    Ok((info, result))
}

pub fn save_class_index_to_file(class_index: &ClassIndex, path: String) -> anyhow::Result<()> {
    let mut file = ZipWriter::new(OpenOptions::new().write(true).create(true).open(path)?);

    let serialized_buf = class_index
        .write_to_vec()
        .with_context(|| "ClassIndex serialization failed")?;

    file.start_file("index", FileOptions::default())
        .with_context(|| "Failed to start file")?;
    file.write_all(&serialized_buf)
        .with_context(|| "Unable to write file contents")?;
    file.finish().with_context(|| "Failed to finish zip file")?;
    Ok(())
}

impl<'a, C> Readable<'a, C> for ClassIndex
where
    C: Context,
{
    fn read_from<R: Reader<'a, C>>(reader: &mut R) -> Result<Self, C::Error> {
        Ok(ClassIndex::new(
            reader.read_value()?,
            reader.read_value()?,
            reader.read_value()?,
        ))
    }
}

impl<C> Writable<C> for ClassIndex
where
    C: Context,
{
    fn write_to<T: ?Sized + Writer<C>>(&self, writer: &mut T) -> Result<(), C::Error> {
        self.constant_pool().write_to(writer)?;
        self.package_index().write_to(writer)?;
        self.classes().write_to(writer)?;
        Ok(())
    }
}

impl<'a, C> Readable<'a, C> for IndexedPackage
where
    C: Context,
{
    fn read_from<R: Reader<'a, C>>(reader: &mut R) -> Result<Self, C::Error> {
        let mut package = IndexedPackage::new(reader.read_value()?, reader.read_value()?);
        reader.read_value::<Vec<u32>>()?.iter().for_each(|index| {
            package.add_sub_package(*index);
        });
        reader.read_value::<Vec<u32>>()?.iter().for_each(|index| {
            package.add_class(*index);
        });
        Ok(package)
    }
}

impl<C> Writable<C> for IndexedPackage
where
    C: Context,
{
    fn write_to<T: ?Sized + Writer<C>>(&self, writer: &mut T) -> Result<(), C::Error> {
        self.package_name_index().write_to(writer)?;
        self.previous_package_index().write_to(writer)?;
        self.sub_packages_indices().write_to(writer)?;
        self.sub_classes_indices().write_to(writer)?;
        Ok(())
    }
}

impl<'a, C> Readable<'a, C> for IndexedClass
where
    C: Context,
{
    fn read_from<R: Reader<'a, C>>(reader: &mut R) -> Result<Self, C::Error> {
        let class = IndexedClass::new(
            reader.read_u32()?,
            reader.read_u32()?,
            reader.read_u8()?,
            reader.read_u16()?,
        );
        class.set_index(reader.read_value()?);
        class.set_signature(reader.read_value()?);
        if let Some(info) = reader.read_value::<Option<IndexedEnclosingTypeInfo>>()? {
            class.set_enclosing_type_info(info);
        }
        reader
            .read_value::<Vec<u32>>()?
            .into_iter()
            .for_each(|c| class.add_member_class(c));
        class.set_fields(Vec::read_from(reader)?).unwrap();
        class.set_methods(Vec::read_from(reader)?).unwrap();
        Ok(class)
    }
}

impl<C> Writable<C> for IndexedClass
where
    C: Context,
{
    fn write_to<T: ?Sized + Writer<C>>(&self, writer: &mut T) -> Result<(), C::Error> {
        self.package_index().write_to(writer)?;
        self.class_name_index().write_to(writer)?;
        self.class_name_start_index().write_to(writer)?;
        self.access_flags().write_to(writer)?;
        self.index().write_to(writer)?;
        self.signature().write_to(writer)?;
        self.enclosing_type_info().write_to(writer)?;
        self.member_classes().write_to(writer)?;
        self.fields().write_to(writer)?;
        self.methods().write_to(writer)?;
        Ok(())
    }
}

impl<'a, C> Readable<'a, C> for IndexedSignatureType
where
    C: Context,
{
    fn read_from<R: Reader<'a, C>>(reader: &mut R) -> Result<Self, C::Error> {
        Ok(match reader.read_u8()? {
            0 => IndexedSignatureType::Unresolved,
            1 => IndexedSignatureType::Primitive(match reader.read_u8()? {
                0 => jni::signature::Primitive::Boolean,
                1 => jni::signature::Primitive::Byte,
                2 => jni::signature::Primitive::Char,
                3 => jni::signature::Primitive::Double,
                4 => jni::signature::Primitive::Float,
                5 => jni::signature::Primitive::Int,
                6 => jni::signature::Primitive::Long,
                7 => jni::signature::Primitive::Short,
                8 => jni::signature::Primitive::Void,
                _ => unreachable!(),
            }),
            2 => IndexedSignatureType::Generic(reader.read_u32()?),
            3 => IndexedSignatureType::Object(reader.read_u32()?),
            4 => {
                IndexedSignatureType::ObjectPlus(Box::new(IndexedSignatureType::read_from(reader)?))
            }
            5 => IndexedSignatureType::ObjectMinus(Box::new(IndexedSignatureType::read_from(
                reader,
            )?)),
            6 => IndexedSignatureType::ObjectTypeBounds(Box::new(<_>::read_from(reader)?)),
            7 => IndexedSignatureType::ObjectInnerClass(Box::new(<_>::read_from(reader)?)),
            8 => IndexedSignatureType::Array(Box::new(<_>::read_from(reader)?)),
            _ => unreachable!(),
        })
    }
}

impl<C> Writable<C> for IndexedSignatureType
where
    C: Context,
{
    fn write_to<T: ?Sized + Writer<C>>(&self, writer: &mut T) -> Result<(), C::Error> {
        match self {
            IndexedSignatureType::Unresolved => writer.write_u8(0)?,
            IndexedSignatureType::Primitive(p) => {
                writer.write_u8(1)?;
                writer.write_u8(match p {
                    jni::signature::Primitive::Boolean => 0,
                    jni::signature::Primitive::Byte => 1,
                    jni::signature::Primitive::Char => 2,
                    jni::signature::Primitive::Double => 3,
                    jni::signature::Primitive::Float => 4,
                    jni::signature::Primitive::Int => 5,
                    jni::signature::Primitive::Long => 6,
                    jni::signature::Primitive::Short => 7,
                    jni::signature::Primitive::Void => 8,
                })?;
            }
            IndexedSignatureType::Generic(i) => {
                writer.write_u8(2)?;
                writer.write_u32(*i)?;
            }
            IndexedSignatureType::Object(i) => {
                writer.write_u8(3)?;
                writer.write_u32(*i)?;
            }
            IndexedSignatureType::ObjectPlus(i) => {
                writer.write_u8(4)?;
                i.write_to(writer)?;
            }
            IndexedSignatureType::ObjectMinus(i) => {
                writer.write_u8(5)?;
                i.write_to(writer)?;
            }
            IndexedSignatureType::ObjectTypeBounds(i) => {
                writer.write_u8(6)?;
                i.write_to(writer)?;
            }
            IndexedSignatureType::ObjectInnerClass(i) => {
                writer.write_u8(7)?;
                i.write_to(writer)?;
            }
            IndexedSignatureType::Array(b) => {
                writer.write_u8(8)?;
                b.write_to(writer)?;
            }
        }
        Ok(())
    }
}

impl<'a, C> Readable<'a, C> for IndexedMethodSignature
where
    C: Context,
{
    fn read_from<R: Reader<'a, C>>(reader: &mut R) -> Result<Self, C::Error> {
        Ok(IndexedMethodSignature::new(
            reader.read_value()?,
            reader.read_value()?,
            reader.read_value()?,
            reader.read_value()?,
        ))
    }
}

impl<C> Writable<C> for IndexedMethodSignature
where
    C: Context,
{
    fn write_to<T: ?Sized + Writer<C>>(&self, writer: &mut T) -> Result<(), C::Error> {
        self.generic_data().write_to(writer)?;
        self.parameters().write_to(writer)?;
        self.return_type().write_to(writer)?;
        self.exceptions().write_to(writer)?;
        Ok(())
    }
}

impl<'a, C> Readable<'a, C> for IndexedEnclosingTypeInfo
where
    C: Context,
{
    fn read_from<R: Reader<'a, C>>(reader: &mut R) -> Result<Self, C::Error> {
        Ok(IndexedEnclosingTypeInfo::new(
            reader.read_value()?,
            reader.read_value()?,
            reader.read_value()?,
            reader.read_value()?,
        ))
    }
}

impl<C> Writable<C> for IndexedEnclosingTypeInfo
where
    C: Context,
{
    fn write_to<T: ?Sized + Writer<C>>(&self, writer: &mut T) -> Result<(), C::Error> {
        self.class_name().write_to(writer)?;
        self.inner_class_type().write_to(writer)?;
        self.method_name().write_to(writer)?;
        self.method_descriptor().write_to(writer)?;
        Ok(())
    }
}
