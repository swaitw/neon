var addon = require("..");
var assert = require("chai").assert;

const STRICT = function () {
  "use strict";
  return this;
};
const SLOPPY = Function("return this;");

function isStrict(f) {
  try {
    f.caller;
    return false;
  } catch (e) {
    return true;
  }
}

describe("JsFunction", function () {
  it("return a JsFunction built in Rust", function () {
    assert.isFunction(addon.return_js_function());
  });

  it("return a JsFunction built in Rust that implements x => x + 1", function () {
    assert.equal(addon.return_js_function()(41), 42);
  });

  it("call a JsFunction built in JS that implements x => x + 1", function () {
    assert.equal(
      addon.call_js_function(function (x) {
        return x + 1;
      }),
      17
    );
  });

  it("call a JsFunction built in JS with call_with", function () {
    assert.equal(
      addon.call_js_function_idiomatically(function (x) {
        return x + 1;
      }),
      17
    );
  });

  it("call a JsFunction built in JS with .bind().apply()", function () {
    assert.equal(
      addon.call_js_function_with_bind(function (a, b, c, d, e) {
        return a * b * c * d * e;
      }),
      1 * 2 * 3 * 4 * 5
    );
  });

  it("call a JsFunction build in JS with .bind and .args_with", function () {
    assert.equal(
      addon.call_js_function_with_bind_and_args_with(function (a, b, c) {
        return a + b + c;
      }),
      1 + 2 + 3
    );
  });

  it("call a JsFunction build in JS with .bind and .args and With", function () {
    assert.equal(
      addon.call_js_function_with_bind_and_args_and_with(function (a, b, c) {
        return a + b + c;
      }),
      1 + 2 + 3
    );
  });

  it("call parseInt with .bind().apply()", function () {
    assert.equal(addon.call_parse_int_with_bind(), 42);
  });

  it("call a JsFunction built in JS with .bind and .exec", function () {
    let local = 41;
    addon.call_js_function_with_bind_and_exec(function (x) {
      local += x;
    });
    assert.equal(local, 42);
  });

  it("call a JsFunction built in JS as a constructor with .bind and .construct", function () {
    function MyClass(number, string) {
      this.number = number;
      this.string = string;
    }

    const obj = addon.call_js_constructor_with_bind(MyClass);

    assert.instanceOf(obj, MyClass);
    assert.equal(obj.number, 42);
    assert.equal(obj.string, "hello");
  });

  it("bind a JsFunction to an object", function () {
    const result = addon.bind_js_function_to_object(function () {
      return this.prop;
    });

    assert.equal(result, 42);
  });

  it("bind a strict JsFunction to a number", function () {
    assert.isTrue(isStrict(STRICT));

    // strict mode functions are allowed to have a primitive this binding
    const result = addon.bind_js_function_to_number(STRICT);

    assert.strictEqual(result, 42);
  });

  it("bind a sloppy JsFunction to a primitive", function () {
    assert.isFalse(isStrict(SLOPPY));

    // legacy JS functions (aka "sloppy mode") replace primitive this bindings
    // with object wrappers, so 42 will get wrapped as new Number(42)
    const result = addon.bind_js_function_to_number(SLOPPY);

    assert.instanceOf(result, Number);
    assert.strictEqual(typeof result, "object");
    assert.strictEqual(result.valueOf(), 42);
  });

  it("call a JsFunction with zero args", function () {
    assert.equal(addon.call_js_function_with_zero_args(), -Infinity);
  });

  it("call a JsFunction with one arg", function () {
    assert.equal(addon.call_js_function_with_one_arg(), 1.0);
  });

  it("call a JsFunction with two args", function () {
    assert.equal(addon.call_js_function_with_two_args(), 2.0);
  });

  it("call a JsFunction with three args", function () {
    assert.equal(addon.call_js_function_with_three_args(), 3.0);
  });

  it("call a JsFunction with four args", function () {
    assert.equal(addon.call_js_function_with_four_args(), 4.0);
  });

  it("call a JsFunction with a custom this", function () {
    assert.equal(
      addon.call_js_function_with_custom_this(function () {
        return this;
      }).secret,
      42
    );
  });

  it("call a JsFunction with the default this", function () {
    addon.call_js_function_with_implicit_this(function () {
      "use strict"; // ensure the undefined this isn't replaced with the global object
      assert.strictEqual(this, undefined);
    });
  });

  it("exec a JsFunction with the default this", function () {
    addon.exec_js_function_with_implicit_this(function () {
      "use strict"; // ensure the undefined this isn't replaced with the global object
      assert.strictEqual(this, undefined);
    });
  });

  it("call a JsFunction with a heterogeneously typed tuple", function () {
    assert.deepEqual(addon.call_js_function_with_heterogeneous_tuple(), [
      1,
      "hello",
      true,
    ]);
  });

  it("new a JsFunction", function () {
    assert.equal(addon.construct_js_function(Date), 1970);
  });

  it("new a JsFunction with construct_with", function () {
    assert.equal(addon.construct_js_function_idiomatically(Date), 1970);
  });

  it("new a JsFunction with construct_with to create an array", function () {
    assert.deepEqual(
      addon.construct_js_function_with_overloaded_result(),
      [1, 2, 3]
    );
  });

  it("got two parameters, a string and a number", function () {
    addon.check_string_and_number("string", 42);
  });

  it("converts a Rust panic to a throw in a function", function () {
    assert.throws(
      function () {
        addon.panic();
      },
      Error,
      /^internal error in Neon module: zomg$/
    );
  });

  it("lets panic override a throw", function () {
    assert.throws(
      function () {
        addon.panic_after_throw();
      },
      Error,
      /^internal error in Neon module: this should override the RangeError$/
    );
  });

  it("computes the right number of arguments", function () {
    assert.equal(addon.num_arguments(), 0);
    assert.equal(addon.num_arguments("a"), 1);
    assert.equal(addon.num_arguments("a", "b"), 2);
    assert.equal(addon.num_arguments("a", "b", "c"), 3);
    assert.equal(addon.num_arguments("a", "b", "c", "d"), 4);
  });

  it("gets the right `this`-value", function () {
    var o = { iamobject: "i am object" };
    assert.equal(addon.return_this.call(o), o);

    var d = new Date();
    assert.equal(addon.return_this.call(d), d);

    var n = 19;
    assert.notStrictEqual(addon.return_this.call(n), n);
  });

  it("can manipulate an object `this` binding", function () {
    var o = { modified: false };
    addon.require_object_this.call(o);
    assert.equal(o.modified, true);
    // Doesn't throw because of implicit primitive wrapping:
    addon.require_object_this.call(42);
  });

  it("implicitly gets global", function () {
    var global = new Function("return this")();
    assert.equal(addon.return_this.call(undefined), global);
  });

  it("exposes an argument via arguments_opt iff it is there", function () {
    assert.equal(addon.is_argument_zero_some(), false);
    assert.equal(addon.is_argument_zero_some("a"), true);
    assert.equal(addon.is_argument_zero_some("a", "b"), true);
    assert.equal(addon.is_argument_zero_some.call(null), false);
    assert.equal(addon.is_argument_zero_some.call(null, ["a"]), true);
    assert.equal(addon.is_argument_zero_some.call(null, ["a", "b"]), true);
  });

  it("correctly casts an argument via cx.arguments", function () {
    assert.equal(addon.require_argument_zero_string("foobar"), "foobar");
    assert.throws(function () {
      addon.require_argument_zero_string(new Date());
    }, TypeError);
    assert.throws(function () {
      addon.require_argument_zero_string(17);
    }, TypeError);
  });

  it("executes a scoped computation", function () {
    assert.equal(addon.execute_scoped(), 99);
  });

  it("computes a value in a scoped computation", function () {
    const o = {};

    assert.equal(addon.compute_scoped(), 99);
    assert.equal(addon.recompute_scoped(o), o);
  });

  it("catches an exception with cx.try_catch", function () {
    var error = new Error("Something bad happened");
    assert.equal(addon.throw_and_catch(error), error);
    assert.equal(addon.throw_and_catch(42), 42);
    assert.equal(addon.throw_and_catch("a string"), "a string");
    assert.equal(
      addon.call_and_catch(() => {
        throw "shade";
      }),
      "shade"
    );
    assert.equal(
      addon.call_and_catch(() => {
        throw (
          addon.call_and_catch(() => {
            throw (
              addon.call_and_catch(() => {
                throw "once";
              }) + " upon"
            );
          }) + " a"
        );
      }) + " time",
      "once upon a time"
    );
  });

  it("gets a regular value with cx.try_catch", function () {
    assert.equal(
      addon.call_and_catch(() => {
        return 42;
      }),
      42
    );
  });

  it("can return Rust type from cx.try_catch", function () {
    const n = Math.random();
    assert.strictEqual(addon.get_number_or_default(n), n);
    assert.strictEqual(addon.get_number_or_default(), 0);
  });

  it("always provides an object for the this-binding", function () {
    var meta1 = addon.assume_this_is_an_object.call(null);
    assert.strictEqual(meta1.prototype, Object.getPrototypeOf(global));
    assert.strictEqual(meta1.hasOwn, false);
    assert.strictEqual(meta1.property, Object.getPrototypeOf(global).toString);

    var meta2 = addon.assume_this_is_an_object.call(42);
    assert.strictEqual(meta2.prototype, Number.prototype);
    assert.strictEqual(meta2.hasOwn, false);
    assert.strictEqual(meta2.property, Number.prototype.toString);

    var meta3 = addon.assume_this_is_an_object.call(Object.create(null));
    assert.strictEqual(meta3.prototype, null);
    assert.strictEqual(meta3.hasOwn, false);
    assert.strictEqual(meta3.property, undefined);

    var meta4 = addon.assume_this_is_an_object.call({ toString: 17 });
    assert.strictEqual(meta4.prototype, Object.prototype);
    assert.strictEqual(meta4.hasOwn, true);
    assert.strictEqual(meta4.property, 17);
  });

  it("distinguishes calls from constructs", function () {
    assert.equal(addon.is_construct.call({}).wasConstructed, false);
    assert.equal(new addon.is_construct().wasConstructed, true);
  });

  it("should be able to call a function from a closure", function () {
    assert.strictEqual(addon.count_called() + 1, addon.count_called());
  });

  (global.gc ? it : it.skip)(
    "should drop function when going out of scope",
    function (cb) {
      // Run from an `IIFE` to ensure that `f` is out of scope and eligible for garbage
      // collection when `global.gc()` is executed.
      (() => {
        const msg = "Hello, World!";
        const f = addon.caller_with_drop_callback(() => msg, cb);

        assert.strictEqual(f(), msg);
      })();

      global.gc();
    }
  );
});
