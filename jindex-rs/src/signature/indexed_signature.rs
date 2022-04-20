use crate::class_index::{rsplit_once, ClassIndex, ClassIndexBuilder};
use crate::signature::{
    IndexedClassSignature, IndexedEnclosingTypeInfo, IndexedMethodSignature, IndexedSignatureType,
    IndexedTypeParameterData, RawClassSignature, RawEnclosingTypeInfo, RawMethodSignature,
    RawSignatureType, RawTypeParameterData, SignatureType,
};
use ascii::{AsAsciiStr, AsciiChar, AsciiStr, AsciiString};
use std::collections::HashMap;

pub trait ToIndexedType {
    type Out;

    fn to_indexed_type<'a>(
        &'a self,
        class_index: &ClassIndex,
        constant_pool_map: &mut HashMap<&'a AsciiStr, u32>,
    ) -> Self::Out;
}

pub trait ToSignatureIndexedType {
    fn to_signature_string(&self, class_index: &ClassIndex) -> String;
}

pub trait ToDescriptorIndexedType {
    fn to_descriptor_string(
        &self,
        class_index: &ClassIndex,
        generic_data: &[&IndexedTypeParameterData],
    ) -> String;
}

impl<T> SignatureType<T> {
    pub fn extract_base_object_type(&self) -> &T {
        match &self {
            SignatureType::Object(t) => t,
            SignatureType::ObjectPlus(t) => t.extract_base_object_type(),
            SignatureType::ObjectMinus(t) => t.extract_base_object_type(),
            SignatureType::ObjectTypeBounds(t) => &t.as_ref().0,
            SignatureType::ObjectInnerClass(parts) => {
                parts.last().unwrap().extract_base_object_type()
            }
            _ => panic!("Not an object type"),
        }
    }
}

impl ToIndexedType for RawSignatureType {
    type Out = IndexedSignatureType;

    fn to_indexed_type<'a>(
        &'a self,
        class_index: &ClassIndex,
        constant_pool_map: &mut HashMap<&'a AsciiStr, u32>,
    ) -> Self::Out {
        match &self {
            RawSignatureType::Primitive(p) => IndexedSignatureType::Primitive(*p),
            RawSignatureType::Generic(name) => {
                IndexedSignatureType::Generic(ClassIndexBuilder::get_index_from_pool(
                    name,
                    constant_pool_map,
                    &mut class_index.constant_pool_mut(),
                ))
            }
            RawSignatureType::Array(inner) => IndexedSignatureType::Array(Box::new(
                inner.to_indexed_type(class_index, constant_pool_map),
            )),
            RawSignatureType::Object(name) => index_object_type(name, class_index),
            RawSignatureType::ObjectPlus(inner) => IndexedSignatureType::ObjectPlus(Box::new(
                inner.to_indexed_type(class_index, constant_pool_map),
            )),
            RawSignatureType::ObjectMinus(inner) => IndexedSignatureType::ObjectMinus(Box::new(
                inner.to_indexed_type(class_index, constant_pool_map),
            )),
            RawSignatureType::ObjectInnerClass(inner) => {
                let inner = inner.as_ref();
                let base_type_signature = inner.first().unwrap();
                let mut type_name = base_type_signature.extract_base_object_type().clone();

                let mut new_vec = Vec::with_capacity(inner.len());
                //Add base type
                new_vec.push(base_type_signature.to_indexed_type(class_index, constant_pool_map));
                //Add inner classes
                inner.iter().skip(1).for_each(|s| {
                    //Separator
                    type_name.push_str(unsafe { "$".as_ascii_str_unchecked() });
                    new_vec.push(match s {
                        RawSignatureType::Object(name) => {
                            //Add inner class name
                            type_name.push_str(name);

                            let index_or_none = index_for_object_type(&type_name, class_index);

                            match index_or_none {
                                Some(i) => IndexedSignatureType::Object(i),
                                _ => IndexedSignatureType::Unresolved,
                            }
                        }
                        RawSignatureType::ObjectTypeBounds(inner) => {
                            let (main_type, vec) = inner.as_ref();
                            //Add inner class name
                            type_name.push_str(main_type);

                            let main_type_index_or_none =
                                index_for_object_type(&type_name, class_index);

                            match main_type_index_or_none {
                                Some(main_type_index) => {
                                    IndexedSignatureType::ObjectTypeBounds(Box::new((
                                        main_type_index,
                                        vec.to_indexed_type(class_index, constant_pool_map),
                                    )))
                                }
                                _ => IndexedSignatureType::Unresolved,
                            }
                        }
                        _ => unreachable!(),
                    })
                });

                IndexedSignatureType::ObjectInnerClass(Box::new(new_vec))
            }
            RawSignatureType::ObjectTypeBounds(inner) => {
                let (main_type, vec) = inner.as_ref();
                let main_type_index_or_none = index_for_object_type(main_type, class_index);

                let mut indexed_vec = vec.to_indexed_type(class_index, constant_pool_map);
                indexed_vec.shrink_to_fit();
                match main_type_index_or_none {
                    Some(main_type_index) => IndexedSignatureType::ObjectTypeBounds(Box::new((
                        main_type_index,
                        indexed_vec,
                    ))),
                    _ => IndexedSignatureType::Unresolved,
                }
            }
            _ => unreachable!(),
        }
    }
}

