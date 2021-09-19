//! Representations of JavaScript's core builtin types.
//!
//! ## Modeling JavaScript Types
//!
//! All JavaScript values in Neon implement the abstract [`Value`] trait, which
//! is the most generic way to work with JavaScript values. Neon provides a
//! number of types that implement this trait, each representing a particular
//! type of JavaScript value.
//!
//! By convention, JavaScript types in Neon have the prefix `Js` in their name,
//! such as [`JsNumber`](crate::types::JsNumber) (for the JavaScript `number`
//! type) or [`JsFunction`](crate::types::JsFunction) (for the JavaScript
//! `function` type).
//!
//! ### Handles and Casts
//!
//! Access to JavaScript values in Neon works through [handles](crate::handle),
//! which ensure the safe interoperation between Rust and the JavaScript garbage
//! collector. This means, for example, a Rust variable that stores a JavaScript string
//! will have the type `Handle<JsString>` rather than [`JsString`](crate::types::JsString).
//!
//! Neon types model the JavaScript type hierarchy through the use of *casts*.
//! The [`Handle::upcast()`](crate::handle::Handle::upcast) method safely converts
//! a handle to a JavaScript value of one type into a handle to a value of its
//! supertype. For example, it's safe to treat a [`JsArray`](crate::types::JsArray)
//! as a [`JsObject`](crate::types::JsObject), so you can do an "upcast" and it will
//! never fail:
//!
//! ```
//! # use neon::prelude::*;
//! fn as_object(array: Handle<JsArray>) -> Handle<JsObject> {
//!     let object: Handle<JsObject> = array.upcast();
//!     object
//! }
//! ```
//!
//! Unlike upcasts, the [`Handle::downcast()`](crate::handle::Handle::downcast) method
//! requires a runtime check to test a value's type at runtime, so it can fail with
//! a [`DowncastError`](crate::handle::DowncastError):
//!
//! ```
//! # use neon::prelude::*;
//! fn as_array<'a>(
//!     cx: &mut impl Context<'a>,
//!     object: Handle<'a, JsObject>
//! ) -> JsResult<'a, JsArray> {
//! #   #[cfg(feature = "legacy-runtime")]
//! #   return object.downcast().or_throw(cx);
//! #   #[cfg(feature = "napi-1")]
//! #   return
//!     object.downcast(cx).or_throw(cx)
//! #   ;
//! }
//! ```
//!
//! ### The JavaScript Type Hierarchy
//!
//! ![The Neon type hierarchy, described in detail below.][types]
//!
//! The JavaScript type hierarchy includes:
//!
//! - [`JsValue`](JsValue): This is the top of the type hierarchy, and can refer to
//!   any JavaScript value. (For TypeScript programmers, this can be thought of as
//!   similar to TypeScript's [`unknown`][unknown] type.)
//! - [`JsObject`](JsObject): This is the top of the object type hierarchy. Object
//!   types all implement the [`Object`](crate::object::Object) trait, which allows
//!   getting and setting properties.
//!   - **Standard object types:** [`JsFunction`](JsFunction), [`JsArray`](JsArray),
//!     [`JsDate`](JsDate), and [`JsError`](JsError).
//!   - **Typed arrays:** [`JsBuffer`](JsBuffer) and [`JsArrayBuffer`](JsArrayBuffer).
//!   - **Custom types:** [`JsBox`](JsBox), a special Neon type that allows the creation
//!     of custom objects that own Rust data structures.
//! - **Primitive types:** These are the built-in JavaScript datatypes that are not
//!   object types: [`JsNumber`](JsNumber), [`JsBoolean`](JsBoolean),
//!   [`JsString`](JsString), [`JsNull`](JsNull), and [`JsUndefined`](JsUndefined).
//!
//! [types]: https://raw.githubusercontent.com/neon-bindings/neon/main/doc/types.jpg
//! [unknown]: https://mariusschulz.com/blog/the-unknown-type-in-typescript#the-unknown-type

pub(crate) mod binary;
#[cfg(feature = "napi-1")]
pub(crate) mod boxed;
#[cfg(feature = "napi-5")]
pub(crate) mod date;
pub(crate) mod error;

