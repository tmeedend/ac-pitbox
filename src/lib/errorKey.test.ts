import { describe, expect, it } from "vitest";
import { parseErrorKey } from "./errorKey";

describe("parseErrorKey", () => {
  it("reads a bare key, as every error but one is written", () => {
    expect(parseErrorKey("errors.gameRunning")).toEqual({ key: "errors.gameRunning", values: {} });
  });

  /** What `errors::with_values` writes: the missing car must be named, or
   * the message tells nobody what to reinstall. */
  it("splits a key from the values of its placeholders", () => {
    expect(parseErrorKey('errors.carModelsMissing{"name":"vrc_arc_auriel90"}')).toEqual({
      key: "errors.carModelsMissing",
      values: { name: "vrc_arc_auriel90" },
    });
  });

  it("leaves a raw diagnostic alone", () => {
    expect(parseErrorKey("suppression du déploiement : accès refusé")).toBeNull();
  });

  /** A broken suffix must not hide the sentence: the key still translates. */
  it("keeps the key when the values cannot be read", () => {
    expect(parseErrorKey("errors.carModelsMissing{oops")).toEqual({ key: "errors.carModelsMissing", values: {} });
  });
});
