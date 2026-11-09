import { describe, expect, it } from "vitest";
import { credentialFromLocation } from "./credential";

describe("musician access links", () => {
  it("reads a credential from the URL fragment", () => {
    expect(
      credentialFromLocation({
        hash: "#credential=serum-playground",
      }),
    ).toBe("serum-playground");
  });

  it("does not treat an ordinary fragment as a credential", () => {
    expect(
      credentialFromLocation({
        hash: "#serum",
      }),
    ).toBe("");
  });
});
