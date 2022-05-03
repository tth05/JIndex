use crate::signature::generic_data_parser::parse_generic_signature_data;
use crate::signature::{
    MethodSignature, ParseError, ParseResultData, RawClassSignature, RawMethodSignature,
    RawSignatureType, RawTypeParameterData, SignaturePrimitive, SignatureType,
};
use ascii::{AsAsciiStr, AsciiChar, AsciiStr, IntoAsciiString};
use cafebabe::attributes::AttributeData;
use cafebabe::attributes::AttributeData::Signature;
use std::borrow::Cow;
use std::str::FromStr;

impl RawSignatureType {
    pub(super) fn parse_str(input: &str) -> Result<ParseResultData<RawSignatureType>, ParseError> {
        return RawSignatureType::parse_ascii(input.as_ascii_str()?);
    }

    pub(super) fn parse_ascii(
        input: &AsciiStr,
    ) -> Result<ParseResultData<RawSignatureType>, ParseError> {
        if let Some(first_char) = input.get_ascii(0) {
            let result = match first_char {
                AsciiChar::Z => (1, SignatureType::Primitive(SignaturePrimitive::Boolean)),
                AsciiChar::B => (1, SignatureType::Primitive(SignaturePrimitive::Byte)),
                AsciiChar::C => (1, SignatureType::Primitive(SignaturePrimitive::Char)),
                AsciiChar::D => (1, SignatureType::Primitive(SignaturePrimitive::Double)),
                AsciiChar::F => (1, SignatureType::Primitive(SignaturePrimitive::Float)),
                AsciiChar::I => (1, SignatureType::Primitive(SignaturePrimitive::Int)),
                AsciiChar::J => (1, SignatureType::Primitive(SignaturePrimitive::Long)),
                AsciiChar::S => (1, SignatureType::Primitive(SignaturePrimitive::Short)),
                AsciiChar::V => (1, SignatureType::Primitive(SignaturePrimitive::Void)),
                AsciiChar::L => {
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
                            RawSignatureType::ObjectInnerClass(Box::new(parts)),
                        )
                    }
                }
                AsciiChar::BracketOpen => {
                    let inner = SignatureType::parse_ascii(&input[1..])?;

                    (1 + inner.0, SignatureType::Array(Box::new(inner.1)))
                }
                AsciiChar::T => {
                    let semi_colon_index = input
                        .chars()
                        .position(|ch| ch == ';')
                        .ok_or(ParseError::Eof)?;
                    let sig = SignatureType::Generic(input[1..semi_colon_index].to_ascii_string());

                    (semi_colon_index as u16 + 1, sig)
                }
                AsciiChar::Minus => {
                    let inner = SignatureType::parse_ascii(&input[1..])?;
                    (1 + inner.0, SignatureType::ObjectMinus(Box::new(inner.1)))
                }
                AsciiChar::Plus => {
                    let inner = SignatureType::parse_ascii(&input[1..])?;
                    (1 + inner.0, SignatureType::ObjectPlus(Box::new(inner.1)))
                }
                _ => return Err(ParseError::UnexpectedChar(first_char.as_char())),
            };

            Ok(result)
        } else {
            Err(ParseError::Eof)
        }
    }

    fn parse_object(input: &AsciiStr) -> Result<ParseResultData<RawSignatureType>, ParseError> {
        //Find < or ;
        let mut special_char_index = input
            .chars()
            .position(|ch| ch == '<' || ch == ';' || ch == '.')
            .ok_or(ParseError::Eof)?;
        //Parse the first type, which we'll need either way
        let base_type = input[..special_char_index].to_ascii_string();

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
        input: &AsciiStr,
    ) -> Result<ParseResultData<Vec<Option<RawSignatureType>>>, ParseError> {
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
                let parse_result = SignatureType::parse_ascii(&input[index..])?;
                index += parse_result.0 as usize;
                Some(parse_result.1)
            });
        }

        //Consume '>' unchecked
        index += 1;

        Ok((index as u16, vec))
    }
}

impl FromStr for RawSignatureType {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(SignatureType::parse_str(s)?.1)
    }
}

impl ToString for RawSignatureType {
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
            _ => unreachable!(),
        }
    }
}

impl ToString for RawTypeParameterData {
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

impl RawClassSignature {
    pub fn new(
        super_class: Option<RawSignatureType>,
        interfaces: Option<Vec<RawSignatureType>>,
    ) -> Self {
        Self {
            generic_data: None,
            super_class,
            interfaces,
        }
    }
}

impl ToString for RawClassSignature {
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

impl FromStr for RawClassSignature {
    type Err = ParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let generic_data = parse_generic_signature_data(input).ok();
        let input = unsafe { input.as_ascii_str_unchecked() }; //parse_generic_signature_data validated the input for us

        let mut start_index = if let Some(ref result) = generic_data {
            result.0 as usize
        } else {
            0
        };

        let mut other_classes = {
            let mut parameters = Vec::new();
            while start_index < input.len() {
                let parse_result = SignatureType::parse_ascii(&input[start_index..])?;
                start_index += parse_result.0 as usize;
                parameters.push(parse_result.1);
            }

            parameters
        };

        Ok(RawClassSignature {
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

impl ToString for RawMethodSignature {
    fn to_string(&self) -> String {
        (if self.generic_data.is_some() {
            String::from('<') + &join_vec(self.generic_data()) + ">"
        } else {
            String::new()
        }) + "("
            + &join_vec(self.parameters())
            + ")"
            + &self.return_type.to_string()
            + &self
                .exceptions()
                .map(|v| {
                    v.iter()
                        .map(|e| String::from('^') + &e.to_string())
                        .collect::<Vec<_>>()
                        .join("")
                })
                .unwrap_or_default()
    }
}

impl RawMethodSignature {
    pub fn from_data<'a>(
        input: &'a str,
        exception_attribute_supplier: &dyn Fn() -> Option<&'a Vec<Cow<'a, str>>>,
    ) -> Result<Self, ParseError> {
        let generic_data = parse_generic_signature_data(input).ok();
        let input = unsafe { input.as_ascii_str_unchecked() }; //parse_generic_signature_data validated the input for us

        let mut start_index = if let Some(ref result) = generic_data {
            result.0 as usize
        } else {
            0
        };

        let parameters = if input.get_ascii(start_index).ok_or(ParseError::Eof)? == '(' {
            start_index += 1; //Skip '('

            let mut parameters = Vec::new();
            while input.get_ascii(start_index).ok_or(ParseError::Eof)? != ')' {
                let parse_result = SignatureType::parse_ascii(&input[start_index..])?;
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

        let return_type = SignatureType::parse_ascii(&input[start_index..])?;
        start_index += return_type.0 as usize;

        let mut exceptions = Vec::new();
        while input.get_ascii(start_index).map_or(false, |c| c == '^') {
            start_index += 1; //Skip '^'

            let parse_result = SignatureType::parse_ascii(&input[start_index..])?;
            start_index += parse_result.0 as usize;
            exceptions.push(parse_result.1);
        }

        if exceptions.is_empty() {
            if let Some(vec) = exception_attribute_supplier() {
                exceptions.extend(vec.iter().filter_map(|s| {
                    s.as_ascii_str()
                        .ok()
                        .map(|a| RawSignatureType::Object(a.to_ascii_string()))
                }));
            }
        }

        Ok(MethodSignature {
            generic_data: generic_data.map(|v| Box::new(v.1)),
            parameters: Some(parameters).filter(|v| !v.is_empty()).map(Box::new),
            return_type: return_type.1,
            exceptions: Some(exceptions).filter(|v| !v.is_empty()).map(Box::new),
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
