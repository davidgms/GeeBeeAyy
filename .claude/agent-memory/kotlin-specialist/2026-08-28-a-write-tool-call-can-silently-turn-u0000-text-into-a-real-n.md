### 2026-08-28 - A `Write` tool call can silently turn `\u0000` text into a real NUL byte

- **Context**: writing `EmulationViewModel.kt` from scratch with the `Write`
  tool; the source contained the string literal `'\u0000'` (a Kotlin char
  escape, six characters: backslash-u-0-0-0-0).
- **Finding**: the file that landed on disk contained an actual `0x00` byte
  at that position instead of the six-character escape sequence. The
  `Read` tool rendered it back looking identical to what was intended (a
  terminal/log level of NUL usually renders invisibly), so nothing looked
  wrong on inspection - it only surfaced because `grep -n` on the file
  returned nothing at all for patterns that were unambiguously present
  (GNU grep treats a file containing a NUL byte as binary and silently
  changes its matching behaviour rather than erroring). `cat -A` (or
  `python3 -c "open(path,'rb').read().count(b'\x00')"`) is what actually
  exposed it.
- **Application**: after writing or editing a Kotlin/Rust/any-text file that
  contains a `\u0000`-style escape literal, verify with `grep -c` on a known
  substring near it, or directly check for `b'\x00'` in the file's raw bytes,
  rather than trusting a `Read` tool round-trip. If a `grep` that should
  obviously match returns nothing, suspect a binary/NUL contamination before
  suspecting the pattern.
