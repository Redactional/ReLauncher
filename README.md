# ReLauncher

Installs, updates and launches the Re programs (such as [ReEtched](https://github.com/Redactional/ReEtched)) on Windows, macOS and Linux.

- **One place for every Re program**: see what's installed, what's new, and launch it.
- **Always current**: each program's latest version and release notes come straight from its GitHub releases.
- **Installs where you want**: pick the install folder in Settings. The defaults need no administrator rights:
  - Windows: `%LOCALAPPDATA%\Programs\Re`
  - macOS: `~/Applications`
  - Linux: `~/.local/share/re-programs`
- **Shows up in the system search**:
  - Windows: a Start Menu shortcut.
  - macOS: Spotlight and Launchpad pick up the `.app`.
  - Linux: a `.desktop` entry with the program's icon.
- **Sets up what programs need**: requirements like the Microsoft Visual C++ Runtime are checked and installed along with the program. Optional ones are listed with a link to get them.
- **Clean updates and removal**: an update swaps in the new version in one step. Uninstall removes only folders ReLauncher created.

## Adding a program

The list of programs is [`programs.json`](programs.json) in this repository. ReLauncher downloads it at startup, so new programs appear without a ReLauncher update.

```json
{
  "programs": [
    {
      "id": "reetched",
      "name": "ReEtched",
      "description": "Open-source 3D texture painter.",
      "repo": "https://github.com/Redactional/ReEtched",
      "icon": "https://raw.githubusercontent.com/Redactional/ReEtched/main/assets/icons/ReEtched_1024.png",
      "launch": { "windows": "reetched.exe", "macos": "ReEtched.app", "linux": "reetched" },
      "requirements": [
        {
          "name": "Microsoft Visual C++ Runtime",
          "platforms": ["windows"],
          "check": { "files": ["%SystemRoot%\\System32\\vcruntime140.dll"] },
          "install": { "run": "https://aka.ms/vs/17/release/vc_redist.x64.exe", "args": ["/install", "/quiet", "/norestart"], "elevated": true }
        }
      ]
    }
  ]
}
```

| Field | Meaning |
|---|---|
| `id` | Stable key for install records and shortcuts. Don't change it once published. |
| `repo` | The GitHub repository. Its latest published (non-draft, non-prerelease) release is the version ReLauncher installs. |
| `icon` | Raw URL of a PNG icon. |
| `launch` | What to run inside the unpacked program, per OS. |
| `requirements` | Things the program needs on the system. `check` is met when any listed file exists (`%VAR%`, `$VAR` and `~` are expanded) or any listed command runs successfully. `install.run` downloads and runs an installer (`elevated` asks for administrator rights on Windows). `install.open` opens a page instead. `optional` requirements don't block installs. `platforms` limits a requirement to `windows`, `macos` or `linux`. |

### Release archives

Each release needs one zip per platform whose name ends in `-<platform>.zip`:

- `-windows-x64.zip`
- `-macos-arm64.zip`
- `-linux-x64.zip`

For example, `ReEtched-0.2.0-windows-x64.zip`. The files can sit at the top of the zip or inside one folder. On macOS, the zip holds the `.app`.

To try a changed `programs.json` before pushing it, set `RELAUNCHER_CATALOG` to the file's path (or another URL).

## Building

```bash
cargo run
cargo test
```

Installers are built with [cargo-packager](https://github.com/crabnebula-dev/cargo-packager). Pushing a `v*` tag builds them on all three platforms and attaches them to a draft GitHub release.

## License

MIT or Apache-2.0, at your option.
