import { describe, expect, it } from "vitest";
import { joinArgs, splitArgs } from "./shellwords";

describe("splitArgs", () => {
  it("separa por espacios y respeta las comillas", () => {
    expect(splitArgs(`nginx -g 'daemon off;'`)).toEqual({ ok: true, args: ["nginx", "-g", "daemon off;"] });
    expect(splitArgs(`sh -c "echo \\"hola\\" $HOME"`)).toEqual({ ok: true, args: ["sh", "-c", `echo "hola" $HOME`] });
    expect(splitArgs(`a\\ b  ''  c`)).toEqual({ ok: true, args: ["a b", "", "c"] });
    expect(splitArgs("   ")).toEqual({ ok: true, args: [] });
  });
  it("informa de comillas sin cerrar", () => {
    expect(splitArgs(`echo 'hola`)).toEqual({ ok: false, error: "unclosedQuote" });
    expect(splitArgs(`echo "hola`)).toEqual({ ok: false, error: "unclosedQuote" });
    expect(splitArgs(`echo \\`)).toEqual({ ok: false, error: "trailingEscape" });
  });
  it("joinArgs es el inverso", () => {
    const args = ["nginx", "-g", "daemon off;", "it's", "", "a=b"];
    const line = joinArgs(args);
    expect(line).toBe(`nginx -g 'daemon off;' 'it'\\''s' '' a=b`);
    expect(splitArgs(line)).toEqual({ ok: true, args });
  });
});
