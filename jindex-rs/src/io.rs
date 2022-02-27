use std::fs::OpenOptions;
use std::io::{BufWriter, Read, Write};

use crate::class_index::{
    IndexedClass, IndexedField, IndexedMethod, IndexedMethodSignature, IndexedSignature,
};
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
        Ok(IndexedClass::new(
            reader.read_u32()?,
            reader.read_u32()?,
            reader.read_u16()?,
            Vec::read_from(reader)?,
            Vec::read_from(reader)?,
        ))
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
