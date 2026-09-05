mod generic_data_parser;
pub mod indexed_signature;
pub mod raw_signature;

use ascii::{AsAsciiStrError, AsciiStr};
use compact_str::CompactString;
use speedy::{Readable, Writable};
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};

pub type SignaturePrimitive = jni::signature::Primitive;

#[derive(Debug)]
pub enum SignatureType<T> {
    Unresolved,
    /// I, J, L...
    Primitive(SignaturePrimitive),
    /// TPARAM_NAME;
    Generic(T),
    /// Lsome/type;
    Object(T),
    /// -Lsome/type;
    ObjectMinus(Box<SignatureType<T>>),
    /// +Lsome/type;
    ObjectPlus(Box<SignatureType<T>>),
    /// Lit/unimi/dsi/fastutil/ints/AbstractInt2ObjectSortedMap<TV;>.KeySet;
    ObjectInnerClass(Box<Vec<SignatureType<T>>>),
    /// Lsome/type<Lsome/type/bound;>; --- If any parameter is Option::None,
    /// that parameter is *
    ObjectTypeBounds(Box<(T, Vec<Option<SignatureType<T>>>)>),
    /// [L/some/type;
    Array(Box<SignatureType<T>>),
}

pub type RawSignatureType = SignatureType<CompactString>;
pub type IndexedSignatureType = SignatureType<u32>;

/// Contains number of consumed characters and result object
type ParseResultData<T> = (u16, T);

/// Maps generic parameter names to their bound types. If the associated Option
/// is None, java/lang/Object should be implied as the only bound.
#[derive(Readable, Writable, Debug)]
pub struct TypeParameterData<T> {
    name: T,
    type_bound: Option<SignatureType<T>>,
    interface_bounds: Option<Vec<SignatureType<T>>>,
}

type RawTypeParameterData = TypeParameterData<CompactString>;
pub type IndexedTypeParameterData = TypeParameterData<u32>;

#[derive(Readable, Writable, Debug)]
pub struct ClassSignature<T> {
    generic_data: Option<Vec<TypeParameterData<T>>>,
    /// The super class, or None if it is java/lang/Object
    super_class: Option<SignatureType<T>>,
    interfaces: Option<Vec<SignatureType<T>>>,
}

pub type RawClassSignature = ClassSignature<CompactString>;
pub type IndexedClassSignature = ClassSignature<u32>;

impl<T> ClassSignature<T> {
    pub fn generic_data(&self) -> Option<&Vec<TypeParameterData<T>>> {
        self.generic_data.as_ref()
    }

    pub fn super_class(&self) -> Option<&SignatureType<T>> {
        self.super_class.as_ref()
    }

    pub fn interfaces(&self) -> Option<&Vec<SignatureType<T>>> {
        self.interfaces.as_ref()
    }
}

#[derive(Debug)]
/// Some fields here are in an extra Box because they blow up the size of the
/// struct otherwise. This makes sense because a lot of these are created and
/// without the Boxes the size of this struct would be doubled, even though
/// generic data, exceptions and parameters are used for less than half of all
/// methods
// The optional boxed Vec is one pointer; an optional boxed slice is two.
// Keep sparse per-method storage small and measure changes with the corpus.
#[allow(clippy::box_collection)]
pub struct MethodSignature<T> {
    generic_data: Option<Box<Vec<TypeParameterData<T>>>>,
    parameters: Option<Box<Vec<SignatureType<T>>>>,
    return_type: SignatureType<T>,
    exceptions: Option<Box<Vec<SignatureType<T>>>>,
}

impl<T> MethodSignature<T> {
    pub fn generic_data(&self) -> Option<&Vec<TypeParameterData<T>>> {
        self.generic_data.as_ref().map(|b| b.as_ref())
    }

    pub fn parameters(&self) -> Option<&Vec<SignatureType<T>>> {
        self.parameters.as_ref().map(|b| b.as_ref())
    }

    pub fn parameter_count(&self) -> usize {
        self.parameters.as_ref().map(|b| b.len()).unwrap_or(0)
    }

    pub fn return_type(&self) -> &SignatureType<T> {
        &self.return_type
    }

    pub fn exceptions(&self) -> Option<&Vec<SignatureType<T>>> {
        self.exceptions.as_ref().map(|b| b.as_ref())
    }
}

