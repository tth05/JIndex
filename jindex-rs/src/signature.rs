use ascii::{AsAsciiStr, AsAsciiStrError, AsciiChar, AsciiString, IntoAsciiString, ToAsciiChar};
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;

pub type SignaturePrimitive = jni::signature::Primitive;

#[derive(Debug)]
pub enum SignatureType {
    /// I, J, L...
    Primitive(SignaturePrimitive),
    /// TPARAM_NAME;
    Generic(AsciiString),
    /// Lsome/type;
    Object(AsciiString),
    /// -Lsome/type;
    ObjectMinus(Box<SignatureType>),
    /// +Lsome/type;
    ObjectPlus(Box<SignatureType>),
    /// Lit/unimi/dsi/fastutil/ints/AbstractInt2ObjectSortedMap<TV;>.KeySet;
    ObjectInnerClass(Box<Vec<SignatureType>>),
    /// Lsome/type<Lsome/type/bound;>; --- If any parameter is Option::None, that parameter is *
    ObjectTypeBounds(Box<(AsciiString, Vec<Option<SignatureType>>)>),
    /// [L/some/type;
    Array(Box<SignatureType>),
}

/// Contains number of consumed characters and result object
type ParseResultData<T> = (u16, T);

impl SignatureType {
    fn parse(input: &str) -> Result<ParseResultData<SignatureType>, ParseError> {
        if let Some(first_char) = input.chars().next() {
            let result = match first_char {
                'Z' => (1, SignatureType::Primitive(SignaturePrimitive::Boolean)),
                'B' => (1, SignatureType::Primitive(SignaturePrimitive::Byte)),
                'C' => (1, SignatureType::Primitive(SignaturePrimitive::Char)),
                'D' => (1, SignatureType::Primitive(SignaturePrimitive::Double)),
                'F' => (1, SignatureType::Primitive(SignaturePrimitive::Float)),
                'I' => (1, SignatureType::Primitive(SignaturePrimitive::Int)),
                'J' => (1, SignatureType::Primitive(SignaturePrimitive::Long)),
                'S' => (1, SignatureType::Primitive(SignaturePrimitive::Short)),
                'V' => (1, SignatureType::Primitive(SignaturePrimitive::Void)),
                'L' => {
                    let object = SignatureType::parse_object(&input[1..])?;
                    let mut index = object.0 as usize;
                    if input.get_ascii(index).ok_or(ParseError::Eof)? == ';' {
                        (index as u16 + 1, object.1)
                    } else {
                        let mut parts = vec![object.1];
                        while input.get_ascii(index).ok_or(ParseError::Eof)? != ';' {
                            let data = SignatureType::parse_object(&input[(index + 1)..])?;
                            index += data.0 as usize;
                            parts.push(data.1);
                        }

                        (
                            index as u16 + 1,
                            SignatureType::ObjectInnerClass(Box::new(parts)),
                        )
                    }
                }
                '[' => {
                    let inner = SignatureType::parse(&input[1..])?;

                    (1 + inner.0, SignatureType::Array(Box::new(inner.1)))
                }
                'T' => {
                    let semi_colon_index = input.find(|c| c == ';').ok_or(ParseError::Eof)?;
                    let sig = SignatureType::Generic(
                        input[1..semi_colon_index].as_ascii_str()?.to_ascii_string(),
                    );

                    (semi_colon_index as u16 + 1, sig)
                }
                '-' => {
                    let inner = SignatureType::parse(&input[1..])?;
                    (1 + inner.0, SignatureType::ObjectMinus(Box::new(inner.1)))
                }
                '+' => {
                    let inner = SignatureType::parse(&input[1..])?;
                    (1 + inner.0, SignatureType::ObjectPlus(Box::new(inner.1)))
                }
                _ => return Err(ParseError::UnexpectedChar(first_char)),
            };

            Ok(result)
        } else {
            Err(ParseError::Eof)
        }
    }

    fn parse_object(input: &str) -> Result<ParseResultData<SignatureType>, ParseError> {
        //Find < or ;
        let mut special_char_index = input
            .find(|c| c == '<' || c == ';' || c == '.')
            .ok_or(ParseError::Eof)?;
        //Parse the first type, which we'll need either way
        let base_type = AsciiString::from(input[..special_char_index].as_ascii_str()?);

        //Parse the generic type bounds if there are any
        let sig = match SignatureType::parse_generic_type_bounds(&input[special_char_index..]) {
            Ok(data) => {
                special_char_index += data.0 as usize;
                SignatureType::ObjectTypeBounds(Box::new((base_type, data.1)))
            }
            Err(_) => SignatureType::Object(base_type),
        };

        Ok((special_char_index as u16 + 1, sig))
    }

