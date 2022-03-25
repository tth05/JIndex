use crate::class_index::{rsplit_once, ClassIndex, ClassIndexBuilder};
use crate::signature::{
    IndexedClassSignature, IndexedMethodSignature, IndexedSignatureType, IndexedTypeParameterData,
    RawClassSignature, RawMethodSignature, RawSignatureType, RawTypeParameterData, SignatureType,
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
                let base_type_signature_name = base_type_signature.extract_base_object_type();
                let indexed_base_type_signature =
                    base_type_signature.to_indexed_type(class_index, constant_pool_map);

                let mut new_vec = Vec::with_capacity(inner.len());
                new_vec.push(indexed_base_type_signature);
                inner.iter().skip(1).for_each(|s| {
                    new_vec.push(match s {
                        RawSignatureType::Object(name) => {
                            let index_or_none = index_for_object_type(
                                &(base_type_signature_name.clone()
                                    + "$".as_ascii_str().unwrap()
                                    + name),
                                class_index,
                            );

                            match index_or_none {
                                Some(i) => IndexedSignatureType::Object(i),
                                _ => IndexedSignatureType::Unresolved,
                            }
                        }
                        RawSignatureType::ObjectTypeBounds(inner) => {
                            //TODO: Somewhat copied from RawSignatureType::ObjectTypeBounds(inner) => { match arm
                            let (main_type, vec) = inner.as_ref();
                            let main_type_index_or_none = index_for_object_type(
                                &(base_type_signature_name.clone()
                                    + "$".as_ascii_str().unwrap()
                                    + main_type),
                                class_index,
                            );

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
                match main_type_index_or_none {
                    Some(main_type_index) => IndexedSignatureType::ObjectTypeBounds(Box::new((
                        main_type_index,
                        vec.to_indexed_type(class_index, constant_pool_map),
                    ))),
                    _ => IndexedSignatureType::Unresolved,
                }
            }
            _ => unreachable!(),
        }
    }
}

impl IndexedClassSignature {
    pub fn new(
        generic_data: Option<Vec<IndexedTypeParameterData>>,
        super_class: Option<IndexedSignatureType>,
        interfaces: Option<Vec<IndexedSignatureType>>,
    ) -> Self {
        Self {
            generic_data,
            super_class,
            interfaces,
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

impl IndexedMethodSignature {
    pub fn new(
        generic_data: Option<Vec<IndexedTypeParameterData>>,
        parameters: Vec<IndexedSignatureType>,
        return_type: IndexedSignatureType,
    ) -> Self {
        Self {
            generic_data,
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

impl<T> SignatureType<T> {
    pub fn extract_base_object_type(&self) -> &T {
        match &self {
            SignatureType::Object(t) => t,
            SignatureType::ObjectPlus(t) => t.extract_base_object_type(),
            SignatureType::ObjectMinus(t) => t.extract_base_object_type(),
            SignatureType::ObjectTypeBounds(t) => &t.as_ref().0,
            _ => panic!("Not an object type"),
        }
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