pub type RawMethodSignature = MethodSignature<CompactString>;
pub type IndexedMethodSignature = MethodSignature<u32>;

impl SignatureType<u32> {
    pub(crate) fn validate_snapshot(
        &self,
        entries: &rustc_hash::FxHashSet<u32>,
        classes: usize,
    ) -> anyhow::Result<()> {
        use anyhow::ensure;
        match self {
            Self::Unresolved | Self::Primitive(_) => {}
            Self::Generic(name) => ensure!(entries.contains(name), "Invalid generic name offset"),
            Self::Object(class) => {
                ensure!((*class as usize) < classes, "Invalid signature class index")
            }
            Self::ObjectPlus(inner) | Self::ObjectMinus(inner) | Self::Array(inner) => {
                inner.validate_snapshot(entries, classes)?
            }
            Self::ObjectInnerClass(parts) => {
                ensure!(!parts.is_empty(), "Empty inner-class signature");
                for part in parts.iter() {
                    part.validate_snapshot(entries, classes)?;
                }
            }
            Self::ObjectTypeBounds(bounds) => {
                ensure!(
                    (bounds.0 as usize) < classes,
                    "Invalid bounded signature class index"
                );
                for bound in bounds.1.iter().flatten() {
                    bound.validate_snapshot(entries, classes)?;
                }
            }
        }
        Ok(())
    }
}

impl TypeParameterData<u32> {
    fn validate_snapshot(
        &self,
        entries: &rustc_hash::FxHashSet<u32>,
        classes: usize,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            entries.contains(&self.name),
            "Invalid type parameter name offset"
        );
        for bound in self
            .type_bound
            .iter()
            .chain(self.interface_bounds.iter().flatten())
        {
            bound.validate_snapshot(entries, classes)?;
        }
        Ok(())
    }
}

impl ClassSignature<u32> {
    pub(crate) fn validate_snapshot(
        &self,
        entries: &rustc_hash::FxHashSet<u32>,
        classes: usize,
    ) -> anyhow::Result<()> {
        for parameter in self.generic_data.iter().flatten() {
            parameter.validate_snapshot(entries, classes)?;
        }
        for signature in self
            .super_class
            .iter()
            .chain(self.interfaces.iter().flatten())
        {
            signature.validate_snapshot(entries, classes)?;
        }
        Ok(())
    }
}

impl MethodSignature<u32> {
    pub(crate) fn validate_snapshot(
        &self,
        entries: &rustc_hash::FxHashSet<u32>,
        classes: usize,
    ) -> anyhow::Result<()> {
        for parameter in self.generic_data().into_iter().flatten() {
            parameter.validate_snapshot(entries, classes)?;
        }
        for signature in self
            .parameters()
            .into_iter()
            .flatten()
            .chain(self.exceptions().into_iter().flatten())
            .chain(std::iter::once(&self.return_type))
        {
            signature.validate_snapshot(entries, classes)?;
        }
        Ok(())
    }
}

impl EnclosingTypeInfo<u32> {
    pub(crate) fn validate_snapshot(
        &self,
        entries: &rustc_hash::FxHashSet<u32>,
        classes: usize,
    ) -> anyhow::Result<()> {
        if let Some(class) = self.class_name {
            anyhow::ensure!((class as usize) < classes, "Invalid enclosing class index");
        }
        if let Some(name) = self.method_name {
            anyhow::ensure!(
                entries.contains(&name),
                "Invalid enclosing method name offset"
            );
        }
        if let Some(method) = &self.method_descriptor {
            method.validate_snapshot(entries, classes)?;
        }
        Ok(())
    }
}

#[derive(Readable, Writable, Eq, PartialEq, Clone, Copy, Debug)]
pub enum InnerClassType {
    Member,
    Anonymous,
    Local,
}

impl InnerClassType {
    pub fn as_index(&self) -> u8 {
        match *self {
            InnerClassType::Member => 0,
            InnerClassType::Anonymous => 1,
            InnerClassType::Local => 2,
        }
    }
}

#[derive(Debug)]
pub struct EnclosingTypeInfo<T> {
    class_name: Option<T>,
    inner_class_type: InnerClassType,
    method_name: Option<T>,
    //This is in a box because it's rarely used but would increase the size of this struct by 56
    // bytes
    method_descriptor: Option<Box<MethodSignature<T>>>,
}

