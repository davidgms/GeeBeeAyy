#!/usr/bin/env python3
"""Regenerate the game-code -> No-Intro-name table the cover fetcher uses.

The table is the only part of cover art that ships in the APK. The pictures
never do - see docs/cover-art.md for why that line matters.

Source: libretro-database's No-Intro DAT for GBA, which is CC-BY-SA-4.0. The
DAT's `serial` field is exactly the 4-byte game code at 0xAC in the cart
header, so the join is exact rather than heuristic.

Run it when the DAT gets a new release, roughly once a year:

    python3 tools/gen-cover-index.py

It rewrites android/app/src/main/assets/gba-game-codes.json in place and
prints what changed.
"""

import json
import pathlib
import re
import sys
import urllib.request

DAT_URL = (
    "https://raw.githubusercontent.com/libretro/libretro-database/master/"
    "metadat/no-intro/Nintendo%20-%20Game%20Boy%20Advance.dat"
)
OUT = pathlib.Path(__file__).resolve().parent.parent / (
    "android/app/src/main/assets/gba-game-codes.json"
)


def main() -> int:
    print(f"fetching {DAT_URL}")
    text = urllib.request.urlopen(DAT_URL, timeout=60).read().decode("utf-8", "replace")

    version = re.search(r'\n\tversion "([^"]+)"', text)
    entries = re.findall(r'game \(\s*\n\tname "([^"]+)"(.*?)\n\)', text, re.S)
    if not entries:
        print("no entries parsed - has the DAT format changed?", file=sys.stderr)
        return 1

    # First name wins. Where a code has several, they are revisions of one
    # title (`(USA)` / `(Rev 1)` / `(Virtual Console)`) that share box art.
    table: dict[str, str] = {}
    for name, body in entries:
        serial = re.search(r'\n\tserial "([^"]+)"', body)
        if serial and serial.group(1) not in table:
            table[serial.group(1)] = name

    previous = json.loads(OUT.read_text()) if OUT.is_file() else {}
    OUT.write_text(json.dumps(dict(sorted(table.items())), indent=0, ensure_ascii=False))

    added = table.keys() - previous.keys()
    removed = previous.keys() - table.keys()
    print(f"DAT version {version.group(1) if version else '?'}")
    print(f"{len(entries)} entries, {len(table)} game codes -> {OUT}")
    print(f"{OUT.stat().st_size / 1024:.0f} KB, +{len(added)} codes, -{len(removed)}")
    # A code that lost its entry stops resolving, which is worth seeing.
    for code in sorted(removed):
        print(f"  gone: {code} was {previous[code]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