pub(crate) mod internal;
pub(crate) mod utf8;

use self::internal::{FunctionCallback, ValueInternal};
use self::utf8::Utf8;
use crate::context::internal::Env;
use crate::context::{Context, FunctionContext};
use crate::handle::internal::SuperType;
use crate::handle::{Handle, Managed};
use crate::object::{Object, This};
use crate::result::{JsResult, JsResultExt, NeonResult, Throw};
use crate::types::internal::Callback;
use neon_runtime;
use neon_runtime::raw;
use smallvec::SmallVec;
use std::fmt;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::os::raw::c_void;

pub use self::binary::{BinaryData, BinaryViewType, JsArrayBuffer, JsBuffer};
#[cfg(feature = "napi-1")]
pub use self::boxed::{Finalize, JsBox};
#[cfg(feature = "napi-5")]
pub use self::date::{DateError, DateErrorKind, JsDate};
pub use self::error::JsError;

pub(crate) fn build<'a, T: Managed, F: FnOnce(&mut raw::Local) -> bool>(
    env: Env,
    init: F,
) -> JsResult<'a, T> {
    unsafe {
        let mut local: raw::Local = std::mem::zeroed();
        if init(&mut local) {
            Ok(Handle::new_internal(T::from_raw(env, local)))
        } else {
            Err(Throw)
        }
    }
}

impl<T: Value> SuperType<T> for JsValue {
    fn upcast_internal(v: T) -> JsValue {
        JsValue(v.to_raw())
    }
}

impl<T: Object> SuperType<T> for JsObject {
    fn upcast_internal(v: T) -> JsObject {
        JsObject(v.to_raw())
    }
}

/// The trait shared by all JavaScript values.
pub trait Value: ValueInternal {
    fn to_string<'a, C: Context<'a>>(self, cx: &mut C) -> JsResult<'a, JsString> {
        let env = cx.env();
        build(env, |out| unsafe {
            neon_runtime::convert::to_string(out, env.to_raw(), self.to_raw())
        })
    }

    fn as_value<'a, C: Context<'a>>(self, _: &mut C) -> Handle<'a, JsValue> {
        JsValue::new_internal(self.to_raw())
    }
}

/// A JavaScript value of any type.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct JsValue(raw::Local);

impl Value for JsValue {}

impl Managed for JsValue {
    fn to_raw(self) -> raw::Local {
        self.0
    }

    fn from_raw(_: Env, h: raw::Local) -> Self {
        JsValue(h)
    }
}

impl ValueInternal for JsValue {
    fn name() -> String {
        "any".to_string()
    }

    fn is_typeof<Other: Value>(_env: Env, _other: Other) -> bool {
        true
    }
}

unsafe impl This for JsValue {
    #[cfg(feature = "legacy-runtime")]
    fn as_this(h: raw::Local) -> Self {
        JsValue(h)
    }

    #[cfg(feature = "napi-1")]
    fn as_this(_env: Env, h: raw::Local) -> Self {
        JsValue(h)
    }
}

impl JsValue {
    pub(crate) fn new_internal<'a>(value: raw::Local) -> Handle<'a, JsValue> {
        Handle::new_internal(JsValue(value))
    }
}

/// The JavaScript `undefined` value.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct JsUndefined(raw::Local);

impl JsUndefined {
    #[cfg(feature = "legacy-runtime")]
    pub fn new<'a>() -> Handle<'a, JsUndefined> {
        JsUndefined::new_internal(Env::current())
    }

    #[cfg(feature = "napi-1")]
    pub fn new<'a, C: Context<'a>>(cx: &mut C) -> Handle<'a, JsUndefined> {
        JsUndefined::new_internal(cx.env())
    }

    pub(crate) fn new_internal<'a>(env: Env) -> Handle<'a, JsUndefined> {
        unsafe {
            let mut local: raw::Local = std::mem::zeroed();
            neon_runtime::primitive::undefined(&mut local, env.to_raw());
            Handle::new_internal(JsUndefined(local))
        }
    }

    #[allow(clippy::wrong_self_convention)]
    fn as_this_compat(env: Env, _: raw::Local) -> Self {
        unsafe {
            let mut local: raw::Local = std::mem::zeroed();
            neon_runtime::primitive::undefined(&mut local, env.to_raw());
            JsUndefined(local)
        }
    }
}

