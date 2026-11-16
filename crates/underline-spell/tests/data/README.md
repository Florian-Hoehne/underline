Small dictionaries for the tests of `underline-spell` (ADR-030), written for
them; they are no real language and come with no license of their own (MIT,
as the project).

- `xx-TEST`: UTF-8, a few English words with a suffix rule.
- `xx-LATIN`: ISO 8859-1, to test decoding when a dictionary is read.
- `xx-BROKEN`: says UTF-8 but holds bytes that are not, to test the error.
