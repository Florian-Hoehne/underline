# Vendored PlantUML engine (ADR-009)

`plantuml.js`, `themes.js` and `LICENSE` are taken unchanged from the npm package `@plantuml/core` 1.2026.9
(integrity `sha512-Qy46og9hEb+wOFtjUfniWkl/BDwHpDuebQX5rF+35s9PS2G4cKZf1iWw4E/ELLQ0pTkESPDoQTbKWcc3ZyVcVA==`),
the official PlantUML build compiled to JavaScript with TeaVM, MIT licensed.

To update: download the new version of the package, check its license, replace the three files, change the version
here and in `src/diagram/plantuml.rs`, and run `cargo test -p underline-facade`.
