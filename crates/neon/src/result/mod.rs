//! Represents JavaScript exceptions as a Rust [`Result`](std::result) type.
//!
//! Most interactions with the JavaScript engine can throw a JavaScript exception. Neon APIs
//! that can throw an exception are called _throwing APIs_ and return the type
//! [`NeonResult`] (or its shorthand [`JsResult`]).
//!
//! When a throwing API triggers a JavaScript exception, it returns an [Err]
//! result. This indicates that the thread associated with the [`Context`]
//! is now throwing, and allows Rust code to perform any cleanup. See the
//! [`neon::context`](crate::context) module documentation for more about
//! [contexts and exceptions](crate::context#throwing-exceptions).
//!
//! Typically, Neon code can manage JavaScript exceptions correctly and conveniently by
//! using Rust's [question mark (`?`)][question-mark] operator. This ensures that Rust code
//! "short-circuits" when an exception is thrown and returns back to JavaScript without
//! calling any throwing APIs.
//!
//! ## Example
//!
//! Neon functions typically use [`JsResult`] for their return type. This
//! example defines a function that extracts a property called `"message"` from an object,
//! throwing an exception if the argument is not of the right type or extracting the property
//! fails:
//!
//! ```
//! # use neon::prelude::*;
//! fn get_message(mut cx: FunctionContext) -> JsResult<JsValue> {
//!     let obj: Handle<JsObject> = cx.argument(0)?;
//!     obj.prop(&mut cx, "message").get()
//! }
//! ```
//!
//! [question-mark]: https://doc.rust-lang.org/edition-guide/rust-2018/error-handling-and-panics/the-question-mark-operator-for-easier-error-handling.html

use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    marker::PhantomData,
};

use crate::{context::Context, handle::Handle, types::Value};

/// A [unit type][unit] indicating that the JavaScript thread is throwing an exception.
///
/// `Throw` deliberately does not implement [`std::error::Error`]. It's
/// not recommended to chain JavaScript exceptions with other kinds of Rust errors,
/// since throwing means that the JavaScript thread is unavailable until the exception
/// is handled.
///
/// [unit]: https://doc.rust-lang.org/book/ch05-01-defining-structs.html#unit-like-structs-without-any-fields
#[derive(Debug)]
pub struct Throw(PhantomData<*mut ()>); // *mut is !Send + !Sync, making it harder to accidentally store

impl Throw {
    #[cfg(feature = "sys")]
    /// Creates a `Throw` struct representing a JavaScript exception
    /// state.
    ///
    /// # Safety
    ///
    /// `Throw` should *only* be constructed when the JavaScript VM is in a
    /// throwing state. I.e., when [`Status::PendingException`](crate::sys::bindings::Status::PendingException)
    /// is returned.
    pub unsafe fn new() -> Self {
        Self(PhantomData)
    }

    #[cfg(not(feature = "sys"))]
    pub(crate) unsafe fn new() -> Self {
        Self(PhantomData)
    }
}

impl Display for Throw {
    fn fmt(&self, fmt: &mut Formatter) -> FmtResult {
        fmt.write_str("JavaScript Error")
    }
}

/// The result type for throwing APIs.
pub type NeonResult<T> = Result<T, Throw>;

/// Shorthand for a [`NeonResult`] that produces JavaScript values.
pub type JsResult<'b, T> = NeonResult<Handle<'b, T>>;

/// Extension trait for converting Rust [`Result`] values
/// into [`NeonResult`] values by throwing JavaScript exceptions.
pub trait ResultExt<T> {
    fn or_throw<'a, C: Context<'a>>(self, cx: &mut C) -> NeonResult<T>;
}

impl<'a, 'b, T, E> ResultExt<Handle<'a, T>> for Result<Handle<'a, T>, Handle<'b, E>>
where
    T: Value,
    E: Value,
{
    fn or_throw<'cx, C: Context<'cx>>(self, cx: &mut C) -> JsResult<'a, T> {
        self.or_else(|err| cx.throw(err))
    }
}