    fn parse_generic_type_bounds(
        input: &str,
    ) -> Result<ParseResultData<Vec<Option<SignatureType>>>, ParseError> {
        let mut index = 0;
        let first_char = input.get_ascii(index).ok_or(ParseError::Eof)?;
        if first_char != '<' {
            return Err(ParseError::UnexpectedChar(first_char.as_char()));
        }

        let mut vec = Vec::with_capacity(1);

        //Consume '<'
        index += 1;
        while input.get_ascii(index).ok_or(ParseError::Eof)? != '>' {
            vec.push(if input.get_ascii(index).ok_or(ParseError::Eof)? == '*' {
                index += 1;
                None
            } else {
                let parse_result = SignatureType::parse(&input[index..])?;
                index += parse_result.0 as usize;
                Some(parse_result.1)
            });
        }

        //Consume '>' unchecked
        index += 1;

        Ok((index as u16, vec))
    }
}

impl FromStr for SignatureType {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(SignatureType::parse(s)?.1)
    }
}

impl ToString for SignatureType {
    fn to_string(&self) -> String {
        match &self {
            SignatureType::ObjectTypeBounds(inner) => {
                let (actual_type, type_bounds) = inner.as_ref();

                String::from('L')
                    + actual_type.as_ref()
                    + "<"
                    + &type_bounds
                        .iter()
                        .map(|t| {
                            t.as_ref()
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| String::from('*'))
                        })
                        .fold(String::new(), |a, b| a + &b)
                    + ">;"
            }
            SignatureType::ObjectInnerClass(inner) => {
                String::from('L')
                    + &inner
                        .as_ref()
                        .iter()
                        .map(|s| s.to_string())
                        .fold(String::new(), |a, b| {
                            let separator = if a.is_empty() { "" } else { "." };
                            a + (separator) + &b[1..b.len() - 1]
                        })
                    + ";"
            }
            SignatureType::Primitive(p) => p.to_string(),
            SignatureType::Object(name) => String::from('L') + name.as_ref() + ";",
            SignatureType::Generic(inner) => String::from('T') + inner.as_ref() + ";",
            SignatureType::ObjectMinus(inner) => String::from('-') + &inner.to_string(),
            SignatureType::ObjectPlus(inner) => String::from('+') + &inner.to_string(),
            SignatureType::Array(inner) => String::from('[') + &inner.to_string(),
        }
    }
}

/// Maps generic parameter names to their bound types. If the associated Option is None,
/// java/lang/Object should be implied as the only bound.
pub struct TypeParameterData {
    name: AsciiString,
    type_bound: Option<SignatureType>,
    interface_bounds: Option<Vec<SignatureType>>,
}

impl ToString for TypeParameterData {
    fn to_string(&self) -> String {
        self.name.to_string()
            + ":"
            + &self
                .type_bound
                .as_ref()
                .map_or(String::new(), |t| t.to_string())
            + &self
                .interface_bounds
                .as_ref()
                .unwrap_or(&Vec::new())
                .iter()
                .map(|i| i.to_string())
                .fold(String::new(), |a, b| a + ":" + &b)
    }
}

/// Parses stuff like '<T:Ljava/lang/Object;:Ljava/lang/Comparable;B>'
pub fn parse_generic_signature_data(
    input: &str,
) -> Result<ParseResultData<Vec<TypeParameterData>>, ParseError> {
    if let Some(first_char) = input.chars().next() {
        if first_char != '<' {
            return Err(ParseError::UnexpectedChar(first_char));
        }

        let mut parts = Vec::new();
        let mut current_index = 1;
        while current_index < input.len() && !input[current_index..].starts_with('>') {
            let part = parse_generic_signature_data_single(&input[current_index..])?;
            current_index += part.0 as usize;
            //TODO: Find new name if it already exists
            parts.push(part.1);
        }
        Ok((current_index as u16 + 1, parts))
    } else {
        Err(ParseError::Eof)
    }
}

fn parse_generic_signature_data_single(
    input: &str,
) -> Result<ParseResultData<TypeParameterData>, ParseError> {
    let mut separator_index = input.find(|c| c == ':').ok_or(ParseError::Eof)?;
    let name = input[..separator_index]
        .as_ascii_str()
        .unwrap()
        .to_ascii_string();

    let mut is_first = true;
    let mut type_bound = None;
    let mut interface_bounds = Vec::new();
    loop {
        if separator_index >= input.len() {
            return Err(ParseError::Eof);
        }

        if is_first && input[separator_index + 1..].starts_with(':') {
            separator_index += 1;
        } else {
            if !is_first && !input[separator_index..].starts_with(':') {
                break;
            }

            separator_index += 1;
            let t = SignatureType::parse(&input[separator_index..])?;
            separator_index += t.0 as usize;

            if is_first {
                //Exclude Object signatures because these are the default
                if let SignatureType::Object(str) = &t.1 {
                    if str.eq("java/lang/Object") {
                        is_first = false;
                        continue;
                    }
                }

                type_bound = Some(t.1);
            } else {
                interface_bounds.push(t.1);
            }
        }

        is_first = false;
    }

    Ok((
        separator_index as u16,
        TypeParameterData {
            name,
            type_bound,
            interface_bounds: Some(interface_bounds).filter(|v| !v.is_empty()),
        },
    ))
}

