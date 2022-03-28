use crate::class_index::ClassIndex;
use jni::objects::JFieldID;
use jni::signature::{JavaType, Primitive};
use jni::sys::{jlong, jobject};
use jni::JNIEnv;
use std::lazy::SyncOnceCell;

pub struct FieldIDs {
    pub class_index_pointer_id: JFieldID<'static>,
    pub indexed_class_pointer_id: JFieldID<'static>,
    pub indexed_class_index_pointer_id: JFieldID<'static>,
    pub indexed_method_pointer_id: JFieldID<'static>,
    pub indexed_method_class_pointer_id: JFieldID<'static>,
    pub indexed_method_index_pointer_id: JFieldID<'static>,
    pub indexed_field_pointer_id: JFieldID<'static>,
    pub indexed_field_class_pointer_id: JFieldID<'static>,
    pub indexed_field_index_pointer_id: JFieldID<'static>,
}
unsafe impl Send for FieldIDs {}
unsafe impl Sync for FieldIDs {}

static CACHED_FIELD_IDS: SyncOnceCell<FieldIDs> = SyncOnceCell::new();

pub fn cached_field_ids() -> &'static FieldIDs {
    CACHED_FIELD_IDS.get().unwrap()
}

pub unsafe fn init_field_ids(env: JNIEnv) {
    if CACHED_FIELD_IDS.get().is_some() {
        return;
    }

    unsafe fn transmute_field_id(env: JNIEnv, name: &str, class_name: &str) -> JFieldID<'static> {
        std::mem::transmute::<_, _>(
            env.get_field_id(
                env.find_class("com/github/tth05/jindex/".to_owned() + class_name)
                    .unwrap(),
                name,
                "J",
            )
            .unwrap(),
        )
    }

    let _ = CACHED_FIELD_IDS.set(FieldIDs {
        class_index_pointer_id: transmute_field_id(env, "classIndexPointer", "ClassIndex"),
        indexed_class_pointer_id: transmute_field_id(env, "pointer", "IndexedClass"),
        indexed_class_index_pointer_id: transmute_field_id(
            env,
            "classIndexPointer",
            "IndexedClass",
        ),
        indexed_method_pointer_id: transmute_field_id(env, "pointer", "IndexedMethod"),
        indexed_method_class_pointer_id: transmute_field_id(env, "classPointer", "IndexedMethod"),
        indexed_method_index_pointer_id: transmute_field_id(
            env,
            "classIndexPointer",
            "IndexedMethod",
        ),
        indexed_field_pointer_id: transmute_field_id(env, "pointer", "IndexedField"),
        indexed_field_class_pointer_id: transmute_field_id(env, "classPointer", "IndexedField"),
        indexed_field_index_pointer_id: transmute_field_id(
            env,
            "classIndexPointer",
            "IndexedField",
        ),
    });
}

pub unsafe fn get_field_with_id<'a, T>(env: JNIEnv, this: jobject, field_id: &JFieldID) -> &'a T {
    &*(env
        .get_field_unchecked(this, *field_id, JavaType::Primitive(Primitive::Long))
        .unwrap()
        .j()
        .unwrap() as *mut T)
}

pub unsafe fn get_class_index(
    env: JNIEnv,
    this: jobject,
    field_id: &JFieldID,
) -> (jlong, &'static ClassIndex) {
    let class_index_pointer = env
        .get_field_unchecked(this, *field_id, JavaType::Primitive(Primitive::Long))
        .unwrap()
        .j()
        .unwrap();
    let class_index = &*(class_index_pointer as *const ClassIndex);
    (class_index_pointer, class_index)
}