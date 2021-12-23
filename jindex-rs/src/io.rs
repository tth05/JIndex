use crate::class_index::{IndexedClass, IndexedPackage};
use crate::constant_pool::ConstantPoolStringView;
use ascii::AsciiString;
use speedy::{Context, LittleEndian, Readable, Reader, Writable, Writer};

fn write_ascii_string<T>(str: AsciiString, writer: &mut T) -> Result<(), speedy::Error>
where
    T: ?Sized + Writer<LittleEndian>,
{
    let bytes = str.as_bytes();
    writer.write_u16(bytes.len() as u16)?;
    writer.write_bytes(bytes)?;
    Ok(())
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
            reader.read_u32()?,
        ))
    }
}

impl<C> Writable<C> for IndexedClass
where
    C: Context,
{
    fn write_to<T: ?Sized + Writer<C>>(&self, writer: &mut T) -> Result<(), C::Error> {
        writer.write_u32(self.class_name_index())?;
        writer.write_u32(self.method_data_index())?;
        writer.write_u16(self.method_count())?;
        writer.write_u32(self.package_index())?;
        Ok(())
    }
}

impl<'a, C> Readable<'a, C> for IndexedPackage
where
    C: Context,
{
    fn read_from<R: Reader<'a, C>>(reader: &mut R) -> Result<Self, C::Error> {
        Ok(IndexedPackage::new(
            reader.read_u32()?,
            reader.read_u32()?,
            reader.read_u32()?,
        ))
    }
}

impl<C> Writable<C> for IndexedPackage
where
    C: Context,
{
    fn write_to<T: ?Sized + Writer<C>>(&self, writer: &mut T) -> Result<(), C::Error> {
        writer.write_u32(self.index())?;
        writer.write_u32(self.package_name_index())?;
        writer.write_u32(self.previous_package_index())?;
        Ok(())
    }
}