impl ToSignatureIndexedType for IndexedSignatureType {
    fn to_signature_string(&self, class_index: &ClassIndex) -> String {
        match &self {
            SignatureType::ObjectTypeBounds(inner) => {
                let (actual_type, type_bounds) = inner.as_ref();

                String::from('L')
                    + get_full_class_name(class_index, actual_type).as_str()
                    + "<"
                    + &type_bounds
                        .iter()
                        .map(|t| {
                            t.as_ref()
                                .map(|v| v.to_signature_string(class_index))
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
                        .map(|s| s.to_signature_string(class_index))
                        .fold(String::new(), |a, b| {
                            let is_first = a.is_empty();
                            let separator = if is_first { "" } else { "." };

                            //Removes the 'L' and ';'
                            let b = &b[1..b.len() - 1];
                            let class_name_start_index = (match b.find(|c| c == '<') {
                                Some(end) => &b[..end], //Removes the type parameters
                                None => b,
                            })
                            //We check for is_first here to exit rfind instantly
                            .rfind(|c| is_first || c == '$' || c == '/') //Remove the package name and parent class name
                            .map_or(0, |u| match is_first {
                                false => u + 1,
                                _ => 0, //The first element needs to keep the package name
                            });

                            a + (separator) + &b[class_name_start_index..]
                        })
                    + ";"
            }
            SignatureType::Primitive(p) => p.to_string(),
            SignatureType::Object(index) => {
                String::from('L') + get_full_class_name(class_index, index).as_str() + ";"
            }
            SignatureType::Generic(index) => {
                String::from('T')
                    + class_index
                        .constant_pool()
                        .string_view_at(*index)
                        .into_ascii_string(&class_index.constant_pool())
                        .as_str()
                    + ";"
            }
            SignatureType::ObjectMinus(inner) => {
                String::from('-') + &inner.to_signature_string(class_index)
            }
            SignatureType::ObjectPlus(inner) => {
                String::from('+') + &inner.to_signature_string(class_index)
            }
            SignatureType::Array(inner) => {
                String::from('[') + &inner.to_signature_string(class_index)
            }
            SignatureType::Unresolved => String::from("!unresolved!"),
        }
    }
}

impl ToDescriptorIndexedType for IndexedSignatureType {
    fn to_descriptor_string(
        &self,
        class_index: &ClassIndex,
        generic_data: &[&IndexedTypeParameterData],
    ) -> String {
        match &self {
            SignatureType::Object(_)
            | SignatureType::ObjectPlus(_)
            | SignatureType::ObjectMinus(_)
            | SignatureType::ObjectTypeBounds(_)
            | SignatureType::ObjectInnerClass(_) => {
                String::from('L')
                    + class_index
                        .class_at_index(*self.extract_base_object_type())
                        .class_name_with_package(&class_index.constant_pool())
                        .as_str()
                    + ";"
            }
            SignatureType::Generic(name) => {
                let generic_param_name = class_index.constant_pool().string_view_at(*name);
                match generic_data
                    .iter()
                    .find(|g| {
                        class_index.constant_pool().string_view_at(g.name) == generic_param_name
                    })
                    .filter(|p| p.type_bound.is_some())
                {
                    Some(param) => param
                        .type_bound
                        .as_ref()
                        .unwrap()
                        .to_descriptor_string(class_index, generic_data),
                    None => "Ljava/lang/Object;".to_owned(), //If there's no type bound or the parameter was not found
                }
            }
            SignatureType::Array(inner) => {
                String::from('[') + &inner.to_descriptor_string(class_index, generic_data)
            }
            SignatureType::Primitive(p) => p.to_string(),
            SignatureType::Unresolved => String::from("!unresolved!"),
        }
    }
}

fn get_full_class_name(class_index: &ClassIndex, index: &u32) -> AsciiString {
    class_index
        .class_at_index(*index)
        .class_name_with_package(&class_index.constant_pool())
}

impl IndexedClassSignature {
    pub fn new(
        generic_data: Option<Vec<IndexedTypeParameterData>>,
        super_class: Option<IndexedSignatureType>,
        interfaces: Option<Vec<IndexedSignatureType>>,
    ) -> Self {
        Self {
            generic_data: generic_data.map(|mut v| {
                v.shrink_to_fit();
                v
            }),
            super_class,
            interfaces: interfaces.map(|mut v| {
                v.shrink_to_fit();
                v
            }),
        }
    }
}

impl ToIndexedType for RawClassSignature {
    type Out = IndexedClassSignature;

    fn to_indexed_type<'a>(
        &'a self,
        class_index: &ClassIndex,
        constant_pool_map: &mut HashMap<&'a AsciiStr, u32>,
    ) -> Self::Out {
        IndexedClassSignature::new(
            self.generic_data
                .to_indexed_type(class_index, constant_pool_map),
            self.super_class
                .to_indexed_type(class_index, constant_pool_map),
            self.interfaces
                .to_indexed_type(class_index, constant_pool_map),
        )
    }
}

impl ToSignatureIndexedType for IndexedClassSignature {
    fn to_signature_string(&self, class_index: &ClassIndex) -> String {
        (if self.generic_data.is_some() {
            String::from('<') + &self.generic_data.to_signature_string(class_index) + ">"
        } else {
            String::new()
        }) + &(match &self.super_class {
            Some(inner) => inner.to_signature_string(class_index),
            None => String::from("Ljava/lang/Object;"),
        }) + &self.interfaces.to_signature_string(class_index)
    }
}

impl IndexedTypeParameterData {
    fn new(
        name_index: u32,
        type_bound: Option<IndexedSignatureType>,
        interface_bounds: Option<Vec<IndexedSignatureType>>,
    ) -> Self {
        Self {
            name: name_index,
            type_bound,
            interface_bounds,
        }
    }
}

impl ToIndexedType for RawTypeParameterData {
    type Out = IndexedTypeParameterData;

