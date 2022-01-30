use std::fs::OpenOptions;
use std::io::{BufReader, BufWriter, Read, Write};

use crate::class_index::IndexedClass;
use flate2::bufread::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use speedy::{Context, Readable, Reader, Writable, Writer};

use crate::constant_pool::ClassIndexConstantPool;
use crate::ClassIndex;

pub fn load_class_index_from_file(path: String) -> ClassIndex {
    let file = OpenOptions::new().read(true).open(path).unwrap();

    let file_size = file.metadata().unwrap().len();

    let reader = BufReader::new(file);
    let mut output_buf = Vec::with_capacity(file_size as usize);
    let mut decoder = GzDecoder::new(reader);
    decoder
        .read_to_end(&mut output_buf)
        .expect("Decompression failed");

    ClassIndex::read_from_buffer(&output_buf).expect("Deserialization failed")
}

pub fn save_class_index_to_file(class_index: &ClassIndex, path: String) {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(path)
        .unwrap();

    let serialized_buf = class_index.write_to_vec().expect("Serialization failed");

    let mut writer = BufWriter::new(file);
    let output_buf = Vec::with_capacity(serialized_buf.len() / 2);

    let mut encoder = GzEncoder::new(output_buf, Compression::best());
    encoder
        .write_all(&serialized_buf)
        .expect("Compression failed");
    writer
        .write_all(&encoder.finish().unwrap())
        .expect("Write to file failed");
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
