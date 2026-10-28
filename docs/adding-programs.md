# Adding a program

ReLauncher's library comes from [`programs.json`](../programs.json) in this repository. ReLauncher downloads it at startup, so a change here reaches every user without a ReLauncher update.

## The entry

```json
{
  "programs": [
    {
      "id": "reetched",
      "name": "ReEtched",
      "description": "Open-source 3D texture painter.",
      "repo": "https://github.com/Redactional/ReEtched",
      "icon": "https://raw.githubusercontent.com/Redactional/ReEtched/main/assets/icons/ReEtched_1024.png",
      "docs": "docs/manual",
      "launch": { "windows": "reetched.exe", "macos": "ReEtched.app", "linux": "reetched" },
      "requirements": [
        {
          "name": "Microsoft Visual C++ Runtime",
          "platforms": ["windows"],
          "check": { "files": ["%SystemRoot%\\System32\\vcruntime140.dll"] },
          "install": {
            "run": "https://aka.ms/vs/17/release/vc_redist.x64.exe",
            "args": ["/install", "/quiet", "/norestart"],
            "elevated": true
          }
        },
        {
          "name": "Python 3.8 or newer",
          "optional": true,
          "note": "Only needed for Python scripts and plugins.",
          "check": { "commands": [["py", "-3", "--version"], ["python3", "--version"]] },
          "install": { "open": "https://www.python.org/downloads/" }
        }
      ]
    }
  ]
}
```

| Field | Meaning |
|---|---|
| `id` | Stable key for install records and shortcuts. Never change it once published. |
| `name`, `description` | Shown in the library. The first line of the description is also used for system search entries. |
| `repo` | The program's GitHub repository. Its latest published release is what ReLauncher installs. Drafts and pre-releases are skipped. |
| `icon` | Raw URL of a square PNG, 256 px or larger. |
| `docs` | Optional. Folder of the program's user manual in its repository; see [Documentation](#documentation). |
| `launch` | What to run inside the unpacked program, per system. Leave a system out if the program doesn't support it. |
| `requirements` | Things the program needs on the system (see below). |

### Requirements

- **`check`** decides whether a requirement is met. It's met when any of these succeeds:
  - **`files`**: any of the listed files exists. `%VAR%`, `$VAR` and a leading `~` are expanded.
  - **`commands`**: any of the listed commands runs successfully.
- **`install`** says how to get it:
  - **`run`**: the URL of an installer to download and run, with `args`. Set `elevated` if it needs administrator rights; Windows then asks for permission.
  - **`open`**: a page to open, for things the user installs themselves.
  - Neither: the `note` explains what to do.
- **`optional`** requirements are shown but never block an install. Required ones with a `run` installer are installed before the program.
- **`platforms`** limits a requirement to `windows`, `macos` or `linux`. Leave it out for all.

## Release archives

Every release needs one `.zip` per supported system, named so it ends in:

- `-windows-x64.zip`
- `-macos-arm64.zip`
- `-linux-x64.zip`

For example, `ReEtched-0.2.0-windows-x64.zip`.

- The files can sit at the top of the zip or inside a single folder.
- On macOS the zip holds the `.app`; `ditto -c -k --keepParent` keeps it intact.
- On Linux, mark the executable as executable before zipping.

ReEtched's [release workflow](https://github.com/Redactional/ReEtched/blob/main/.github/workflows/release.yml) shows one way to build them.

## Documentation

The [website](https://redactional.github.io/ReLauncher/docs.html) shows each program's manual, read live from the program's own repository. Set `docs` to a folder on the `main` branch that contains:

- **`index.json`**, the table of contents:
  ```json
  {
    "title": "ReEtched manual",
    "sections": [
      { "title": "Basics", "pages": [
        { "slug": "getting-started", "title": "Getting started" },
        { "slug": "interface", "title": "The interface" }
      ] }
    ]
  }
  ```
- **One Markdown file per page**, named `<slug>.md`, starting with a `# Title`.

How pages are displayed:
- **Links:** link to other pages with plain relative links like `[Painting](painting.md)`, or `painting.md#brushes` for a section.
- **Images:** paths are relative to the docs folder.
- **Search and "On this page":** the website builds the search, the "On this page" list and the previous/next links itself.
- **Updates:** a change to the manual goes live as soon as it's on `main`. ReLauncher and the website need no update.

## Testing a change

Point ReLauncher at your edited file before pushing it:

```bash
# Windows (PowerShell)
$env:RELAUNCHER_CATALOG = "C:\path\to\programs.json"; cargo run
# macOS / Linux
RELAUNCHER_CATALOG=path/to/programs.json cargo run
```

`RELAUNCHER_CATALOG` also accepts a URL. The unit tests check that the shipped `programs.json` parses.