impl<T> EnclosingTypeInfo<T> {
    pub fn new(
        //This is in an Option because when indexed, it might end up as unresolved
        class_name: Option<T>,
        inner_class_type: InnerClassType,
        method_name: Option<T>,
        method_descriptor: Option<MethodSignature<T>>,
    ) -> Self {
        EnclosingTypeInfo {
            class_name,
            inner_class_type,
            method_name,
            method_descriptor: method_descriptor.map(Box::new),
        }
    }

    pub fn class_name(&self) -> Option<&T> {
        self.class_name.as_ref()
    }

    pub fn inner_class_type(&self) -> &InnerClassType {
        &self.inner_class_type
    }

    pub fn method_name(&self) -> Option<&T> {
        self.method_name.as_ref()
    }
    pub fn method_descriptor(&self) -> Option<&MethodSignature<T>> {
        self.method_descriptor.as_ref().map(|b| b.as_ref())
    }
}

pub type RawEnclosingTypeInfo = EnclosingTypeInfo<CompactString>;
pub type IndexedEnclosingTypeInfo = EnclosingTypeInfo<u32>;

fn starts_with<T>(str: &AsciiStr, prefix: T) -> bool
where
    T: AsRef<AsciiStr>,
{
    str.as_bytes().starts_with(prefix.as_ref().as_bytes())
}

pub enum ParseError {
    Eof,
    AsciiStringError(AsAsciiStrError),
    UnexpectedChar(char),
}

impl From<AsAsciiStrError> for ParseError {
    fn from(e: AsAsciiStrError) -> Self {
        ParseError::AsciiStringError(e)
    }
}

impl Debug for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Error for ParseError {}

impl ParseError {
    fn as_str(&self) -> String {
        use self::ParseError::*;
        match self {
            UnexpectedChar(c) => format!("Unexpected char '{}'", c),
            AsciiStringError(e) => e.to_string(),
            Eof => "End of input reached unexpectedly".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::generic_data_parser::parse_generic_signature_data;
    use crate::signature::{ClassSignature, MethodSignature, RawSignatureType, SignatureType};
    use std::str::FromStr;

    #[test]
    fn test_signature_type_parser_object() {
        let input = "Ljava/lang/Object;";
        let result = SignatureType::parse_str(input);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.0, 18);
        assert_eq!(input, result.1.to_string());
    }

    #[test]
    fn test_signature_type_parser_object_plus_minus() {
        macro_rules! test_object_prefix {
            ($prefix: literal, $type: ident) => {
                let input = $prefix.to_owned() + "Ljava/lang/Object;";
                let result = SignatureType::parse_str(&input);
                assert!(result.is_ok());
                let result = result.unwrap();
                assert_eq!(result.0, 19);
                assert_eq!(input, result.1.to_string());
            };
        }

        //? extends Object
        test_object_prefix!("+", ObjectPlus);
        //? super Object
        test_object_prefix!("-", ObjectMinus);
    }

    #[test]
    fn test_signature_type_parser_object_with_type_bounds() {
        let data = vec![
            //Normal type bounds
            "Ljava/lang/Object<+TC;Ltest;>;",
            //Double object bound
            "Lnet/minecraft/util/registry/RegistryNamespaced<Lnet/minecraft/util/ResourceLocation;Lnet/minecraft/item/Item;>;",
            //Wildcard
            "Ljava/lang/Object<*Lother/type;**>;",
            //Inner classes
            "Lgnu/trove/map/custom_hash/TObjectByteCustomHashMap<TK;>.MapBackedView<TK;>.AnotherOne;",
        ];

        for input in data {
            let result = SignatureType::parse_str(input);
            assert!(result.is_ok());
            let result = result.unwrap();
            assert_eq!(result.0, input.len() as u16);
            assert_eq!(input, result.1.to_string());
        }
    }

    #[test]
    fn test_signature_type_parser_array() {
        let input = "[[Ljava/lang/Object;";
        let result = SignatureType::parse_str(input);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.0, 20);
        assert_eq!(input, result.1.to_string());
    }

