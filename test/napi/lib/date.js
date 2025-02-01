var addon = require("..");
var assert = require("chai").assert;

describe("JsDate", function () {
  it("should create a date", function () {
    const date = addon.create_date();
    assert.instanceOf(date, Date);
  });

  it("should create date from time", function () {
    const date = addon.create_date_from_value(31415);
    assert.instanceOf(date, Date);
    assert.equal(date.toUTCString(), new Date(31415).toUTCString());
  });

  it("should check if date is valid", function () {
    const dateIsValid = addon.check_date_is_valid(31415);
    assert.isTrue(dateIsValid);
  });

  it("should try new date", function () {
    assert.isUndefined(addon.try_new_date());
  });

  it("should try new lossy date", function () {
    assert.isUndefined(addon.try_new_lossy_date());
  });

  it("should handle nan dates", function () {
    assert.isUndefined(addon.nan_dates());
  });

  it("should check if date is invalid", function () {
    const date = addon.create_and_get_invalid_date();
    assert.isNaN(date);
  });

  it("should get date value", function () {
    const dateValue = addon.get_date_value();
    assert.equal(dateValue, 31415);
  });
});
