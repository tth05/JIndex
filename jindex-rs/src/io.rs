use std::fs::OpenOptions;
use std::io::{BufReader, BufWriter, Read, Seek, Write};
use std::time::Instant;

use crate::builder::BuildTimeInfo;
use crate::class_index::ClassIndex;
use crate::class_index_members::IndexedClass;
use crate::package_index::IndexedPackage;
use anyhow::{bail, ensure, Context as AnyhowContext};
use speedy::{Context, Readable, Reader, Writable, Writer};
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::signature::{IndexedEnclosingTypeInfo, IndexedMethodSignature, IndexedSignatureType};

const SNAPSHOT_MAGIC: &[u8; 8] = b"JINDEX\0\0";
const SNAPSHOT_VERSION: u16 = 5;
const SNAPSHOT_HEADER_LENGTH: usize = SNAPSHOT_MAGIC.len() + size_of::<u16>();
const SNAPSHOT_COMPRESSION_LEVEL: i32 = 3;

thread_local! {
    static SIGNATURE_DEPTH: std::cell::Cell<u16> = const { std::cell::Cell::new(0) };
}

struct SignatureDepth;

impl SignatureDepth {
    fn enter() -> Option<Self> {
        SIGNATURE_DEPTH.with(|depth| {
            if depth.get() >= 256 {
                return None;
            }
            depth.set(depth.get() + 1);
            Some(Self)
        })
    }
}

impl Drop for SignatureDepth {
    fn drop(&mut self) {
        SIGNATURE_DEPTH.with(|depth| depth.set(depth.get() - 1));
    }
}

pub fn load_class_index_from_file(path: String) -> anyhow::Result<(BuildTimeInfo, ClassIndex)> {
    let now = Instant::now();
    let archive = ZipArchive::new(OpenOptions::new().read(true).open(path)?)?;
    let mut info = BuildTimeInfo {
        class_reading_time: now.elapsed().as_millis(),
        ..Default::default()
    };
    let now = Instant::now();
    let result = read_snapshot_archive(archive)?;
    info.deserialization_time = now.elapsed().as_millis();
    Ok((info, result))
}

fn read_snapshot_archive(mut archive: ZipArchive<impl Read + Seek>) -> anyhow::Result<ClassIndex> {
    let compression_method = archive
        .by_index_raw(0)
        .with_context(|| "File with index 0 not found")?
        .compression();
    ensure!(
        compression_method == CompressionMethod::Zstd,
        "Unsupported JIndex snapshot compression {:?}; expected Zstandard",
        compression_method
    );
    let file = archive
        .by_index(0)
        .with_context(|| "File with index 0 not found")?;
    read_snapshot(file)
}

fn read_snapshot(file: impl Read) -> anyhow::Result<ClassIndex> {
    // Buffer outside Speedy so we retain its unread bytes and can verify EOF and the ZIP CRC.
    let mut file = BufReader::with_capacity(64 * 1024, file);
    let mut header = [0; SNAPSHOT_HEADER_LENGTH];
    file.read_exact(&mut header)
        .with_context(|| "Unsupported JIndex snapshot: missing format header")?;
    validate_snapshot_header(&header)?;
    let result = ClassIndex::read_from_stream_unbuffered(&mut file)
        .with_context(|| "Failed to deserialize ClassIndex")?;
    ensure!(
        file.read(&mut [0])? == 0,
        "Unsupported JIndex snapshot: trailing payload bytes"
    );
    Ok(result)
}

fn validate_snapshot_header(bytes: &[u8]) -> anyhow::Result<()> {
    ensure!(
        bytes.len() >= SNAPSHOT_HEADER_LENGTH,
        "Unsupported JIndex snapshot: missing format header"
    );
    ensure!(
        &bytes[..SNAPSHOT_MAGIC.len()] == SNAPSHOT_MAGIC,
        "Unsupported JIndex snapshot: invalid magic"
    );
    let version =
        u16::from_le_bytes([bytes[SNAPSHOT_MAGIC.len()], bytes[SNAPSHOT_MAGIC.len() + 1]]);
    if version != SNAPSHOT_VERSION {
        bail!(
            "Unsupported JIndex snapshot version {}; expected {}",
            version,
            SNAPSHOT_VERSION
        );
    }
    Ok(())
}

