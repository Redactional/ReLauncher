# Contributing to ReLauncher

Thanks for helping make ReLauncher better.

## Reporting bugs and ideas

Use the [issue forms](https://github.com/Redactional/ReLauncher/issues/new/choose). Problems inside a program (not installing or launching it) belong in that program's own repository.

## Building and checks

You need [Rust](https://rustup.rs) 1.96 or newer.

```bash
cargo run
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs the same checks on Windows, macOS and Linux.

- Keep pull requests focused: one feature or fix each.
- Logic with branches, parsing or file handling comes with a small test. Tests never touch the real Start Menu, applications folder or settings.
- New dependencies: pin a minor version and say why it's needed.

## Project layout

```
src/
  main.rs      window setup
  app.rs       the UI: library, program page, dialogs
  theme.rs     the shared Re look (colors, cards, buttons)
  catalog.rs   programs.json: the program list and requirements
  github.rs    latest releases from GitHub
  http.rs      downloads
  install.rs   install, update, uninstall, launch, system search entries, requirements
  settings.rs  install folder and install records
programs.json  the published program list (see docs/adding-programs.md)
site/          the website, published to GitHub Pages by .github/workflows/pages.yml
```

## Releases

Versions follow [semantic versioning](https://semver.org).

1. Set the new version in `Cargo.toml` and commit it.
2. Tag the commit and push the tag:
   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```
3. The [Release workflow](.github/workflows/release.yml) builds Windows, macOS and Linux and publishes the release with installers and notes generated from the merged pull requests. Running copies of ReLauncher show an update notice.

A tag with a suffix (`v0.2.0-beta.1`) becomes a pre-release. The workflow refuses to run if the tag and `Cargo.toml` disagree. To rebuild an existing tag, run it from the Actions tab.

Changes to `programs.json` need no release: they reach ReLauncher users and the website as soon as they're on `main`.

## Website

The site in `site/` is plain HTML, CSS and JavaScript. `docs.html` renders each program's manual from its repository (see [docs/adding-programs.md](docs/adding-programs.md#documentation)); `site/vendor/` holds the Markdown renderer ([marked](https://github.com/markedjs/marked), MIT) and sanitizer ([DOMPurify](https://github.com/cure53/DOMPurify), Apache 2.0). It reads `programs.json` and each program's latest release from GitHub in the visitor's browser, so it never needs updating for a new version. To preview it, copy `site/`, `programs.json` and the icons into one folder the way the Website workflow does, and serve that folder (for example `python -m http.server`).

## License of contributions

Unless you explicitly state otherwise, any contribution you intentionally submit for inclusion in this work, as defined in the Apache-2.0 license, shall be dual licensed under MIT or Apache-2.0, without any additional terms or conditions.
