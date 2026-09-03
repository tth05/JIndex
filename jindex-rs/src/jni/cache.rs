use crate::class_index::ClassIndex;
use jni::objects::{JFieldID, JObject};
use jni::signature::{Primitive, ReturnType};
use jni::sys::jlong;
use jni::{jni_sig, jni_str, Env};
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

pub unsafe fn init_field_ids(env: &mut Env<'_>) -> anyhow::Result<()> {
    if CACHED_FIELD_IDS.get().is_some() {
        return Ok(());
    }

    unsafe fn transmute_field_id(
        env: &mut Env<'_>,
        name: &'static jni::strings::JNIStr,
        class_name: &'static jni::strings::JNIStr,
    ) -> anyhow::Result<JFieldID> {
        let class = env.find_class(class_name)?;
        Ok(env.get_field_id(&class, name, jni_sig!("J"))?)
    }

    let _ = CACHED_FIELD_IDS.set(FieldIDs {
        class_index_pointer: transmute_field_id(
            env,
            jni_str!("classIndexPointer"),
            jni_str!("com/github/tth05/jindex/ClassIndexChildObject"),
        )?,
        class_index_child_self_pointer: transmute_field_id(
            env,
            jni_str!("pointer"),
            jni_str!("com/github/tth05/jindex/ClassIndexChildObject"),
        )?,
        class_child_class_pointer: transmute_field_id(
            env,
            jni_str!("classPointer"),
            jni_str!("com/github/tth05/jindex/ClassChildObject"),
        )?,
    });
    Ok(())
}

/// Borrows a native child for one JNI invocation.
///
/// # Safety
/// `this` must belong to a live index. Its Java owner must hold the lifecycle read
/// lock until this borrow ends, and the field must point to a `T` in that index.
pub unsafe fn get_field_with_id<'a, T>(
    env: &mut Env<'_>,
    this: &'a JObject<'_>,
    field_id: &JFieldID,
) -> &'a T {
    let pointer = env
        .get_field_unchecked(this, *field_id, ReturnType::Primitive(Primitive::Long))
        .unwrap()
        .j()
        .unwrap() as *const T;
    assert!(!pointer.is_null(), "Native index child pointer is null");
    &*pointer
}

/// # Safety
/// `this` must retain its Java owner, whose lifecycle read lock must remain held
/// for the returned borrow. Only the owner's cleanup action may free this index.
pub unsafe fn get_class_index<'a>(
    env: &mut Env<'_>,
    this: &'a JObject<'_>,
) -> (jlong, &'a ClassIndex) {
    let class_index_pointer = env
        .get_field_unchecked(
            this,
            cached_field_ids().class_index_pointer,
            ReturnType::Primitive(Primitive::Long),
        )
        .unwrap()
        .j()
        .unwrap();
    assert_ne!(class_index_pointer, 0, "Native class index pointer is null");
    let class_index = &*(class_index_pointer as *const ClassIndex);
    (class_index_pointer, class_index)
}