pub fn save_class_index_to_file(class_index: &ClassIndex, path: String) -> anyhow::Result<()> {
    let mut file = ZipWriter::new(
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?,
    );

    file.start_file(
        "index",
        FileOptions::default()
            .compression_method(CompressionMethod::Zstd)
            .compression_level(Some(SNAPSHOT_COMPRESSION_LEVEL)),
    )
    .with_context(|| "Failed to start file")?;
    file.write_all(SNAPSHOT_MAGIC)
        .with_context(|| "Unable to write snapshot magic")?;
    file.write_all(&SNAPSHOT_VERSION.to_le_bytes())
        .with_context(|| "Unable to write snapshot version")?;
    {
        let mut output = BufWriter::with_capacity(64 * 1024, &mut file);
        class_index
            .write_to_stream(&mut output)
            .with_context(|| "ClassIndex serialization failed")?;
        output
            .flush()
            .with_context(|| "Unable to write file contents")?;
    }
    file.finish().with_context(|| "Failed to finish zip file")?;
    Ok(())
}

impl<'a, C> Readable<'a, C> for ClassIndex
where
    C: Context,
{
    fn read_from<R: Reader<'a, C>>(reader: &mut R) -> Result<Self, C::Error> {
        let pool = reader.read_value()?;
        let packages = reader.read_value()?;
        let classes: Vec<IndexedClass> = reader.read_value()?;
        let semantic = reader.read_value()?;
        crate::snapshot_validation::validate(&pool, &packages, &classes, &semantic)
            .map_err(|error| speedy::Error::custom(format!("Invalid JIndex snapshot: {error}")))?;
        Ok(ClassIndex::new(pool, packages, classes, semantic))
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
        self.semantic_index().write_to(writer)?;
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
        let mut class = IndexedClass::new(
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
        let _depth = SignatureDepth::enter().ok_or_else(|| {
            speedy::Error::custom("Snapshot signature nesting exceeds 256 levels")
        })?;
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
                tag => {
                    return Err(speedy::Error::custom(format!(
                        "Unknown JIndex primitive tag {tag}"
                    ))
                    .into())
                }
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
            tag => {
                return Err(
                    speedy::Error::custom(format!("Unknown JIndex signature tag {tag}")).into(),
                )
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn empty_snapshot() -> Vec<u8> {
        let (_, index) =
            crate::builder::workers::create_class_index_from_bytes(Vec::new()).unwrap();
        let mut bytes = SNAPSHOT_MAGIC.to_vec();
        bytes.extend_from_slice(&SNAPSHOT_VERSION.to_le_bytes());
        index.write_to_stream(&mut bytes).unwrap();
        bytes
    }

    #[test]
    fn valid_zip_crc_does_not_make_invalid_snapshot_ranges_valid() {
        use crate::semantic_index::{DescriptorPool, ReferenceIndexData, SemanticIndex};
        for (sources, field_offsets, reference_offsets) in [
            (vec![7], vec![0], vec![0]),
            (vec![], vec![1], vec![0]),
            (vec![], vec![0], vec![1]),
        ] {
            let mut pool = crate::constant_pool::ClassIndexConstantPool::new(0);
            let packages = crate::package_index::PackageIndex::new(&mut pool).unwrap();
            let semantic = SemanticIndex::new(
                sources,
                DescriptorPool::default(),
                field_offsets,
                vec![0],
                vec![],
                vec![],
                vec![],
                vec![],
                ReferenceIndexData {
                    offsets: reference_offsets,
                    literal_offsets: vec![0],
                    literal_posting_offsets: vec![0],
                    ..Default::default()
                },
            );
            let mut payload = SNAPSHOT_MAGIC.to_vec();
            payload.extend_from_slice(&SNAPSHOT_VERSION.to_le_bytes());
            (pool, packages, Vec::<IndexedClass>::new(), semantic)
                .write_to_stream(&mut payload)
                .unwrap();
            let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
            zip.start_file(
                "index",
                FileOptions::default().compression_method(CompressionMethod::Zstd),
            )
            .unwrap();
            zip.write_all(&payload).unwrap();
            let bytes = zip.finish().unwrap().into_inner();
            let result = std::panic::catch_unwind(|| {
                read_snapshot_archive(ZipArchive::new(Cursor::new(bytes)).unwrap())
            });
            assert!(
                result.is_ok(),
                "Malformed snapshot panicked instead of returning an error"
            );
            assert!(
                result.unwrap().is_err(),
                "Accepted invalid cross-section ranges with a valid ZIP CRC"
            );
        }
    }

    #[test]
    fn snapshot_rejects_invalid_class_and_package_links_without_panicking() {
        use crate::constant_pool::ClassIndexConstantPool;
        use crate::semantic_index::{DescriptorPool, ReferenceIndexData, SemanticIndex};
        use crate::signature::IndexedClassSignature;
        for corruption in 0..5 {
            let mut pool = ClassIndexConstantPool::new(0);
            let root_name = pool.add_string(b"").unwrap();
            let class_name = pool.add_string(b"Fixture").unwrap();
            let root = IndexedPackage::new(if corruption == 1 { 99 } else { root_name }, 0);
            root.add_class(if corruption == 2 { 99 } else { 0 });
            let mut class =
                IndexedClass::new(0, if corruption == 3 { 99 } else { class_name }, 0, 0);
            class.set_index(if corruption == 4 { 99 } else { 0 });
            class.set_signature(IndexedClassSignature::read_from_buffer(&[0, 0, 0]).unwrap());
            class.set_fields(vec![]).unwrap();
            class.set_methods(vec![]).unwrap();
            let semantic = SemanticIndex::new(
                vec![0],
                DescriptorPool::default(),
                vec![0, 0],
                vec![0, 0],
                vec![],
                vec![],
                vec![],
                vec![],
                ReferenceIndexData {
                    offsets: vec![0, 0],
                    literal_offsets: vec![0],
                    literal_posting_offsets: vec![0],
                    ..Default::default()
                },
            );
            let payload = (pool, vec![root], vec![class], semantic)
                .write_to_vec()
                .unwrap();
            let result = std::panic::catch_unwind(|| ClassIndex::read_from_buffer(&payload));
            assert!(result.is_ok(), "Corruption {corruption} caused a panic");
            assert_eq!(
                corruption == 0,
                result.unwrap().is_ok(),
                "Corruption {corruption} had an incorrect verdict"
            );
        }
    }

    #[test]
    fn signature_nesting_is_bounded_and_a_failed_read_does_not_poison_the_next() {
        let mut payload = vec![8; 300];
        payload.extend_from_slice(&[1, 5]);
        assert!(IndexedSignatureType::read_from_buffer(&payload).is_err());
        assert!(IndexedSignatureType::read_from_buffer(&[8, 1, 5]).is_ok());
    }

    #[test]
    fn streaming_snapshot_round_trip_rejects_truncation_and_trailing_bytes() {
        let bytes = empty_snapshot();
        let loaded = read_snapshot(&bytes[..]).unwrap();
        assert_eq!(
            &bytes[SNAPSHOT_HEADER_LENGTH..],
            &loaded.write_to_vec().unwrap()
        );
        for end in 0..bytes.len() {
            assert!(
                read_snapshot(&bytes[..end]).is_err(),
                "Accepted truncated payload of {end} bytes"
            );
        }
        let mut trailing = bytes;
        trailing.push(1);
        assert!(read_snapshot(&trailing[..])
            .err()
            .unwrap()
            .to_string()
            .contains("trailing payload"));
    }

    #[test]
    fn streaming_snapshot_checks_zip_crc_through_eof() {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file(
            "index",
            FileOptions::default().compression_method(CompressionMethod::Zstd),
        )
        .unwrap();
        zip.write_all(&empty_snapshot()).unwrap();
        let mut bytes = zip.finish().unwrap().into_inner();
        assert!(read_snapshot_archive(ZipArchive::new(Cursor::new(&bytes)).unwrap()).is_ok());
        let central = bytes
            .windows(4)
            .position(|window| window == b"PK\x01\x02")
            .unwrap();
        bytes[central + 16] ^= 1;
        assert!(read_snapshot_archive(ZipArchive::new(Cursor::new(&bytes)).unwrap()).is_err());
    }

    #[test]
    fn invalid_signature_tags_return_errors() {
        for bytes in [&[255_u8][..], &[1_u8, 255][..]] {
            assert!(IndexedSignatureType::read_from_buffer(bytes).is_err());
        }
    }

    #[test]
    fn snapshot_payload_rejects_missing_header() {
        let error = validate_snapshot_header(&[1, 2, 3]).unwrap_err();
        assert_eq!(
            "Unsupported JIndex snapshot: missing format header",
            error.to_string()
        );
    }

    #[test]
    fn snapshot_payload_rejects_previous_version() {
        let mut snapshot = SNAPSHOT_MAGIC.to_vec();
        snapshot.extend_from_slice(&(SNAPSHOT_VERSION - 1).to_le_bytes());

        let error = validate_snapshot_header(&snapshot).unwrap_err();
        assert_eq!(
            format!(
                "Unsupported JIndex snapshot version {}; expected {}",
                SNAPSHOT_VERSION - 1,
                SNAPSHOT_VERSION
            ),
            error.to_string()
        );
    }

    #[test]
    fn snapshot_payload_accepts_current_version() {
        let mut snapshot = SNAPSHOT_MAGIC.to_vec();
        snapshot.extend_from_slice(&SNAPSHOT_VERSION.to_le_bytes());
        snapshot.extend_from_slice(&[1, 2, 3]);

        validate_snapshot_header(&snapshot).unwrap();
    }
}
