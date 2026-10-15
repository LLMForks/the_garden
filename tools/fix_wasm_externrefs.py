#!/usr/bin/env python3
"""Points the `__wbindgen_externrefs` export at the externref table.

With some Rust/wasm-bindgen combinations wasm-bindgen exports the funcref
(indirect call) table under that name, and the page fails at startup with
"WebAssembly.Table.grow(): failed to grow table". If the export is already
correct this script changes nothing.

    python3 tools/fix_wasm_externrefs.py dist/web/the_garden_bg.wasm
"""
import sys

FUNCREF, EXTERNREF = 0x70, 0x6F
NAME = "__wbindgen_externrefs"


def leb(b, i):
    r = s = 0
    while True:
        x = b[i]
        i += 1
        r |= (x & 0x7F) << s
        s += 7
        if x < 0x80:
            return r, i


def enc(n):
    out = bytearray()
    while True:
        x = n & 0x7F
        n >>= 7
        if n:
            out.append(x | 0x80)
        else:
            out.append(x)
            return bytes(out)


def main(path):
    d = open(path, "rb").read()
    assert d[:4] == b"\0asm", "not a wasm module"
    sections = []
    i = 8
    while i < len(d):
        sid = d[i]
        size, j = leb(d, i + 1)
        sections.append((sid, d[j:j + size]))
        i = j + size
    # table types in index space order: imported tables first, then defined ones
    tables = []
    for sid, body in sections:
        if sid == 2:
            n, j = leb(body, 0)
            for _ in range(n):
                l, j = leb(body, j)
                j += l
                l, j = leb(body, j)
                j += l
                kind = body[j]
                j += 1
                if kind == 0:
                    _, j = leb(body, j)
                elif kind == 1:
                    tables.append(body[j])
                    fl = body[j + 1]
                    j += 2
                    _, j = leb(body, j)
                    if fl & 1:
                        _, j = leb(body, j)
                elif kind == 2:
                    fl = body[j]
                    j += 1
                    _, j = leb(body, j)
                    if fl & 1:
                        _, j = leb(body, j)
                elif kind == 3:
                    j += 2
    for sid, body in sections:
        if sid == 4:
            n, j = leb(body, 0)
            for _ in range(n):
                tables.append(body[j])
                fl = body[j + 1]
                j += 2
                _, j = leb(body, j)
                if fl & 1:
                    _, j = leb(body, j)
    ext = [k for k, t in enumerate(tables) if t == EXTERNREF]
    out_sections = []
    changed = False
    for sid, body in sections:
        if sid == 7 and ext:
            n, j = leb(body, 0)
            entries = []
            for _ in range(n):
                l, j2 = leb(body, j)
                name = body[j2:j2 + l]
                j2 += l
                kind = body[j2]
                idx, j3 = leb(body, j2 + 1)
                if kind == 1 and name.decode() == NAME and tables[idx] != EXTERNREF:
                    print(f"{NAME}: table {idx} is not externref, using table {ext[0]}")
                    idx = ext[0]
                    changed = True
                entries.append(enc(len(name)) + name + bytes([kind]) + enc(idx))
                j = j3
            body = enc(n) + b"".join(entries)
        out_sections.append((sid, body))
    if not changed:
        print(f"{NAME}: already fine")
        return
    out = bytearray(d[:8])
    for sid, body in out_sections:
        out += bytes([sid]) + enc(len(body)) + body
    open(path, "wb").write(out)


if __name__ == "__main__":
    main(sys.argv[1])