impl Value for JsUndefined {}

impl Managed for JsUndefined {
    fn to_raw(self) -> raw::Local {
        self.0
    }

    fn from_raw(_: Env, h: raw::Local) -> Self {
        JsUndefined(h)
    }
}

unsafe impl This for JsUndefined {
    #[cfg(feature = "legacy-runtime")]
    fn as_this(h: raw::Local) -> Self {
        JsUndefined::as_this_compat(Env::current(), h)
    }

    #[cfg(feature = "napi-1")]
    fn as_this(env: Env, h: raw::Local) -> Self {
        JsUndefined::as_this_compat(env, h)
    }
}

impl ValueInternal for JsUndefined {
    fn name() -> String {
        "undefined".to_string()
    }

    fn is_typeof<Other: Value>(env: Env, other: Other) -> bool {
        unsafe { neon_runtime::tag::is_undefined(env.to_raw(), other.to_raw()) }
    }
}

/// The JavaScript `null` value.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct JsNull(raw::Local);

impl JsNull {
    #[cfg(feature = "legacy-runtime")]
    pub fn new<'a>() -> Handle<'a, JsNull> {
        JsNull::new_internal(Env::current())
    }

    #[cfg(feature = "napi-1")]
    pub fn new<'a, C: Context<'a>>(cx: &mut C) -> Handle<'a, JsNull> {
        JsNull::new_internal(cx.env())
    }

    pub(crate) fn new_internal<'a>(env: Env) -> Handle<'a, JsNull> {
        unsafe {
            let mut local: raw::Local = std::mem::zeroed();
            neon_runtime::primitive::null(&mut local, env.to_raw());
            Handle::new_internal(JsNull(local))
        }
    }
}

impl Value for JsNull {}

impl Managed for JsNull {
    fn to_raw(self) -> raw::Local {
        self.0
    }

    fn from_raw(_: Env, h: raw::Local) -> Self {
        JsNull(h)
    }
}

impl ValueInternal for JsNull {
    fn name() -> String {
        "null".to_string()
    }

    fn is_typeof<Other: Value>(env: Env, other: Other) -> bool {
        unsafe { neon_runtime::tag::is_null(env.to_raw(), other.to_raw()) }
    }
}

/// A JavaScript boolean primitive value.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct JsBoolean(raw::Local);

impl JsBoolean {
    pub fn new<'a, C: Context<'a>>(cx: &mut C, b: bool) -> Handle<'a, JsBoolean> {
        JsBoolean::new_internal(cx.env(), b)
    }

    pub(crate) fn new_internal<'a>(env: Env, b: bool) -> Handle<'a, JsBoolean> {
        unsafe {
            let mut local: raw::Local = std::mem::zeroed();
            neon_runtime::primitive::boolean(&mut local, env.to_raw(), b);
            Handle::new_internal(JsBoolean(local))
        }
    }

    #[cfg(feature = "legacy-runtime")]
    pub fn value(self) -> bool {
        unsafe { neon_runtime::primitive::boolean_value(self.to_raw()) }
    }

    #[cfg(feature = "napi-1")]
    pub fn value<'a, C: Context<'a>>(self, cx: &mut C) -> bool {
        let env = cx.env().to_raw();
        unsafe { neon_runtime::primitive::boolean_value(env, self.to_raw()) }
    }
}

impl Value for JsBoolean {}

impl Managed for JsBoolean {
    fn to_raw(self) -> raw::Local {
        self.0
    }

    fn from_raw(_: Env, h: raw::Local) -> Self {
        JsBoolean(h)
    }
}

impl ValueInternal for JsBoolean {
    fn name() -> String {
        "boolean".to_string()
    }

    fn is_typeof<Other: Value>(env: Env, other: Other) -> bool {
        unsafe { neon_runtime::tag::is_boolean(env.to_raw(), other.to_raw()) }
    }
}

