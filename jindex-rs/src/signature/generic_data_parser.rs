use crate::signature::{
    starts_with, ParseError, ParseResultData, RawTypeParameterData, SignatureType,
    TypeParameterData,
};
use ascii::{AsAsciiStr, AsciiChar, AsciiStr};
use compact_str::ToCompactString;

/// Parses stuff like '<T:Ljava/lang/Object;:Ljava/lang/Comparable;B>'
pub fn parse_generic_signature_data(
    input: &str,
) -> Result<ParseResultData<Vec<RawTypeParameterData>>, ParseError> {
    let input = input.as_ascii_str()?;
    if let Some(first_char) = input.get_ascii(0) {
        if first_char != '<' {
            return Err(ParseError::UnexpectedChar(first_char.as_char()));
        }

        let mut parts = Vec::new();
        let mut current_index = 1;
        while current_index < input.len()
            && !starts_with(&input[current_index..], AsciiChar::GreaterThan)
        {
            let part = parse_generic_signature_data_single(&input[current_index..])?;
            current_index += part.0 as usize;
            parts.push(part.1);
        }
        Ok((current_index as u16 + 1, parts))
    } else {
        Err(ParseError::Eof)
    }
}

fn parse_generic_signature_data_single(
    input: &AsciiStr,
) -> Result<ParseResultData<RawTypeParameterData>, ParseError> {
    let mut separator_index = input
        .chars()
        .position(|ch| ch == ':')
        .ok_or(ParseError::Eof)?;
    let name = input[..separator_index].as_str().to_compact_string();

    let mut is_first = true;
    let mut type_bound = None;
    let mut interface_bounds = Vec::new();
    loop {
        if separator_index >= input.len() {
            return Err(ParseError::Eof);
        }

        if is_first && starts_with(&input[separator_index + 1..], AsciiChar::Colon) {
            separator_index += 1;
        } else {
            if !is_first && !starts_with(&input[separator_index..], AsciiChar::Colon) {
                break;
            }

            separator_index += 1;
            let t = SignatureType::parse_ascii(&input[separator_index..])?;
            separator_index += t.0 as usize;

            if is_first {
                //Exclude Object signatures because these are the default
                if let SignatureType::Object(str) = &t.1 {
                    if &str[..] == unsafe { "java/lang/Object".as_ascii_str_unchecked() } {
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
