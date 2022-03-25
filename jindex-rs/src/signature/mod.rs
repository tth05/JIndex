mod generic_data_parser;
pub mod indexed_signature;
pub mod raw_signature;

use ascii::{AsAsciiStrError, AsciiString};
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
    /// Lsome/type<Lsome/type/bound;>; --- If any parameter is Option::None, that parameter is *
    ObjectTypeBounds(Box<(T, Vec<Option<SignatureType<T>>>)>),
    /// [L/some/type;
    Array(Box<SignatureType<T>>),
}

pub type RawSignatureType = SignatureType<AsciiString>;
pub type IndexedSignatureType = SignatureType<u32>;

/// Contains number of consumed characters and result object
type ParseResultData<T> = (u16, T);

/// Maps generic parameter names to their bound types. If the associated Option is None,
/// java/lang/Object should be implied as the only bound.
#[derive(Readable, Writable, Debug)]
pub struct TypeParameterData<T> {
    name: T,
    type_bound: Option<SignatureType<T>>,
    interface_bounds: Option<Vec<SignatureType<T>>>,
}

type RawTypeParameterData = TypeParameterData<AsciiString>;
type IndexedTypeParameterData = TypeParameterData<u32>;

#[derive(Readable, Writable, Debug)]
pub struct ClassSignature<T> {
    generic_data: Option<Vec<TypeParameterData<T>>>,
    /// The super class, or None if it is java/lang/Object
    super_class: Option<SignatureType<T>>,
    interfaces: Option<Vec<SignatureType<T>>>,
}

pub type RawClassSignature = ClassSignature<AsciiString>;
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

#[derive(Readable, Writable, Debug)]
pub struct MethodSignature<T> {
    generic_data: Option<Vec<TypeParameterData<T>>>,
    parameters: Vec<SignatureType<T>>,
    return_type: SignatureType<T>,
}

impl<T> MethodSignature<T> {
    pub fn generic_data(&self) -> Option<&Vec<TypeParameterData<T>>> {
        self.generic_data.as_ref()
    }

    pub fn parameters(&self) -> &Vec<SignatureType<T>> {
        &self.parameters
    }

    pub fn return_type(&self) -> &SignatureType<T> {
        &self.return_type
    }
}

pub type RawMethodSignature = MethodSignature<AsciiString>;
pub type IndexedMethodSignature = MethodSignature<u32>;

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
        let result = SignatureType::parse(input);
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
                let result = SignatureType::parse(&input);
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
            //Wildcard
            "Ljava/lang/Object<*Lother/type;**>;",
            //Inner classes
            "Lgnu/trove/map/custom_hash/TObjectByteCustomHashMap<TK;>.MapBackedView<TK;>.AnotherOne;",
        ];

        for input in data {
            let result = SignatureType::parse(input);
            assert!(result.is_ok());
            let result = result.unwrap();
            assert_eq!(result.0, input.len() as u16);
            assert_eq!(input, result.1.to_string());
        }
    }

    #[test]
    fn test_signature_type_parser_array() {
        let input = "[[Ljava/lang/Object;";
        let result = SignatureType::parse(input);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.0, 20);
        assert_eq!(input, result.1.to_string());
    }

    #[test]
    fn test_signature_type_parser_generic() {
        let input = "TB;";
        let result = SignatureType::parse(input);
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
            let result = SignatureType::parse(&input);
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
        let data = result.get(0).unwrap();
        assert_eq!("T", data.name);
        assert!(data.type_bound.is_some());
        assert_is_object_signature("java/lang/String", data.type_bound.as_ref().unwrap());

        assert!(data.interface_bounds.is_some());
        let interface_bounds = data.interface_bounds.as_ref().unwrap();
        assert_eq!(1, interface_bounds.len());
        assert_is_object_signature("java/lang/Comparable", interface_bounds.get(0).unwrap());

        //Check second parameter
        let data = result.get(1).unwrap();
        assert_eq!("B", data.name);
        assert!(data.type_bound.is_none());

        assert!(data.interface_bounds.is_some());
        let interface_bounds = data.interface_bounds.as_ref().unwrap();
        assert_eq!(1, interface_bounds.len());
        assert_is_object_signature("java/lang/Comparable", interface_bounds.get(0).unwrap());
    }

    #[test]
    fn test_parse_method_generic_signature() {
        let input = "<T:Ljava/lang/String;:Ljava/lang/Comparable;B::Ljava/lang/Comparable;>(-Ljava/util/List<Ljava/lang/String;>;)V";
        let result = MethodSignature::from_str(input);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(input, result.to_string());
    }

    #[test]
    fn test_parse_class_generic_signature() {
        let input = "<T:Ljava/lang/String;:Ljava/lang/Comparable;B::Ljava/lang/Comparable;>(-Ljava/util/List<Ljava/lang/String;>;)V";
        let result = MethodSignature::from_str(input);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(input, result.to_string());

        let input = "<INPUT:Lmekanism/common/recipe/inputs/MachineInput<*TINPUT;*>;OUTPUT:Lmekanism/common/recipe/outputs/MachineOutput<TOUTPUT;>;RECIPE:Lmekanism/common/recipe/machines/MachineRecipe<TINPUT;TOUTPUT;TRECIPE;>;>Lmekanism/common/integration/crafttweaker/util/RecipeMapModification<TINPUT;TRECIPE;>;";
        let result = ClassSignature::from_str(input);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(input, result.to_string());
    }

    fn assert_is_object_signature(str: &str, sig: &RawSignatureType) {
        assert!(matches!(sig, SignatureType::Object(_)));
        if let SignatureType::Object(s) = sig {
            assert_eq!(str, s);
        }
    }
}