/// A JavaScript string primitive value.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct JsString(raw::Local);

/// An error produced when constructing a string that exceeds the JS engine's maximum string size.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub struct StringOverflow(usize);

impl fmt::Display for StringOverflow {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "string size out of range: {}", self.0)
    }
}

/// The result of constructing a new `JsString`.
pub type StringResult<'a> = Result<Handle<'a, JsString>, StringOverflow>;

impl<'a> JsResultExt<'a, JsString> for StringResult<'a> {
    fn or_throw<'b, C: Context<'b>>(self, cx: &mut C) -> JsResult<'a, JsString> {
        match self {
            Ok(v) => Ok(v),
            Err(e) => cx.throw_range_error(&e.to_string()),
        }
    }
}

impl Value for JsString {}

impl Managed for JsString {
    fn to_raw(self) -> raw::Local {
        self.0
    }

    fn from_raw(_: Env, h: raw::Local) -> Self {
        JsString(h)
    }
}

impl ValueInternal for JsString {
    fn name() -> String {
        "string".to_string()
    }

    fn is_typeof<Other: Value>(env: Env, other: Other) -> bool {
        unsafe { neon_runtime::tag::is_string(env.to_raw(), other.to_raw()) }
    }
}

impl JsString {
    #[cfg(feature = "legacy-runtime")]
    pub fn size(self) -> isize {
        unsafe { neon_runtime::string::utf8_len(self.to_raw()) }
    }

    #[cfg(feature = "napi-1")]
    pub fn size<'a, C: Context<'a>>(self, cx: &mut C) -> isize {
        let env = cx.env().to_raw();

        unsafe { neon_runtime::string::utf8_len(env, self.to_raw()) }
    }

    #[cfg(feature = "legacy-runtime")]
    pub fn value(self) -> String {
        unsafe {
            let capacity = neon_runtime::string::utf8_len(self.to_raw());
            let mut buffer: Vec<u8> = Vec::with_capacity(capacity as usize);
            let p = buffer.as_mut_ptr();
            std::mem::forget(buffer);
            let len = neon_runtime::string::data(p, capacity, self.to_raw());
            String::from_raw_parts(p, len as usize, capacity as usize)
        }
    }

    #[cfg(feature = "napi-1")]
    pub fn value<'a, C: Context<'a>>(self, cx: &mut C) -> String {
        let env = cx.env().to_raw();

        unsafe {
            let capacity = neon_runtime::string::utf8_len(env, self.to_raw()) + 1;
            let mut buffer: Vec<u8> = Vec::with_capacity(capacity as usize);
            let p = buffer.as_mut_ptr();
            std::mem::forget(buffer);
            let len = neon_runtime::string::data(env, p, capacity, self.to_raw());
            String::from_raw_parts(p, len as usize, capacity as usize)
        }
    }

    pub fn new<'a, C: Context<'a>, S: AsRef<str>>(cx: &mut C, val: S) -> Handle<'a, JsString> {
        JsString::try_new(cx, val).unwrap()
    }

    pub fn try_new<'a, C: Context<'a>, S: AsRef<str>>(cx: &mut C, val: S) -> StringResult<'a> {
        let val = val.as_ref();
        match JsString::new_internal(cx.env(), val) {
            Some(s) => Ok(s),
            None => Err(StringOverflow(val.len())),
        }
    }

    pub(crate) fn new_internal<'a>(env: Env, val: &str) -> Option<Handle<'a, JsString>> {
        let (ptr, len) = if let Some(small) = Utf8::from(val).into_small() {
            small.lower()
        } else {
            return None;
        };

        unsafe {
            let mut local: raw::Local = std::mem::zeroed();
            if neon_runtime::string::new(&mut local, env.to_raw(), ptr, len) {
                Some(Handle::new_internal(JsString(local)))
            } else {
                None
            }
        }
    }
}

/// A JavaScript number value.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct JsNumber(raw::Local);

