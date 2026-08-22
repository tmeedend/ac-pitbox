# Pit Box

A mod manager for **Assetto Corsa**, for Windows.

Pit Box was built with two goals in mind:

- **Keep your Assetto Corsa folder clean**, no matter how many mods you install
  over time.
- **Enjoy your mod library** through a sleek, modern and uncluttered interface.

No two mods are packaged the same way: some overwrite base game files, some ship
a readme with manual steps and optional extras, others are layered on top of
another mod. Pit Box analyses archives, keeps every mod in a library outside the
game, and links it into `content/` on demand — so activating, layering or
removing one is instant and reversible, and you can see exactly which files any
car or track is currently made of.

**A mod's own files are never modified.** `ui_car.json` and `ui_track.json` are
read, never rewritten; tags, categories, custom names and descriptions all live
in a local database beside the files, so what you have on disk stays exactly
what its author published.

**This is not a Content Manager replacement.** CM is an excellent tool, and Pit
Box drives it rather than competing with it: sessions are launched through CM,
and you can open CM directly from inside Pit Box if you would rather use Pit Box
purely as a mod and file manager.

## Download

Windows installer on the [Releases page](https://github.com/tmeedend/ac-pitbox/releases).

Requirements: **Windows 10/11**, **Assetto Corsa**, **[Content Manager](https://acstuff.ru/app/)**
and **7-Zip** for archive extraction.

> **A clean Assetto Corsa install is required, and Pit Box is beta.** Its whole
> job is to track and isolate mod files from day one, so it needs to start from
> a base install it can account for — pointing it at a folder already full of
> hand-installed mods defeats the point, since it cannot know what is already
> there or where it came from. It is stable in daily use, but treat it as beta:
> keep a backup you would be comfortable falling back to.

Installers are **not code-signed yet**, so Windows SmartScreen shows
"Windows protected your PC / Unknown publisher" on first run — *More info* →
*Run anyway*. See [docs/windows-code-signing.md](docs/windows-code-signing.md).

## Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by [SignPath Foundation](https://signpath.org).

- **Committers, reviewers and approvers**: [Théo (tmeedend)](https://github.com/tmeedend), sole maintainer of the project.
- **Privacy policy**: This program will not transfer any information to other networked systems unless specifically requested by the user or the person installing or operating it.

## Building from source

**Prerequisites**

- **Node 22+**
- **Rust** with the **MSVC toolchain** (C++ linker + Windows SDK) — see the
  [Tauri prerequisites](https://tauri.app/start/prerequisites/)
- **WebView2** — already present on Windows 11

Windows only, and not incidentally: deployment relies on NTFS junctions and
hardlinks, and the backend tests create real ones.

Pit Box is written with Claude as an AI assistant, which is why a project of
this size exists at all as a one-person hobby. The code is open — read it, or
rebuild the installer yourself with the steps below.

```bash
npm install
npm run tauri dev
```

`npm run dev` serves the frontend alone — useful for pure styling work, but
`invoke` does not exist outside Tauri, so every backend call fails and the
screens stay empty. Anything involving data has to run under `tauri dev`.

## Checks

These are exactly what CI runs, so it is worth running them before pushing:

```bash
npm run check && npm run build
```

```bash
cd src-tauri && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

`npm run check` must stay at **0 errors and 0 warnings**, and clippy at 0
warnings — CI treats both as errors. It also verifies that translation files
and version numbers are consistent (see below).

`--workspace` matters: without it cargo only looks at the root package, and the
3D-preview crates under `src-tauri/crates/` go unchecked.

## Releasing

Version numbers live in four files that nothing keeps in sync on its own, so
the bump goes through npm:

```bash
npm version 0.3.0
```

```bash
git push --follow-tags
```

`npm version` bumps `package.json`, then the `version` hook runs
[`scripts/sync-version.mjs`](scripts/sync-version.mjs), which propagates the
number to `tauri.conf.json`, `Cargo.toml` and `Cargo.lock` and stages them —
npm then makes the commit and the `v0.3.0` tag itself. It refuses to run on a
dirty tree.

Pushing the tag triggers [`release.yml`](.github/workflows/release.yml), which
builds the installer and creates a **draft** release: nothing is public until
you review the binaries and publish it. The workflow refuses a tag that does
not match the version being built, rather than spending twenty minutes
producing a `v0.3.0` release whose installer says 0.2.0.

## Contributing

**Translations are the easiest way in** — one file, no code. The app ships six
languages (`fr`, `en`, `it`, `de`, `es`, `pt`) in
[`src/lib/i18n/locales/`](src/lib/i18n/locales/). `en.json` defines every key
that exists and is the fallback: a key missing from a translation shows in
English, never as a raw key, so **an incomplete translation is fine** and can
be improved later.

Two things to know before translating:

- **Assetto Corsa jargon stays in English** — *skin*, *layout*, *pack*,
  *showroom*, *hardlink*, *hotlap*. That is how drivers say it in every
  language, and translating it reads as a machine translation.
- **Keep the `{placeholders}`** exactly as they are. Dropping the `{count}` in
  `"{count} mods imported"` produces a sentence missing its number.
  `npm run check` fails on that, since it is the one mistake nobody can spot by
  proofreading a language they do not speak.

Bug reports and suggestions: [Issues](https://github.com/tmeedend/ac-pitbox/issues).

## Where things are

The repository layout, the conventions and the reasoning behind the
non-obvious choices are documented rather than repeated here — a file tree
copied into a README is the first thing to go stale.

- **[docs/SPEC.md](docs/SPEC.md)** — the reference specification, by domain.
  Describes the app as it actually works. The source of truth.
- **[docs/README.md](docs/README.md)** — index of everything under `docs/`,
  including the import walkthrough, the 3D-preview spec and the research notes
  on the hard parts (CM launching, the KN5 format).
- **[CLAUDE.md](CLAUDE.md)** — working conventions: the non-negotiable rules,
  the pitfalls that do not show up as compile errors, and what to run before
  committing.

## License

[GPL v3](LICENSE). Free to use, modify and redistribute, provided derivative
versions stay under GPL v3.

Pit Box is an independent hobby project, not affiliated with Kunos Simulazioni
or the Content Manager team. Assetto Corsa is a registered trademark of Kunos
Simulazioni S.r.l.
