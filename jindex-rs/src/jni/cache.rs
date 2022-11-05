use crate::class_index::ClassIndex;
use jni::objects::{JFieldID, JObject};
use jni::signature::{JavaType, Primitive, ReturnType};
use jni::sys::{jlong, jobject};
use jni::JNIEnv;
use once_cell::sync::OnceCell;

pub struct FieldIDs {
    pub class_index_pointer: JFieldID,
    pub class_index_child_self_pointer: JFieldID,
    pub class_child_class_pointer: JFieldID,
}

//TODO: Cache constructors
static CACHED_FIELD_IDS: OnceCell<FieldIDs> = OnceCell::new();

pub fn cached_field_ids() -> &'static FieldIDs {
    CACHED_FIELD_IDS.get().unwrap()
}

pub unsafe fn init_field_ids(env: JNIEnv) -> anyhow::Result<()> {
    if CACHED_FIELD_IDS.get().is_some() {
        return Ok(());
    }

    unsafe fn transmute_field_id(
        env: JNIEnv,
        name: &str,
        class_name: &str,
    ) -> anyhow::Result<JFieldID> {
        Ok(std::mem::transmute::<_, _>(env.get_field_id(
            env.find_class("com/github/tth05/jindex/".to_owned() + class_name)?,
            name,
            "J",
        )?))
    }

    let _ = CACHED_FIELD_IDS.set(FieldIDs {
        class_index_pointer: transmute_field_id(env, "classIndexPointer", "ClassIndexChildObject")?,
        class_index_child_self_pointer: transmute_field_id(
            env,
            "pointer",
            "ClassIndexChildObject",
        )?,
        class_child_class_pointer: transmute_field_id(env, "classPointer", "ClassChildObject")?,
    });
    Ok(())
}

pub unsafe fn get_field_with_id<'a, T>(env: JNIEnv, this: JObject, field_id: &JFieldID) -> &'a T {
    &*(env
        .get_field_unchecked(this, *field_id, ReturnType::Primitive(Primitive::Long))
        .unwrap()
        .j()
        .unwrap() as *mut T)
}

pub unsafe fn get_class_index(env: JNIEnv, this: JObject) -> (jlong, &'static ClassIndex) {
    let class_index_pointer = env
        .get_field_unchecked(
            this,
            cached_field_ids().class_index_pointer,
            ReturnType::Primitive(Primitive::Long),
        )
        .unwrap()
        .j()
        .unwrap();
    let class_index = &*(class_index_pointer as *const ClassIndex);
    (class_index_pointer, class_index)
}