    fn to_indexed_type<'a>(
        &'a self,
        class_index: &ClassIndex,
        constant_pool_map: &mut HashMap<&'a AsciiStr, u32>,
    ) -> Self::Out {
        //Force compiler to drop &mut to constant pool
        let index = {
            ClassIndexBuilder::get_index_from_pool(
                &self.name,
                constant_pool_map,
                &mut class_index.constant_pool_mut(),
            )
        };

        IndexedTypeParameterData::new(
            index,
            self.type_bound
                .to_indexed_type(class_index, constant_pool_map),
            self.interface_bounds
                .to_indexed_type(class_index, constant_pool_map),
        )
    }
}

impl ToSignatureIndexedType for IndexedTypeParameterData {
    fn to_signature_string(&self, class_index: &ClassIndex) -> String {
        class_index
            .constant_pool()
            .string_view_at(self.name)
            .into_ascii_string(&class_index.constant_pool())
            .to_string()
            + ":"
            + &(match &self.type_bound {
                Some(inner) => inner.to_signature_string(class_index),
                None => String::from("Ljava/lang/Object;"),
            })
            + &self
                .interface_bounds
                .as_ref()
                .unwrap_or(&Vec::new())
                .iter()
                .map(|i| i.to_signature_string(class_index))
                .fold(String::new(), |a, b| a + ":" + &b)
    }
}

impl IndexedMethodSignature {
    pub fn new(
        generic_data: Option<Vec<IndexedTypeParameterData>>,
        mut parameters: Vec<IndexedSignatureType>,
        return_type: IndexedSignatureType,
    ) -> Self {
        parameters.shrink_to_fit();
        Self {
            generic_data: generic_data.map(|mut v| {
                v.shrink_to_fit();
                v
            }),
            parameters,
            return_type,
        }
    }
}

impl ToIndexedType for RawMethodSignature {
    type Out = IndexedMethodSignature;

    fn to_indexed_type<'a>(
        &'a self,
        class_index: &ClassIndex,
        constant_pool_map: &mut HashMap<&'a AsciiStr, u32>,
    ) -> Self::Out {
        IndexedMethodSignature::new(
            self.generic_data
                .to_indexed_type(class_index, constant_pool_map),
            self.parameters
                .to_indexed_type(class_index, constant_pool_map),
            self.return_type
                .to_indexed_type(class_index, constant_pool_map),
        )
    }
}

