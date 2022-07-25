use jni::objects::{JClass, JString};
use jni::sys::jstring;
use jni::JNIEnv;

#[no_mangle]
pub extern "system" fn Java_com_calsignlabs_metro_1simulator_Engine_hello(
    env: JNIEnv,
    _class: JClass,
    input: JString,
) -> jstring {
    let input: String = env.get_string(input).unwrap().into();
    let output = env.new_string(format!("Hello, {}!", input)).unwrap();
    output.into_inner()
}
