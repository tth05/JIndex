use crate::builder::{get_index_from_pool, ClassToIndexMap};
use crate::class_index::ClassIndex;
use crate::constant_pool::ClassIndexConstantPool;
use crate::rsplit_once;
use crate::signature::{
    IndexedClassSignature, IndexedEnclosingTypeInfo, IndexedMethodSignature, IndexedSignatureType,
    IndexedTypeParameterData, RawClassSignature, RawEnclosingTypeInfo, RawMethodSignature,
    RawSignatureType, RawTypeParameterData, SignatureType,
};
use ascii::{AsAsciiStr, AsciiChar, AsciiStr, AsciiString, IntoAsciiString};
use rustc_hash::FxHashMap;

pub trait ToIndexedType {
    type Out;

    fn to_indexed_type<'a>(
        &'a self,
        constant_pool: &mut ClassIndexConstantPool,
        constant_pool_map: &mut FxHashMap<&'a AsciiStr, u32>,
        class_to_index_map: &ClassToIndexMap,
    ) -> anyhow::Result<Self::Out>;
}

pub trait ToSignatureIndexedType {
    fn to_signature_string(&self, class_index: &ClassIndex) -> String {
        self.to_signature_string0(class_index, false)
    }

    fn to_signature_string0(&self, class_index: &ClassIndex, simple_class_names: bool) -> String;
}

pub trait ToDescriptorIndexedType {
    fn to_descriptor_string(
        &self,
        class_index: &ClassIndex,
        generic_data: &[&IndexedTypeParameterData],
    ) -> String;
}

impl<T> SignatureType<T> {
    pub fn extract_base_object_type(&self) -> Option<T>
    where
        T: Clone,
    {
        match &self {
            SignatureType::Object(t) => Some(t.clone()),
            SignatureType::ObjectPlus(t) => t.extract_base_object_type(),
            SignatureType::ObjectMinus(t) => t.extract_base_object_type(),
            SignatureType::ObjectTypeBounds(t) => Some(t.as_ref().0.clone()),
            SignatureType::ObjectInnerClass(parts) => {
                parts.last().unwrap().extract_base_object_type()
            }
            _ => None,
        }
    }
}

impl IndexedSignatureType {
    pub fn eq_erased(&self, other: &IndexedSignatureType) -> bool {
        match self {
            IndexedSignatureType::Primitive(p) => match other {
                IndexedSignatureType::Primitive(q) => p == q,
                _ => false,
            },
            IndexedSignatureType::Array(inner) => match other {
                IndexedSignatureType::Array(outer) => inner.eq_erased(outer),
                _ => false,
            },
            IndexedSignatureType::Unresolved => matches!(other, IndexedSignatureType::Unresolved),
            // Generics gets erased to Object
            IndexedSignatureType::Generic(_) => true,
            _ => match other {
                // Generics gets erased to Object
                IndexedSignatureType::Generic(_) => true,
                _ => {
                    // Compare the base object type
                    self.extract_base_object_type()
                        .and_then(|t| other.extract_base_object_type().map(|u| t == u))
                        == Some(true)
                }
            },
        }
    }

    pub fn resolve_generic_type_bound<'a>(
        &self,
        class_index: &ClassIndex,
        generic_data: &[&'a IndexedTypeParameterData],
    ) -> Option<&'a Self> {
        match self {
            IndexedSignatureType::Generic(name) => {
                let generic_param_name = class_index.constant_pool().string_view_at(*name);
                match generic_data
                    .iter()
                    .find(|g| {
                        class_index.constant_pool().string_view_at(g.name) == generic_param_name
                    })
                    .filter(|p| p.type_bound.is_some())
                {
                    Some(param) => param.type_bound.as_ref(),
                    None => Option::None, //If there's no type bound or the parameter was not found
                }
            }
            _ => Option::None,
        }
    }
}

impl ToIndexedType for RawSignatureType {
    type Out = IndexedSignatureType;

