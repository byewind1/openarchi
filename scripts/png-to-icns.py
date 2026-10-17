#!/usr/bin/env python3
"""Pack a standard macOS iconset's PNG images into an ICNS container."""

from __future__ import annotations

import struct
import sys
from pathlib import Path


SLOTS = (
    ("ic11", "icon_16x16@2x.png", 32),
    ("ic12", "icon_32x32@2x.png", 64),
    ("ic07", "icon_128x128.png", 128),
    ("ic13", "icon_128x128@2x.png", 256),
    ("ic08", "icon_256x256.png", 256),
    ("ic14", "icon_256x256@2x.png", 512),
    ("ic09", "icon_512x512.png", 512),
    ("ic10", "icon_512x512@2x.png", 1024),
)


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: png-to-icns.py ICONSET_DIRECTORY OUTPUT.icns")

    iconset = Path(sys.argv[1])
    output = Path(sys.argv[2])
    elements = bytearray()
    for element_type, filename, size in SLOTS:
        image = (iconset / filename).read_bytes()
        if image[:8] != b"\x89PNG\r\n\x1a\n" or image[12:16] != b"IHDR":
            raise SystemExit(f"{filename} is not a valid PNG")
        width, height = struct.unpack(">II", image[16:24])
        if (width, height) != (size, size):
            raise SystemExit(f"{filename} must be {size}x{size}, got {width}x{height}")
        elements.extend(element_type.encode("ascii"))
        elements.extend(struct.pack(">I", len(image) + 8))
        elements.extend(image)

    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(b"icns" + struct.pack(">I", len(elements) + 8) + elements)


if __name__ == "__main__":
    main()