impl JsNumber {
    pub fn new<'a, C: Context<'a>, T: Into<f64>>(cx: &mut C, x: T) -> Handle<'a, JsNumber> {
        JsNumber::new_internal(cx.env(), x.into())
    }

    pub(crate) fn new_internal<'a>(env: Env, v: f64) -> Handle<'a, JsNumber> {
        unsafe {
            let mut local: raw::Local = std::mem::zeroed();
            neon_runtime::primitive::number(&mut local, env.to_raw(), v);
            Handle::new_internal(JsNumber(local))
        }
    }

    #[cfg(feature = "legacy-runtime")]
    pub fn value(self) -> f64 {
        unsafe { neon_runtime::primitive::number_value(self.to_raw()) }
    }

    #[cfg(feature = "napi-1")]
    pub fn value<'a, C: Context<'a>>(self, cx: &mut C) -> f64 {
        let env = cx.env().to_raw();
        unsafe { neon_runtime::primitive::number_value(env, self.to_raw()) }
    }
}

impl Value for JsNumber {}

impl Managed for JsNumber {
    fn to_raw(self) -> raw::Local {
        self.0
    }

    fn from_raw(_: Env, h: raw::Local) -> Self {
        JsNumber(h)
    }
}

impl ValueInternal for JsNumber {
    fn name() -> String {
        "number".to_string()
    }

    fn is_typeof<Other: Value>(env: Env, other: Other) -> bool {
        unsafe { neon_runtime::tag::is_number(env.to_raw(), other.to_raw()) }
    }
}

/// A JavaScript object.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct JsObject(raw::Local);

impl Value for JsObject {}

impl Managed for JsObject {
    fn to_raw(self) -> raw::Local {
        self.0
    }

    fn from_raw(_: Env, h: raw::Local) -> Self {
        JsObject(h)
    }
}

unsafe impl This for JsObject {
    #[cfg(feature = "legacy-runtime")]
    fn as_this(h: raw::Local) -> Self {
        JsObject(h)
    }

    #[cfg(feature = "napi-1")]
    fn as_this(_env: Env, h: raw::Local) -> Self {
        JsObject(h)
    }
}

impl ValueInternal for JsObject {
    fn name() -> String {
        "object".to_string()
    }

    fn is_typeof<Other: Value>(env: Env, other: Other) -> bool {
        unsafe { neon_runtime::tag::is_object(env.to_raw(), other.to_raw()) }
    }
}

impl Object for JsObject {}

impl JsObject {
    pub fn new<'a, C: Context<'a>>(c: &mut C) -> Handle<'a, JsObject> {
        JsObject::new_internal(c.env())
    }

    pub(crate) fn new_internal<'a>(env: Env) -> Handle<'a, JsObject> {
        JsObject::build(|out| unsafe { neon_runtime::object::new(out, env.to_raw()) })
    }

    pub(crate) fn build<'a, F: FnOnce(&mut raw::Local)>(init: F) -> Handle<'a, JsObject> {
        unsafe {
            let mut local: raw::Local = std::mem::zeroed();
            init(&mut local);
            Handle::new_internal(JsObject(local))
        }
    }
}

/// A JavaScript array object, i.e. a value for which `Array.isArray`
/// would return `true`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct JsArray(raw::Local);

impl JsArray {
    pub fn new<'a, C: Context<'a>>(cx: &mut C, len: u32) -> Handle<'a, JsArray> {
        JsArray::new_internal(cx.env(), len)
    }

    pub(crate) fn new_internal<'a>(env: Env, len: u32) -> Handle<'a, JsArray> {
        unsafe {
            let mut local: raw::Local = std::mem::zeroed();
            neon_runtime::array::new(&mut local, env.to_raw(), len);
            Handle::new_internal(JsArray(local))
        }
    }

    pub fn to_vec<'a, C: Context<'a>>(self, cx: &mut C) -> NeonResult<Vec<Handle<'a, JsValue>>> {
        let mut result = Vec::with_capacity(self.len_inner(cx.env()) as usize);
        let mut i = 0;
        loop {
            // Since getting a property can trigger arbitrary code,
            // we have to re-check the length on every iteration.
            if i >= self.len_inner(cx.env()) {
                return Ok(result);
            }
            result.push(self.get(cx, i)?);
            i += 1;
        }
    }

    fn len_inner(self, env: Env) -> u32 {
        unsafe { neon_runtime::array::len(env.to_raw(), self.to_raw()) }
    }

    #[cfg(feature = "legacy-runtime")]
    pub fn len(self) -> u32 {
        self.len_inner(Env::current())
    }

    #[cfg(feature = "napi-1")]
    pub fn len<'a, C: Context<'a>>(self, cx: &mut C) -> u32 {
        self.len_inner(cx.env())
    }

    #[cfg(feature = "legacy-runtime")]
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    #[cfg(feature = "napi-1")]
    pub fn is_empty<'a, C: Context<'a>>(self, cx: &mut C) -> bool {
        self.len(cx) == 0
    }
}