    fn to_indexed_type<'a>(
        &'a self,
        constant_pool: &mut ClassIndexConstantPool,
        constant_pool_map: &mut FxHashMap<&'a AsciiStr, u32>,
        class_to_index_map: &ClassToIndexMap,
    ) -> anyhow::Result<Self::Out> {
        Ok(match &self {
            RawSignatureType::Primitive(p) => IndexedSignatureType::Primitive(*p),
            RawSignatureType::Generic(name) => IndexedSignatureType::Generic(get_index_from_pool(
                name,
                constant_pool_map,
                constant_pool,
            )?),
            RawSignatureType::Array(inner) => IndexedSignatureType::Array(Box::new(
                inner.to_indexed_type(constant_pool, constant_pool_map, class_to_index_map)?,
            )),
            RawSignatureType::Object(name) => index_object_type(name, class_to_index_map),
            RawSignatureType::ObjectPlus(inner) => IndexedSignatureType::ObjectPlus(Box::new(
                inner.to_indexed_type(constant_pool, constant_pool_map, class_to_index_map)?,
            )),
            RawSignatureType::ObjectMinus(inner) => IndexedSignatureType::ObjectMinus(Box::new(
                inner.to_indexed_type(constant_pool, constant_pool_map, class_to_index_map)?,
            )),
            RawSignatureType::ObjectInnerClass(inner) => {
                let inner = inner.as_ref();
                let base_type_signature = inner.first().unwrap();
                let mut type_name = base_type_signature.extract_base_object_type().unwrap();

                let mut new_vec = Vec::with_capacity(inner.len());
                //Add base type
                new_vec.push(base_type_signature.to_indexed_type(
                    constant_pool,
                    constant_pool_map,
                    class_to_index_map,
                )?);
                //Add inner classes
                inner.iter().skip(1).for_each(|s| {
                    //Separator
                    type_name.push_str(unsafe { "$".as_ascii_str_unchecked() });
                    new_vec.push(match s {
                        RawSignatureType::Object(name) => {
                            //Add inner class name
                            type_name.push_str(name);

                            let index_or_none =
                                index_for_object_type(&type_name, class_to_index_map);

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
                                index_for_object_type(&type_name, class_to_index_map);

                            match main_type_index_or_none {
                                Some(main_type_index) => vec
                                    .to_indexed_type(
                                        constant_pool,
                                        constant_pool_map,
                                        class_to_index_map,
                                    )
                                    .ok()
                                    .map(|params| {
                                        IndexedSignatureType::ObjectTypeBounds(Box::new((
                                            main_type_index,
                                            params,
                                        )))
                                    })
                                    .unwrap_or(IndexedSignatureType::Unresolved),
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
                let main_type_index_or_none = index_for_object_type(main_type, class_to_index_map);

                let mut indexed_vec =
                    vec.to_indexed_type(constant_pool, constant_pool_map, class_to_index_map)?;
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
        })
    }
}

impl ToSignatureIndexedType for IndexedSignatureType {
    fn to_signature_string0(&self, class_index: &ClassIndex, simple_class_names: bool) -> String {
        match &self {
            SignatureType::ObjectTypeBounds(inner) => {
                let (actual_type, type_bounds) = inner.as_ref();

                String::from('L')
                    + get_full_class_name(class_index, actual_type, false).as_str()
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
                        .map(|s| s.to_signature_string0(class_index, true))
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
                            .rfind(|c| is_first || c == '/') //Remove the package name
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
                String::from('L')
                    + get_full_class_name(class_index, index, simple_class_names).as_str()
                    + ";"
            }
            SignatureType::Generic(index) => {
                String::from('T')
                    + class_index
                        .constant_pool()
                        .string_view_at(*index)
                        .into_ascii_str(class_index.constant_pool())
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
            SignatureType::Unresolved => String::from("Ljindex_unresolved;"),
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
                        .class_at_index(self.extract_base_object_type().unwrap())
                        .class_name_with_package(
                            class_index.package_index(),
                            class_index.constant_pool(),
                        )
                        .as_str()
                    + ";"
            }
            SignatureType::Generic(_) => self
                .resolve_generic_type_bound(class_index, generic_data)
                .map(|s| s.to_descriptor_string(class_index, generic_data))
                .unwrap_or_else(|| "Ljava/lang/Object;".to_owned()),
            SignatureType::Array(inner) => {
                String::from('[') + &inner.to_descriptor_string(class_index, generic_data)
            }
            SignatureType::Primitive(p) => p.to_string(),
            SignatureType::Unresolved => String::from("Ljindex_unresolved;"),
        }
    }
}

fn get_full_class_name(class_index: &ClassIndex, index: &u32, simple: bool) -> AsciiString {
    let class = class_index.class_at_index(*index);
    if simple {
        class.simple_class_name_with_package(
            class_index.package_index(),
            class_index.constant_pool(),
        )
    } else {
        class.class_name_with_package(class_index.package_index(), class_index.constant_pool())
    }
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
        constant_pool: &mut ClassIndexConstantPool,
        constant_pool_map: &mut FxHashMap<&'a AsciiStr, u32>,
        class_to_index_map: &ClassToIndexMap,
    ) -> anyhow::Result<Self::Out> {
        Ok(IndexedClassSignature::new(
            self.generic_data.to_indexed_type(
                constant_pool,
                constant_pool_map,
                class_to_index_map,
            )?,
            self.super_class.to_indexed_type(
                constant_pool,
                constant_pool_map,
                class_to_index_map,
            )?,
            self.interfaces.to_indexed_type(
                constant_pool,
                constant_pool_map,
                class_to_index_map,
            )?,
        ))
    }
}

impl ToSignatureIndexedType for IndexedClassSignature {
    fn to_signature_string0(&self, class_index: &ClassIndex, _: bool) -> String {
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
        constant_pool: &mut ClassIndexConstantPool,
        constant_pool_map: &mut FxHashMap<&'a AsciiStr, u32>,
        class_to_index_map: &ClassToIndexMap,
    ) -> anyhow::Result<Self::Out> {
        Ok(IndexedTypeParameterData::new(
            get_index_from_pool(&self.name, constant_pool_map, constant_pool)?,
            self.type_bound.to_indexed_type(
                constant_pool,
                constant_pool_map,
                class_to_index_map,
            )?,
            self.interface_bounds.to_indexed_type(
                constant_pool,
                constant_pool_map,
                class_to_index_map,
            )?,
        ))
    }
}

impl ToSignatureIndexedType for IndexedTypeParameterData {
    fn to_signature_string0(&self, class_index: &ClassIndex, _: bool) -> String {
        class_index
            .constant_pool()
            .string_view_at(self.name)
            .into_ascii_str(class_index.constant_pool())
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
        parameters: Option<Vec<IndexedSignatureType>>,
        return_type: IndexedSignatureType,
        exceptions: Option<Vec<IndexedSignatureType>>,
    ) -> Self {
        Self {
            generic_data: generic_data.map(|mut v| {
                v.shrink_to_fit();
                Box::new(v)
            }),
            parameters: parameters.map(|mut v| {
                v.shrink_to_fit();
                Box::new(v)
            }),
            return_type,
            exceptions: exceptions.map(|mut v| {
                v.shrink_to_fit();
                Box::new(v)
            }),
        }
    }
}

impl ToIndexedType for RawMethodSignature {
    type Out = IndexedMethodSignature;

    fn to_indexed_type<'a>(
        &'a self,
        constant_pool: &mut ClassIndexConstantPool,
        constant_pool_map: &mut FxHashMap<&'a AsciiStr, u32>,
        class_to_index_map: &ClassToIndexMap,
    ) -> anyhow::Result<Self::Out> {
        Ok(IndexedMethodSignature::new(
            self.generic_data.to_indexed_type(
                constant_pool,
                constant_pool_map,
                class_to_index_map,
            )?,
            self.parameters.to_indexed_type(
                constant_pool,
                constant_pool_map,
                class_to_index_map,
            )?,
            self.return_type.to_indexed_type(
                constant_pool,
                constant_pool_map,
                class_to_index_map,
            )?,
            self.exceptions.to_indexed_type(
                constant_pool,
                constant_pool_map,
                class_to_index_map,
            )?,
        ))
    }
}

impl ToSignatureIndexedType for IndexedMethodSignature {
    fn to_signature_string0(&self, class_index: &ClassIndex, _: bool) -> String {
        (if self.generic_data.is_some() {
            String::from('<') + &self.generic_data.to_signature_string(class_index) + ">"
        } else {
            String::new()
        }) + "("
            + &self.parameters.to_signature_string(class_index)
            + ")"
            + &self.return_type.to_signature_string(class_index)
            + &self
                .exceptions()
                .filter(|v| {
                    v.iter()
                        .any(|e| matches!(e, IndexedSignatureType::Generic(_)))
                })
                .map(|v| {
                    v.iter()
                        .map(|e| String::from('^') + &e.to_signature_string(class_index))
                        .collect::<Vec<_>>()
                        .join("")
                })
                .unwrap_or_default()
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
        constant_pool: &mut ClassIndexConstantPool,
        constant_pool_map: &mut FxHashMap<&'a AsciiStr, u32>,
        class_to_index_map: &ClassToIndexMap,
    ) -> anyhow::Result<Self::Out> {
        let class_name =
            index_for_object_type(self.class_name.as_ref().unwrap(), class_to_index_map);

        Ok(IndexedEnclosingTypeInfo::new(
            class_name,
            self.inner_class_type,
            match self.method_name.as_ref() {
                Some(method_name) => Some(get_index_from_pool(
                    method_name,
                    constant_pool_map,
                    constant_pool,
                )?),
                None => None,
            },
            match self.method_descriptor.as_ref() {
                Some(method_signature) => Some(method_signature.to_indexed_type(
                    constant_pool,
                    constant_pool_map,
                    class_to_index_map,
                )?),
                None => None,
            },
        ))
    }
}

impl<T, X> ToIndexedType for Option<T>
where
    T: ToIndexedType<Out = X>,
{
    type Out = Option<X>;

    fn to_indexed_type<'a>(
        &'a self,
        constant_pool: &mut ClassIndexConstantPool,
        constant_pool_map: &mut FxHashMap<&'a AsciiStr, u32>,
        class_to_index_map: &ClassToIndexMap,
    ) -> anyhow::Result<Self::Out> {
        Ok(match self.as_ref() {
            Some(t) => {
                Some(t.to_indexed_type(constant_pool, constant_pool_map, class_to_index_map)?)
            }
            None => None,
        })
    }
}

impl<T, X> ToIndexedType for Box<T>
where
    T: ToIndexedType<Out = X>,
{
    type Out = X;

    fn to_indexed_type<'a>(
        &'a self,
        constant_pool: &mut ClassIndexConstantPool,
        constant_pool_map: &mut FxHashMap<&'a AsciiStr, u32>,
        class_to_index_map: &ClassToIndexMap,
    ) -> anyhow::Result<Self::Out> {
        self.as_ref()
            .to_indexed_type(constant_pool, constant_pool_map, class_to_index_map)
    }
}

impl<T, X> ToIndexedType for Vec<T>
where
    T: ToIndexedType<Out = X>,
{
    type Out = Vec<X>;

    fn to_indexed_type<'a>(
        &'a self,
        constant_pool: &mut ClassIndexConstantPool,
        constant_pool_map: &mut FxHashMap<&'a AsciiStr, u32>,
        class_to_index_map: &ClassToIndexMap,
    ) -> anyhow::Result<Self::Out> {
        self.iter()
            .map(|s| s.to_indexed_type(constant_pool, constant_pool_map, class_to_index_map))
            .collect()
    }
}

impl<T> ToSignatureIndexedType for Option<T>
where
    T: ToSignatureIndexedType,
{
    fn to_signature_string0(&self, class_index: &ClassIndex, _: bool) -> String {
        self.as_ref().map_or(String::new(), |inner| {
            inner.to_signature_string(class_index)
        })
    }
}

impl<T> ToSignatureIndexedType for Box<T>
where
    T: ToSignatureIndexedType,
{
    fn to_signature_string0(&self, class_index: &ClassIndex, _: bool) -> String {
        self.as_ref().to_signature_string(class_index)
    }
}

impl<T> ToSignatureIndexedType for Vec<T>
where
    T: ToSignatureIndexedType,
{
    fn to_signature_string0(&self, class_index: &ClassIndex, _: bool) -> String {
        self.iter().fold(String::new(), |a, b| {
            a + &b.to_signature_string(class_index)
        })
    }
}

impl<T> ToDescriptorIndexedType for Option<T>
where
    T: ToDescriptorIndexedType,
{
    fn to_descriptor_string(
        &self,
        class_index: &ClassIndex,
        generic_data: &[&IndexedTypeParameterData],
    ) -> String {
        self.as_ref()
            .map(|t| t.to_descriptor_string(class_index, generic_data))
            .unwrap_or_default()
    }
}

impl<T> ToDescriptorIndexedType for Box<T>
where
    T: ToDescriptorIndexedType,
{
    fn to_descriptor_string(
        &self,
        class_index: &ClassIndex,
        generic_data: &[&IndexedTypeParameterData],
    ) -> String {
        self.as_ref()
            .to_descriptor_string(class_index, generic_data)
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

fn index_object_type(
    name: &AsciiString,
    class_to_index_map: &ClassToIndexMap,
) -> IndexedSignatureType {
    let index_or_none = index_for_object_type(name, class_to_index_map);

    match index_or_none {
        Some(i) => IndexedSignatureType::Object(i),
        _ => IndexedSignatureType::Unresolved,
    }
}

fn index_for_object_type(name: &AsciiString, class_to_index_map: &ClassToIndexMap) -> Option<u32> {
    let class_name_parts = rsplit_once(name, AsciiChar::Slash);
    class_to_index_map.get(&class_name_parts).map(|p| p.0)
}