pub struct ClassSignature {
    generic_data: Option<Vec<TypeParameterData>>,
    /// The super class, or None if it is java/lang/Object
    super_class: Option<SignatureType>,
    interfaces: Option<Vec<SignatureType>>,
}

impl ClassSignature {
    pub fn new(super_class: Option<SignatureType>, interfaces: Option<Vec<SignatureType>>) -> Self {
        Self {
            generic_data: None,
            super_class,
            interfaces,
        }
    }
}

impl ToString for ClassSignature {
    fn to_string(&self) -> String {
        (if self.generic_data.is_some() {
            String::from('<') + &join_vec(self.generic_data.as_ref()) + ">"
        } else {
            String::new()
        }) + &self
            .super_class
            .as_ref()
            .map_or(String::new(), |s| s.to_string())
            + &join_vec(self.interfaces.as_ref())
    }
}

impl FromStr for ClassSignature {
    type Err = ParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let generic_data = parse_generic_signature_data(input).ok();
        let mut start_index = if let Some(ref result) = generic_data {
            result.0 as usize
        } else {
            0
        };

        let mut other_classes = {
            let mut parameters = Vec::new();
            while start_index < input.len() {
                let parse_result = SignatureType::parse(&input[start_index..])?;
                start_index += parse_result.0 as usize;
                parameters.push(parse_result.1);
            }

            parameters
        };

        Ok(ClassSignature {
            generic_data: generic_data.map(|v| v.1),
            super_class: Some(other_classes.remove(0)).filter(|s| {
                if let SignatureType::Object(name) = s {
                    name != "java/lang/Object"
                } else {
                    true
                }
            }),
            interfaces: Some(other_classes).filter(|v| !v.is_empty()),
        })
    }
}

pub struct MethodSignature {
    generic_data: Option<Vec<TypeParameterData>>,
    parameters: Vec<SignatureType>,
    return_type: SignatureType,
}

impl ToString for MethodSignature {
    fn to_string(&self) -> String {
        (if self.generic_data.is_some() {
            String::from('<') + &join_vec(self.generic_data.as_ref()) + ">"
        } else {
            String::new()
        }) + "("
            + &join_vec(Some(&self.parameters))
            + ")"
            + &self.return_type.to_string()
    }
}

impl FromStr for MethodSignature {
    type Err = ParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let generic_data = parse_generic_signature_data(input).ok();
        let mut start_index = if let Some(ref result) = generic_data {
            result.0 as usize
        } else {
            0
        };

        let parameters = if input.get_ascii(start_index).ok_or(ParseError::Eof)? == '(' {
            start_index += 1; //Skip '('

            let mut parameters = Vec::new();
            while input.get_ascii(start_index).ok_or(ParseError::Eof)? != ')' {
                let parse_result = SignatureType::parse(&input[start_index..])?;
                start_index += parse_result.0 as usize;
                parameters.push(parse_result.1);
            }

            start_index += 1; //Skip ')'

            parameters
        } else {
            return Err(ParseError::UnexpectedChar(
                input.get_ascii(start_index).unwrap().as_char(),
            ));
        };

        Ok(MethodSignature {
            generic_data: generic_data.map(|v| v.1),
            parameters,
            return_type: SignatureType::parse(&input[start_index..])?.1,
        })
    }
}

fn join_vec<T>(vec: Option<&Vec<T>>) -> String
where
    T: ToString,
{
    vec.unwrap_or(&Vec::new())
        .iter()
        .map(|t| t.to_string())
        .fold(String::new(), |a, b| a + &b)
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
    use crate::signature::{
        parse_generic_signature_data, ClassSignature, MethodSignature, SignatureType,
    };
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
        //Normal type bounds
        let input = "Ljava/lang/Object<+TC;Ltest;>;";
        let result = SignatureType::parse(input);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.0, input.len() as u16);
        assert_eq!(input, result.1.to_string());

        //Wildcard
        let input = "Ljava/lang/Object<*>;";
        let result = SignatureType::parse(input);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.0, input.len() as u16);
        assert_eq!(input, result.1.to_string());

        //Inner classes
        let input = "Lgnu/trove/map/custom_hash/TObjectByteCustomHashMap<TK;>.MapBackedView<TK;>.AnotherOne;";
        let result = SignatureType::parse(input);
        // assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.0, input.len() as u16);
        assert_eq!(input, result.1.to_string());
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

    fn assert_is_object_signature(str: &str, sig: &SignatureType) {
        assert!(matches!(sig, SignatureType::Object(_)));
        if let SignatureType::Object(s) = sig {
            assert_eq!(str, s);
        }
    }
}