impl Value for JsArray {}

impl Managed for JsArray {
    fn to_raw(self) -> raw::Local {
        self.0
    }

    fn from_raw(_: Env, h: raw::Local) -> Self {
        JsArray(h)
    }
}

impl ValueInternal for JsArray {
    fn name() -> String {
        "Array".to_string()
    }

    fn is_typeof<Other: Value>(env: Env, other: Other) -> bool {
        unsafe { neon_runtime::tag::is_array(env.to_raw(), other.to_raw()) }
    }
}

impl Object for JsArray {}

/// A JavaScript function object.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct JsFunction<T: Object = JsObject> {
    raw: raw::Local,
    marker: PhantomData<T>,
}

impl<T: Object> Object for JsFunction<T> {}

// Maximum number of function arguments in V8.
const V8_ARGC_LIMIT: usize = 65535;

unsafe fn prepare_call<'a, 'b, C: Context<'a>, A>(
    cx: &mut C,
    args: &mut [Handle<'b, A>],
) -> NeonResult<(i32, *mut c_void)>
where
    A: Value + 'b,
{
    let argv = args.as_mut_ptr();
    let argc = args.len();
    if argc > V8_ARGC_LIMIT {
        return cx.throw_range_error("too many arguments");
    }
    Ok((argc as i32, argv as *mut c_void))
}

impl JsFunction {
    pub fn new<'a, C, U>(
        cx: &mut C,
        f: fn(FunctionContext) -> JsResult<U>,
    ) -> JsResult<'a, JsFunction>
    where
        C: Context<'a>,
        U: Value,
    {
        build(cx.env(), |out| {
            let env = cx.env().to_raw();
            unsafe {
                let callback = FunctionCallback(f).into_c_callback();
                neon_runtime::fun::new(out, env, callback)
            }
        })
    }
}

impl JsFunction {
    /// Build a [`Call`](crate::types::Call) with an initial arguments list.
    pub fn args<'a, A: Arguments<'a>>(self, args: A) -> Call<'a> {
        let builder = Call {
            callee: Handle::new_internal(self),
            args: vec![],
        };
        builder.args(args)
    }

    /// Build a [`Call`](crate::types::Call) with an initial single argument.
    pub fn arg<'a, V: Value>(self, v: Handle<'a, V>) -> Call<'a> {
        let builder = Call {
            callee: Handle::new_internal(self),
            args: vec![],
        };
        builder.arg(v)
    }

    /// Build a [`FunctionCall`](crate::types::FunctionCall) with a `this` binding.
    pub fn this<'a, V: Value>(self, this: Handle<'a, V>) -> FunctionCall<'a> {
        FunctionCall {
            callee: Handle::new_internal(self),
            this: this.upcast(),
            args: vec![],
        }
    }
}

