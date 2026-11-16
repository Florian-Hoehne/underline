# Dictionaries of the spell check (ADR-030)

Hunspell dictionaries, read at run time by `underline-spell` (spellbook). The
files are as published; only the `.aff` and `.dic` files are renamed to the
language code. Each folder holds the license text of its dictionary. Only
permissively licensed dictionaries are bundled; German and Italian (GPL) wait
for a license decision (OQ-299).

| Folder | Dictionary | Version | Source | License |
|---|---|---|---|---|
| `en-US` | SCOWL `en_US` | 2020.12.07 | `hunspell-en_US-2020.12.07.zip` from https://sourceforge.net/projects/wordlist/files/speller/2020.12.07/ (sha256 `616348ad645a716d91c8a6645065e710f15e9dda3ffef60cdf7ec8a4e27975af`) | SCOWL (permissive, Kevin Atkinson et al.) and BSD (Ispell affix file); see `README_en_US.txt` |
| `en-GB` | SCOWL `en_GB-ise` | 2020.12.07 | `hunspell-en_GB-ise-2020.12.07.zip`, same place (sha256 `2ced4b2ec9d188c3b540dbb75b5143f65b990daf17437758726ff1fab5f01ad3`) | as `en-US`; see `README_en_GB-ise.txt` |
| `fr-FR` | Grammalecte / dicollecte "Classique" | 7.7 | https://github.com/LibreOffice/dictionaries, `fr_FR/dictionaries/fr.aff`, `fr.dic` (commit `fe691977c619b2ca764531b3507e83fab4732651`) | MPL-2.0; see `README_dict_fr.txt`, `MPL-2.0.txt` |
| `es-ES` | RLA-ES `es_ES` | 2.9 | https://github.com/LibreOffice/dictionaries, `es/es_ES.aff`, `es_ES.dic` (commit `762abe74008b94b2ff06db6f4024b59a8254c467`) | GPL-3.0+ / LGPL-3.0+ / MPL-1.1+, used under MPL-1.1; see `LICENSE.md`, `MPL-1.1.txt` |

The source of the MPL dictionaries is the files themselves; changes to them
would be published under the same license.
