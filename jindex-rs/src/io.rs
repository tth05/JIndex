use std::fs::OpenOptions;
use std::io::{Read, Write};

use crate::class_index::{IndexedClass, IndexedSignature};
use speedy::{Context, Readable, Reader, Writable, Writer};
use zip::write::FileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::constant_pool::ClassIndexConstantPool;
use crate::ClassIndex;

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
        let class = IndexedClass::new(reader.read_u32()?, reader.read_u32()?, reader.read_u16()?);
        class.set_fields(Vec::read_from(reader)?);
        class.set_methods(Vec::read_from(reader)?);
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
        self.access_flags().write_to(writer)?;
        self.fields().write_to(writer)?;
        self.methods().write_to(writer)?;
        Ok(())
    }
}

impl<'a, C> Readable<'a, C> for IndexedSignature
where
    C: Context,
{
    fn read_from<R: Reader<'a, C>>(reader: &mut R) -> Result<Self, C::Error> {
        Ok(match reader.read_u8()? {
            0 => IndexedSignature::Primitive(match reader.read_u8()? {
                0 => jni::signature::Primitive::Boolean,
                1 => jni::signature::Primitive::Byte,
                2 => jni::signature::Primitive::Char,
                3 => jni::signature::Primitive::Double,
                4 => jni::signature::Primitive::Float,
                5 => jni::signature::Primitive::Int,
                6 => jni::signature::Primitive::Long,
                7 => jni::signature::Primitive::Short,
                _ => unreachable!(),
            }),
            1 => IndexedSignature::Object(reader.read_u32()?),
            2 => IndexedSignature::Array(Box::new(IndexedSignature::read_from(reader)?)),
            3 => IndexedSignature::Void,
            _ => IndexedSignature::Unresolved,
        })
    }
}

impl<C> Writable<C> for IndexedSignature
where
    C: Context,
{
    fn write_to<T: ?Sized + Writer<C>>(&self, writer: &mut T) -> Result<(), C::Error> {
        match self {
            IndexedSignature::Primitive(p) => {
                writer.write_u8(0)?;
                writer.write_u8(match p {
                    jni::signature::Primitive::Boolean => 0,
                    jni::signature::Primitive::Byte => 1,
                    jni::signature::Primitive::Char => 2,
                    jni::signature::Primitive::Double => 3,
                    jni::signature::Primitive::Float => 4,
                    jni::signature::Primitive::Int => 5,
                    jni::signature::Primitive::Long => 6,
                    jni::signature::Primitive::Short => 7,
                    _ => unreachable!(),
                })?;
            }
            IndexedSignature::Object(i) => {
                writer.write_u8(1)?;
                writer.write_u32(*i)?;
            }
            IndexedSignature::Array(b) => {
                writer.write_u8(2)?;
                b.write_to(writer)?;
            }
            IndexedSignature::Void => writer.write_u8(3)?,
            IndexedSignature::Unresolved => writer.write_u8(4)?,
        }
        Ok(())
    }
}