impl<CL: Object> JsFunction<CL> {
    pub fn call<'a, 'b, C: Context<'a>, T, A, AS>(
        self,
        cx: &mut C,
        this: Handle<'b, T>,
        args: AS,
    ) -> JsResult<'a, JsValue>
    where
        T: Value,
        A: Value + 'b,
        AS: IntoIterator<Item = Handle<'b, A>>,
    {
        let mut args = args.into_iter().collect::<SmallVec<[_; 8]>>();
        let (argc, argv) = unsafe { prepare_call(cx, &mut args) }?;
        let env = cx.env().to_raw();
        build(cx.env(), |out| unsafe {
            neon_runtime::fun::call(out, env, self.to_raw(), this.to_raw(), argc, argv)
        })
    }

    pub fn construct<'a, 'b, C: Context<'a>, A, AS>(self, cx: &mut C, args: AS) -> JsResult<'a, CL>
    where
        A: Value + 'b,
        AS: IntoIterator<Item = Handle<'b, A>>,
    {
        let mut args = args.into_iter().collect::<SmallVec<[_; 8]>>();
        let (argc, argv) = unsafe { prepare_call(cx, &mut args) }?;
        let env = cx.env().to_raw();
        build(cx.env(), |out| unsafe {
            neon_runtime::fun::construct(out, env, self.to_raw(), argc, argv)
        })
    }
}

impl<T: Object> Value for JsFunction<T> {}

impl<T: Object> Managed for JsFunction<T> {
    fn to_raw(self) -> raw::Local {
        self.raw
    }

    fn from_raw(_: Env, h: raw::Local) -> Self {
        JsFunction {
            raw: h,
            marker: PhantomData,
        }
    }
}

impl<T: Object> ValueInternal for JsFunction<T> {
    fn name() -> String {
        "function".to_string()
    }

    fn is_typeof<Other: Value>(env: Env, other: Other) -> bool {
        unsafe { neon_runtime::tag::is_function(env.to_raw(), other.to_raw()) }
    }
}

/// A builder for making a JavaScript function call (e.g., `parseInt("42")`).
///
/// The builder methods make it convenient to assemble the call from parts:
/// ```
/// # use neon::prelude::*;
/// # fn foo(mut cx: FunctionContext) -> JsResult<JsNumber> {
/// # let global = cx.global();
/// # let parseInt = global.get(&mut cx, "parseInt")?;
/// # let parseInt: Handle<JsFunction> = parseInt.downcast_or_throw(&mut cx)?;
/// let x: Handle<JsNumber> = parseInt
///     .arg(cx.string("42"))
///     .call(&mut cx)?;
/// # Ok(x)
/// # }
/// ```
#[derive(Clone)]
pub struct FunctionCall<'a> {
    callee: Handle<'a, JsFunction>,
    this: Handle<'a, JsValue>,
    args: Vec<Handle<'a, JsValue>>,
}

/// A builder for making either a JavaScript function call (e.g., `parseInt("42")`)
/// or constructor call (e.g., `new Array(16)`).
///
/// The builder methods make it convenient to assemble the call from parts:
/// ```
/// # use neon::prelude::*;
/// # fn foo(mut cx: FunctionContext) -> JsResult<JsObject> {
/// # let global = cx.global();
/// # let url = global.get(&mut cx, "URL")?;
/// # let url: Handle<JsFunction> = url.downcast_or_throw(&mut cx)?;
/// let obj = url
///     .arg(cx.string("https://neon-bindings.com"))
///     .new(&mut cx)?;
/// # Ok(obj)
/// # }
/// ```
#[derive(Clone)]
pub struct Call<'a> {
    callee: Handle<'a, JsFunction>,
    args: Vec<Handle<'a, JsValue>>,
}

impl<'a> FunctionCall<'a> {
    /// Set the value of `this` for the function call.
    pub fn this<V: Value>(mut self, this: Handle<'a, V>) -> Self {
        self.this = this.upcast();
        self
    }

    /// Add an argument to the arguments list.
    pub fn arg<V: Value>(mut self, arg: Handle<'a, V>) -> Self {
        self.args.push(arg.upcast());
        self
    }

    /// Add multiple arguments to the arguments list.
    pub fn args<A: Arguments<'a>>(mut self, args: A) -> Self {
        args.append(&mut self.args);
        self
    }

    /// Make the function call. If the function returns without throwing, the result value
    /// is downcast to the type `V`, throwing a `TypeError` if the downcast fails.
    pub fn call<'b, C: Context<'b>, V: Value>(self, cx: &mut C) -> JsResult<'b, V> {
        let v = self.callee.call(cx, self.this, self.args)?;
        v.downcast_or_throw(cx)
    }

    /// Make the function call for side effect, discarding the result value. This method is
    /// preferable to [`call()`](crate::types::FunctionCall::call) when the result value is
    /// not needed, since it does not require specifying a result type.
    pub fn exec<'b, C: Context<'b>>(self, cx: &mut C) -> NeonResult<()> {
        let _ = self.callee.call(cx, self.this, self.args)?;
        Ok(())
    }
}