    #[test]
    fn test_signature_type_parser_generic() {
        let input = "TB;";
        let result = SignatureType::parse_str(input);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.0, 3);
        assert_eq!(input, result.1.to_string());
    }

    #[test]
    fn test_signature_type_parser_primitive() {
        let primitives = vec!['Z', 'B', 'C', 'D', 'F', 'I', 'J', 'S', 'V'];
        for p in primitives {
            let input = p.to_string();
            let result = SignatureType::parse_str(&input);
            assert!(result.is_ok());
            let result = result.unwrap();
            assert_eq!(result.0, 1);
            assert_eq!(input, result.1.to_string());
        }
    }

    #[test]
    fn test_generic_signature_parser() {
        let str = "<T:Ljava/lang/String;:Ljava/lang/Comparable;B::Ljava/lang/Comparable;>";
        let result = parse_generic_signature_data(str);

        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(str.len(), result.0 as usize);
        let result = result.1;
        assert_eq!(2, result.len());

        //Check first parameter
        let data = result.first().unwrap();
        assert_eq!("T", data.name);
        assert!(data.type_bound.is_some());
        assert_is_object_signature("java/lang/String", data.type_bound.as_ref().unwrap());

        assert!(data.interface_bounds.is_some());
        let interface_bounds = data.interface_bounds.as_ref().unwrap();
        assert_eq!(1, interface_bounds.len());
        assert_is_object_signature("java/lang/Comparable", interface_bounds.first().unwrap());

        //Check second parameter
        let data = result.get(1).unwrap();
        assert_eq!("B", data.name);
        assert!(data.type_bound.is_none());

        assert!(data.interface_bounds.is_some());
        let interface_bounds = data.interface_bounds.as_ref().unwrap();
        assert_eq!(1, interface_bounds.len());
        assert_is_object_signature("java/lang/Comparable", interface_bounds.first().unwrap());
    }

    #[test]
    fn test_parse_method_generic_signature() {
        let input = "<T:Ljava/lang/String;:Ljava/lang/Comparable;B::Ljava/lang/Comparable;>(-Ljava/util/List<Ljava/lang/String;>;)V";
        let result = MethodSignature::from_data(input, &|| Option::None);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(input, result.to_string());
    }

    #[test]
    fn test_parse_method_generic_signature_with_exceptions() {
        let input = "<T:Ljava/lang/String;:Ljava/lang/Comparable;B::Ljava/lang/Comparable;>(-Ljava/util/List<Ljava/lang/String;>;)V^Ljava/lang/Exception;^Ljava/lang/RuntimeException;^TB;";
        let result = MethodSignature::from_data(input, &|| Option::None);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(input, result.to_string());
    }

    #[test]
    fn test_parse_class_generic_signature() {
        let input = "<T:Ljava/lang/String;:Ljava/lang/Comparable;B::Ljava/lang/Comparable;>(-Ljava/util/List<Ljava/lang/String;>;)V";
        let result = MethodSignature::from_data(input, &|| Option::None);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(input, result.to_string());

        let input = "<INPUT:Lmekanism/common/recipe/inputs/MachineInput<*TINPUT;*>;OUTPUT:Lmekanism/common/recipe/outputs/MachineOutput<TOUTPUT;>;RECIPE:Lmekanism/common/recipe/machines/MachineRecipe<TINPUT;TOUTPUT;TRECIPE;>;>Lmekanism/common/integration/crafttweaker/util/RecipeMapModification<TINPUT;TRECIPE;>;";
        let result = ClassSignature::from_str(input);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(input, result.to_string());
    }

    #[test]
    fn test_parse_inner_class_generic_with_dollar() {
        let input = "Lscala/collection/parallel/mutable/ParArray<TT;>.ParArrayIterator$;";
        let result = RawSignatureType::parse_str(input);
        assert!(result.is_ok());
        let result = result.unwrap().1;
        assert_eq!(input, result.to_string());
        println!(
            "{:?}",
            match result {
                RawSignatureType::ObjectInnerClass(parts) => {
                    assert_eq!(2, parts.len());
                    assert_eq!("LParArrayIterator$;", parts.get(1).unwrap().to_string());
                }
                _ => panic!(""),
            }
        );
    }

    fn assert_is_object_signature(str: &str, sig: &RawSignatureType) {
        assert!(matches!(sig, SignatureType::Object(_)));
        if let SignatureType::Object(s) = sig {
            assert_eq!(str, s.as_str());
        }
    }
}
