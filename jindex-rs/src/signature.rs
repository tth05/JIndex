use ascii::{AsAsciiStr, AsAsciiStrError, AsciiChar, AsciiString, ToAsciiChar};
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;

pub type SignaturePrimitive = jni::signature::Primitive;

pub enum SignatureType {
    /// I, J, L...
    Primitive(SignaturePrimitive),
    /// TPARAM_NAME;
    Generic(AsciiChar),
    /// Lsome/type;
    Object(AsciiString),
    /// -Lsome/type;
    ObjectMinus(Box<SignatureType>),
    /// +Lsome/type;
    ObjectPlus(Box<SignatureType>),
    /// Lsome/type<Lsome/type/bound;>;
    ObjectGeneric(Box<(AsciiString, Vec<SignatureType>)>),
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
                    //Find < or ;
                    let mut special_char_index = input
                        .find(|c| c == '<' || c == ';')
                        .ok_or(ParseError::Eof)?;
                    //Parse the first type, which we'll need either way
                    let base_type = AsciiString::from(input[1..special_char_index].as_ascii_str()?);

                    //Parse the generic type bounds if there are any
                    let sig = if input.get_ascii(special_char_index).unwrap() == '<' {
                        let mut vec = Vec::with_capacity(1);

                        //Consume '<'
                        special_char_index += 1;
                        while input.get_ascii(special_char_index).ok_or(ParseError::Eof)? != '>' {
                            let parse_result = SignatureType::parse(&input[special_char_index..])?;
                            special_char_index += parse_result.0 as usize;
                            vec.push(parse_result.1);
                        }

                        //Consume '>' unchecked
                        special_char_index += 1;

                        SignatureType::ObjectGeneric(Box::new((base_type, vec)))
                    } else {
                        SignatureType::Object(base_type)
                    };

                    (special_char_index as u16 + 1, sig)
                }
                '[' => {
                    let inner = SignatureType::parse(&input[1..])?;

                    (1 + inner.0, SignatureType::Array(Box::new(inner.1)))
                }
                'T' => {
                    let semi_colon_index = input.find(|c| c == ';').ok_or(ParseError::Eof)?;
                    let sig = SignatureType::Generic(
                        input[1..semi_colon_index]
                            .as_ascii_str()?
                            .first()
                            .ok_or(ParseError::Eof)?,
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
}

impl FromStr for SignatureType {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(SignatureType::parse(s)?.1)
    }
}

/// Maps generic parameter names to their bound types. If the associated Option is None,
/// java/lang/Object should be implied as the only bound.
pub struct TypeParameterData {
    name: AsciiChar,
    type_bound: Option<SignatureType>,
    interface_bounds: Option<Vec<SignatureType>>,
}

/// Parses stuff like '<T:Ljava/lang/Object;:Ljava/lang/Comparable;B>'
pub fn parse_generic_signature_data(
    input: &str,
) -> Result<ParseResultData<Vec<TypeParameterData>>, ParseError> {
    if let Some(first_char) = input.chars().next() {
        if first_char != '<' {
            return Err(ParseError::UnexpectedChar('<'));
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
            name: input
                .chars()
                .next()
                .ok_or(ParseError::Eof)?
                .to_ascii_char()
                .unwrap(),
            type_bound,
            interface_bounds: Some(interface_bounds).filter(|v| !v.is_empty()),
        },
    ))
}

pub struct MethodSignature {
    generic_data: Option<TypeParameterData>,
    parameters: Vec<SignatureType>,
    return_type: SignatureType,
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
    use crate::signature::{parse_generic_signature_data, SignatureType};
    use ascii::AsciiChar;

    #[test]
    fn test_signature_type_parser_object() {
        let result = SignatureType::parse("Ljava/lang/Object;");
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.0, 18);
        assert!(matches!(result.1, SignatureType::Object(_)));
        assert_is_object_signature("java/lang/Object", &result.1);
    }

    #[test]
    fn test_signature_type_parser_object_plus_minus() {
        //? extends Object
        let result = SignatureType::parse("+Ljava/lang/Object;");
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.0, 19);
        assert!(matches!(result.1, SignatureType::ObjectPlus(_)));
        if let SignatureType::ObjectPlus(inner) = result.1 {
            assert_is_object_signature("java/lang/Object", &inner);
        }

        //? super Object
        let result = SignatureType::parse("-Ljava/lang/Object;");
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.0, 19);
        assert!(matches!(result.1, SignatureType::ObjectMinus(_)));
        if let SignatureType::ObjectMinus(inner) = result.1 {
            assert_is_object_signature("java/lang/Object", &inner);
        }
    }

    #[test]
    fn test_signature_type_parser_array() {
        let result = SignatureType::parse("[[Ljava/lang/Object;");
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.0, 20);
        assert!(matches!(result.1, SignatureType::Array(_)));
        if let SignatureType::Array(inner) = result.1 {
            assert!(matches!(*inner, SignatureType::Array(_)));
            if let SignatureType::Array(object) = *inner {
                assert!(matches!(*object, SignatureType::Object(_)));
                assert_is_object_signature("java/lang/Object", &object);
            }
        }
    }

    #[test]
    fn test_signature_type_parser_generic() {
        let result = SignatureType::parse("TB;");
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.0, 3);
        assert!(matches!(result.1, SignatureType::Generic(_)));
        if let SignatureType::Generic(name) = result.1 {
            assert_eq!(AsciiChar::B, name);
        }
    }

    #[test]
    fn test_signature_type_parser_primitive() {
        let primitives = vec!['Z', 'B', 'C', 'D', 'F', 'I', 'J', 'S', 'V'];
        for p in primitives {
            let result = SignatureType::parse(&p.to_string());
            assert!(result.is_ok());
            let result = result.unwrap();
            assert_eq!(result.0, 1);
            assert!(matches!(result.1, SignatureType::Primitive(_)));
            if let SignatureType::Primitive(p_type) = result.1 {
                assert_eq!(p.to_string(), p_type.to_string());
            }
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
        assert_eq!('T', data.name);
        assert!(data.type_bound.is_some());
        assert_is_object_signature("java/lang/String", data.type_bound.as_ref().unwrap());

        assert!(data.interface_bounds.is_some());
        let interface_bounds = data.interface_bounds.as_ref().unwrap();
        assert_eq!(1, interface_bounds.len());
        assert_is_object_signature("java/lang/Comparable", interface_bounds.get(0).unwrap());

        //Check second parameter
        let data = result.get(1).unwrap();
        assert_eq!('B', data.name);
        assert!(data.type_bound.is_none());

        assert!(data.interface_bounds.is_some());
        let interface_bounds = data.interface_bounds.as_ref().unwrap();
        assert_eq!(1, interface_bounds.len());
        assert_is_object_signature("java/lang/Comparable", interface_bounds.get(0).unwrap());
    }

    fn assert_is_object_signature(str: &str, sig: &SignatureType) {
        assert!(matches!(sig, SignatureType::Object(_)));
        if let SignatureType::Object(s) = sig {
            assert_eq!(str, s);
        }
    }
}