impl ToSignatureIndexedType for IndexedMethodSignature {
    fn to_signature_string(&self, class_index: &ClassIndex) -> String {
        (if self.generic_data.is_some() {
            String::from('<') + &self.generic_data.to_signature_string(class_index) + ">"
        } else {
            String::new()
        }) + "("
            + &self.parameters.to_signature_string(class_index)
            + ")"
            + &self.return_type.to_signature_string(class_index)
    }
}

impl ToDescriptorIndexedType for IndexedMethodSignature {
    fn to_descriptor_string(
        &self,
        class_index: &ClassIndex,
        generic_data: &[&IndexedTypeParameterData],
    ) -> String {
        String::from('(')
            + &self
                .parameters
                .to_descriptor_string(class_index, generic_data)
            + ")"
            + &self
                .return_type
                .to_descriptor_string(class_index, generic_data)
    }
}

impl ToIndexedType for RawEnclosingTypeInfo {
    type Out = IndexedEnclosingTypeInfo;

    fn to_indexed_type<'a>(
        &'a self,
        class_index: &ClassIndex,
        constant_pool_map: &mut HashMap<&'a AsciiStr, u32>,
    ) -> Self::Out {
        let class_name;
        let method_name;
        {
            // This block ensures that the constant pool reference is dropped before we index the method descriptor
            let pool = &mut class_index.constant_pool_mut();
            class_name =
                ClassIndexBuilder::get_index_from_pool(&self.class_name, constant_pool_map, pool);
            method_name = self.method_name.as_ref().map(|method_name| {
                ClassIndexBuilder::get_index_from_pool(method_name, constant_pool_map, pool)
            });
        }

        IndexedEnclosingTypeInfo::new(
            class_name,
            method_name,
            self.method_descriptor.as_ref().map(|method_signature| {
                method_signature.to_indexed_type(class_index, constant_pool_map)
            }),
        )
    }
}

impl<T, X> ToIndexedType for Option<T>
where
    T: ToIndexedType<Out = X>,
{
    type Out = Option<X>;

    fn to_indexed_type<'a>(
        &'a self,
        class_index: &ClassIndex,
        constant_pool_map: &mut HashMap<&'a AsciiStr, u32>,
    ) -> Self::Out {
        self.as_ref()
            .map(|t| t.to_indexed_type(class_index, constant_pool_map))
    }
}

impl<T, X> ToIndexedType for Vec<T>
where
    T: ToIndexedType<Out = X>,
{
    type Out = Vec<X>;

    fn to_indexed_type<'a>(
        &'a self,
        class_index: &ClassIndex,
        constant_pool_map: &mut HashMap<&'a AsciiStr, u32>,
    ) -> Self::Out {
        self.iter()
            .map(|s| s.to_indexed_type(class_index, constant_pool_map))
            .collect()
    }
}

impl<T> ToSignatureIndexedType for Option<T>
where
    T: ToSignatureIndexedType,
{
    fn to_signature_string(&self, class_index: &ClassIndex) -> String {
        self.as_ref().map_or(String::new(), |inner| {
            inner.to_signature_string(class_index)
        })
    }
}

impl<T> ToSignatureIndexedType for Vec<T>
where
    T: ToSignatureIndexedType,
{
    fn to_signature_string(&self, class_index: &ClassIndex) -> String {
        self.iter().fold(String::new(), |a, b| {
            a + &b.to_signature_string(class_index)
        })
    }
}

impl<T> ToDescriptorIndexedType for Vec<T>
where
    T: ToDescriptorIndexedType,
{
    fn to_descriptor_string(
        &self,
        class_index: &ClassIndex,
        generic_data: &[&IndexedTypeParameterData],
    ) -> String {
        self.iter().fold(String::new(), |a, b| {
            a + &b.to_descriptor_string(class_index, generic_data)
        })
    }
}

fn index_object_type(name: &AsciiString, class_index: &ClassIndex) -> IndexedSignatureType {
    let index_or_none = index_for_object_type(name, class_index);

    match index_or_none {
        Some(i) => IndexedSignatureType::Object(i),
        _ => IndexedSignatureType::Unresolved,
    }
}

fn index_for_object_type(name: &AsciiString, class_index: &ClassIndex) -> Option<u32> {
    let class_name_parts = rsplit_once(name, AsciiChar::Slash);
    class_index
        .find_class(class_name_parts.0, class_name_parts.1)
        .map(|s| s.0)
}
