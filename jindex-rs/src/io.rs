use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::ptr::write;

use crate::class_index::{ClassIndex, IndexedClass};
use speedy::{Context, Readable, Reader, Writable, Writer};
use zip::write::FileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::constant_pool::ClassIndexConstantPool;
use crate::signature::{
    IndexedClassSignature, IndexedEnclosingTypeInfo, IndexedMethodSignature, IndexedSignatureType,
};

pub fn load_class_index_from_file(path: String) -> ClassIndex {
    let mut archive = ZipArchive::new(OpenOptions::new().read(true).open(path).unwrap()).unwrap();
    let mut file = archive.by_index(0).unwrap();
    let file_size = file.size();

    let mut output_buf = Vec::with_capacity(file_size as usize);
    file.read_to_end(&mut output_buf)
        .expect("Unable to read from zip file");

    ClassIndex::read_from_buffer(&output_buf).expect("Deserialization failed")
}

pub fn save_class_index_to_file(class_index: &ClassIndex, path: String) {
    let mut file = ZipWriter::new(
        OpenOptions::new()
            .write(true)
            .create(true)
            .open(path)
            .unwrap(),
    );

    let serialized_buf = class_index.write_to_vec().expect("Serialization failed");

    file.start_file("index", FileOptions::default())
        .expect("Unable to start file");
    file.write_all(&serialized_buf)
        .expect("Unable to write file contents");
    file.finish().expect("Failed to write zip file");
}

impl<'a, C> Readable<'a, C> for ClassIndex
where
    C: Context,
{
    fn read_from<R: Reader<'a, C>>(reader: &mut R) -> Result<Self, C::Error> {
        Ok(ClassIndex::new(
            ClassIndexConstantPool::read_from(reader)?,
            Vec::read_from(reader)?,
        ))
    }
}

impl<C> Writable<C> for ClassIndex
where
    C: Context,
{
    fn write_to<T: ?Sized + Writer<C>>(&self, writer: &mut T) -> Result<(), C::Error> {
        self.constant_pool().write_to(writer)?;
        self.classes().write_to(writer)?;
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
        class.set_signature(IndexedClassSignature::read_from(reader)?);
        class.set_enclosing_type_info(IndexedEnclosingTypeInfo::read_from(reader)?);
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
        self.signature().write_to(writer)?;
        self.enclosing_type_info().write_to(writer)?;
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

impl<'a, C> Readable<'a, C> for IndexedEnclosingTypeInfo
where
    C: Context,
{
    fn read_from<R: Reader<'a, C>>(reader: &mut R) -> Result<Self, C::Error> {
        Ok(IndexedEnclosingTypeInfo::new(
            reader.read_u32()?,
            reader.read_value::<Option<u32>>()?,
            reader.read_value::<Option<IndexedMethodSignature>>()?,
        ))
    }
}

impl<C> Writable<C> for IndexedEnclosingTypeInfo
where
    C: Context,
{
    fn write_to<T: ?Sized + Writer<C>>(&self, writer: &mut T) -> Result<(), C::Error> {
        writer.write_u32(*self.class_name())?;
        self.method_name().write_to(writer)?;
        self.method_descriptor().write_to(writer)?;
        Ok(())
    }
}
