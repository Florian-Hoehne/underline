# Dictionaries of the spell check (ADR-030)

Hunspell dictionaries, read at run time by `underline-spell` (spellbook). The
files are as published; only the `.aff` and `.dic` files are renamed to the
language code. Each folder holds the license text of its dictionary. The
German and Italian dictionaries are licensed under the GPL; they are shipped
unmodified, as separate data files next to the MIT-licensed program (mere
aggregation, GPLv2 §2, GPLv3 §5; OQ-299, ADR-030 update 2026-11-16).

| Folder | Dictionary | Version | Source | License |
|---|---|---|---|---|
| `en-US` | SCOWL `en_US` | 2020.12.07 | `hunspell-en_US-2020.12.07.zip` from https://sourceforge.net/projects/wordlist/files/speller/2020.12.07/ (sha256 `616348ad645a716d91c8a6645065e710f15e9dda3ffef60cdf7ec8a4e27975af`) | SCOWL (permissive, Kevin Atkinson et al.) and BSD (Ispell affix file); see `README_en_US.txt` |
| `en-GB` | SCOWL `en_GB-ise` | 2020.12.07 | `hunspell-en_GB-ise-2020.12.07.zip`, same place (sha256 `2ced4b2ec9d188c3b540dbb75b5143f65b990daf17437758726ff1fab5f01ad3`) | as `en-US`; see `README_en_GB-ise.txt` |
| `fr-FR` | Grammalecte / dicollecte "Classique" | 7.7 | https://github.com/LibreOffice/dictionaries, `fr_FR/dictionaries/fr.aff`, `fr.dic` (commit `fe691977c619b2ca764531b3507e83fab4732651`) | MPL-2.0; see `README_dict_fr.txt`, `MPL-2.0.txt` |
| `es-ES` | RLA-ES `es_ES` | 2.9 | https://github.com/LibreOffice/dictionaries, `es/es_ES.aff`, `es_ES.dic` (commit `762abe74008b94b2ff06db6f4024b59a8254c467`) | GPL-3.0+ / LGPL-3.0+ / MPL-1.1+, used under MPL-1.1; see `LICENSE.md`, `MPL-1.1.txt` |
| `de-DE` | igerman98 + frami `de_DE_frami` | igerman98 2016-12-07 + frami 2017-01-09 (`# Version: 20161207+frami20170109` in the `.aff`; extension 2017.01.12) | https://github.com/LibreOffice/dictionaries, `de/de_DE_frami.aff`, `de_DE_frami.dic` (commit `c0865a85eb67dc377f2cf8e8fe2c4fa9e53f0dcb`); sha256 `.aff` `646bf3333ac69c23e9d794533ee5241d6f755c359e8fe10a648f87613743d594`, `.dic` `4ca3c958b0e5545910999bc246f668840bf8ede3df8e5e6790d05edd5a586c38` | GPL-2.0 or GPL-3.0 (the `.aff` and `README_de_DE_frami.txt`); see `COPYING_GPLv2`, `COPYING_GPLv3`, `README_de_DE_frami.txt`, `README_extension_owner.txt` |
| `de-CH` | igerman98 + frami `de_CH_frami` | as `de-DE` | same place, `de/de_CH_frami.aff`, `de_CH_frami.dic` (same commit); sha256 `.aff` `d86884be4c3f07e5e08057884856feabf2872b4e2f90a587a11376082bc4126b`, `.dic` `80a4df38cdc3e16d2f042f8eba4a99668ab9c075662e4262c62f8bba46c7bd16` | as `de-DE` |
| `it-IT` | Italian Writing Aids (LibreItalia) `it_IT` | 5.1.1 (2022-11-07; `.aff` header 5.1.0) | same place, `it_IT/it_IT.aff`, `it_IT.dic` (same commit); sha256 `.aff` `951afaa19272f13555b8823e8bcf9ccf78f8fe1a07835bdfb912ab3e4d537c2b`, `.dic` `bae1e3501dcd2a923669592493b3fde6c02aae7c7aab83bf5e5b49077e73dd64` | GPL-3.0 (`README_it_IT.txt`, which holds the license text at its end); see also `COPYING_GPLv3`, `CHANGELOG.txt` |

The source of the MPL dictionaries is the files themselves; changes to them
would be published under the same license.

## Source of the GPL dictionaries (GPLv2 §3, GPLv3 §6)

The `.aff` and `.dic` files of `de-DE`, `de-CH` and `it-IT` are shipped as
published, in the form LibreOffice maintains them; Underline changes nothing
in them. Their corresponding source (for the German dictionaries also the
igerman98 word lists they are generated from) is available, at no charge,
from:

- the LibreOffice dictionaries repository at the commit above:
  https://github.com/LibreOffice/dictionaries/tree/c0865a85eb67dc377f2cf8e8fe2c4fa9e53f0dcb/de and
  https://github.com/LibreOffice/dictionaries/tree/c0865a85eb67dc377f2cf8e8fe2c4fa9e53f0dcb/it_IT
  (upstream: https://git.libreoffice.org/dictionaries);
- the igerman98 word lists the German dictionaries are built from:
  https://www.j3e.de/ispell/igerman98/dict/igerman98-20161207.tar.bz2.

**Written offer:** the corresponding source of `de-DE`, `de-CH` and `it-IT` is
available for at least three years from each release of Underline that
contains them, on request through
https://github.com/Florian-Hoehne/underline/issues, or from the places above.
Should those places go away, the same commit is in LibreOffice's own
repository, https://git.libreoffice.org/dictionaries, the fallback mirror.