impl<'a> Call<'a> {
    /// Set the value of `this` for the function call. Once a call has a `this` binding
    /// specified, it is required to be a [`FunctionCall`](crate::types::FunctionCall).
    pub fn this<V: Value>(self, this: Handle<'a, V>) -> FunctionCall<'a> {
        FunctionCall {
            callee: self.callee,
            this: this.upcast(),
            args: self.args,
        }
    }

    /// Add an argument to the arguments list.
    pub fn arg<V: Value>(mut self, arg: Handle<'a, V>) -> Self {
        self.args.push(arg.upcast());
        self
    }

    /// Add multiple arguments to the arguments list.
    pub fn args<A: Arguments<'a>>(mut self, args: A) -> Self {
        args.append(&mut self.args);
        self
    }

    /// Call the function as a constructor (like a JavaScript `new` expression).
    /// If the function returns without throwing, returns the resulting object.
    pub fn new<'b, C: Context<'b>>(self, cx: &mut C) -> JsResult<'b, JsObject> {
        self.callee.construct(cx, self.args)
    }

    /// Make the function call. If the function returns without throwing, the result value
    /// is downcast to the type `V`, throwing a `TypeError` if the downcast fails.
    pub fn call<'b, C: Context<'b>, V: Value>(self, cx: &mut C) -> JsResult<'b, V> {
        let undefined: Handle<JsValue> = cx.undefined().upcast();
        let v = self.callee.call(cx, undefined, self.args)?;
        v.downcast_or_throw(cx)
    }

    /// Make the function call for side effect, discarding the result value. This method is
    /// preferable to [`call()`](crate::types::Call::call) when the result value is not
    /// needed, since it does not require specifying a result type.
    pub fn exec<'b, C: Context<'b>>(self, cx: &mut C) -> NeonResult<()> {
        let undefined: Handle<JsValue> = cx.undefined().upcast();
        let _ = self.callee.call(cx, undefined, self.args)?;
        Ok(())
    }
}

/// The trait for specifying arguments in a [`Call`](crate::types::Call) or
/// [`FunctionCall`](crate::types::FunctionCall).
pub trait Arguments<'a> {
    /// Append the arguments to an arguments vector.
    fn append(self, args: &mut Vec<Handle<'a, JsValue>>);
}

macro_rules! impl_arguments {
    { (); (); } => {
        impl<'a> Arguments<'a> for () {
            fn append(self, _args: &mut Vec<Handle<'a, JsValue>>) { }
        }
    };

    { ($tname1:ident,$($tnames:ident,)*); ($vname1:ident,$($vnames:ident,)*); } => {
        impl<'a, $tname1: Value, $($tnames: Value,)*> Arguments<'a> for (Handle<'a, $tname1>, $(Handle<'a, $tnames>,)*) {
            fn append(self, args: &mut Vec<Handle<'a, JsValue>>) {
                let ($vname1, $($vnames,)*) = self;
                args.push($vname1.upcast());
                $(args.push($vnames.upcast());)*
            }
        }

        impl_arguments! {
            ($($tnames,)*);
            ($($vnames,)*);
        }
    };
}

impl_arguments! {
    (V1, V2, V3, V4, V5, V6, V7, V8,
     V9, V10, V11, V12, V13, V14, V15, V16,
     V17, V18, V19, V20, V21, V22, V23, V24,
     V25, V26, V27, V28, V29, V30, V31, V32,);

    (v1, v2, v3, v4, v5, v6, v7, v8,
     v9, v10, v11, v12, v13, v14, v15, v16,
     v17, v18, v19, v20, v21, v22, v23, v24,
     v25, v26, v27, v28, v29, v30, v31, v32,);
}
