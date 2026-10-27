//! Installing (download, unpack, register with the system search), updating,
//! launching and removing programs, and their requirements.

use crate::catalog::{Program, Requirement, os};
use crate::github::Release;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Written into every program folder ReLauncher makes; uninstall only deletes
/// folders that have it.
const MARKER: &str = "relauncher.json";

/// Downloads `release` for this platform into `dir/<name>`, replacing an older
/// copy, and adds it to the system search. Returns the program's folder.
pub fn install(
    program: &Program,
    release: &Release,
    dir: &Path,
    progress: &mut dyn FnMut(&str, f32),
) -> Result<PathBuf> {
    let Some(asset) = &release.asset else {
        bail!(
            "release {} has no download for {}",
            release.tag,
            crate::catalog::platform()
        );
    };
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let archive = dir.join(format!(".{}-download.zip", program.id));
    progress("Downloading", 0.0);
    crate::http::download(&asset.url, &archive, |done, total| {
        let total = total.unwrap_or(asset.size).max(1);
        progress("Downloading", done as f32 / total as f32);
    })?;

    progress("Unpacking", 1.0);
    let staging = dir.join(format!(".{}-new", program.id));
    let _ = std::fs::remove_dir_all(&staging);
    let unpacked = unzip(&archive, &staging, program.launch.current());
    let _ = std::fs::remove_file(&archive);
    let root = unpacked?;

    let target = dir.join(&program.name);
    std::fs::write(
        root.join(MARKER),
        serde_json::to_vec_pretty(&serde_json::json!({
            "id": program.id,
            "version": release.version(),
        }))?,
    )?;
    // Swap: the old copy moves aside first, so a failed swap leaves it in place.
    let old = dir.join(format!(".{}-old", program.id));
    let _ = std::fs::remove_dir_all(&old);
    if target.exists() {
        std::fs::rename(&target, &old)
            .with_context(|| format!("{} is in use; close it and try again", program.name))?;
    }
    if let Err(e) = std::fs::rename(&root, &target) {
        let _ = std::fs::rename(&old, &target);
        return Err(e).with_context(|| format!("moving into {}", target.display()));
    }
    let _ = std::fs::remove_dir_all(&old);
    let _ = std::fs::remove_dir_all(&staging);

    progress("Adding to the system search", 1.0);
    // Tests don't touch the real Start Menu / applications folder.
    if !cfg!(test)
        && let Err(e) = register(program, &target)
    {
        // The program works without it; say so but don't fail the install.
        eprintln!("could not add {} to the system search: {e:#}", program.name);
    }
    Ok(target)
}

/// Unpacks `archive` into `dest`; returns the program's root folder. Archives may
/// hold the files directly or inside one top-level folder.
fn unzip(archive: &Path, dest: &Path, launch: Option<&str>) -> Result<PathBuf> {
    let file = std::fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file).context("reading the downloaded archive")?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        // enclosed_name rejects absolute paths and `..` (zip-slip).
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        let out = dest.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut f = std::fs::File::create(&out)?;
        std::io::copy(&mut entry, &mut f)?;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&out, std::fs::Permissions::from_mode(mode & 0o7777))?;
        }
    }
    let found = |root: &Path| launch.is_none_or(|l| root.join(l).exists());
    if found(dest) {
        return Ok(dest.to_owned());
    }
    let mut tops = std::fs::read_dir(dest)?.flatten().map(|e| e.path());
    if let (Some(only), None) = (tops.next(), tops.next())
        && only.is_dir()
        && found(&only)
    {
        return Ok(only);
    }
    bail!(
        "the archive doesn't contain {}",
        launch.unwrap_or("the program")
    )
}

pub fn executable(program: &Program, folder: &Path) -> Option<PathBuf> {
    program.launch.current().map(|l| folder.join(l))
}

pub fn launch(program: &Program, folder: &Path) -> Result<()> {
    let Some(exe) = executable(program, folder) else {
        bail!("{} has nothing to run on this system", program.name);
    };
    let mut cmd = if os() == "macos" && exe.extension().is_some_and(|e| e == "app") {
        let mut c = Command::new("open");
        c.arg(&exe);
        c
    } else {
        Command::new(&exe)
    };
    cmd.current_dir(folder)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("starting {}", exe.display()))?;
    Ok(())
}

/// Removes the program's folder (only one ReLauncher made) and its search entries.
pub fn uninstall(program: &Program, folder: &Path) -> Result<()> {
    if !cfg!(test) {
        unregister(program);
    }
    if !folder.exists() {
        return Ok(());
    }
    if !folder.join(MARKER).is_file() {
        bail!(
            "{} wasn't installed by ReLauncher; not deleting it",
            folder.display()
        );
    }
    std::fs::remove_dir_all(folder)
        .with_context(|| format!("{} is in use; close it and try again", program.name))
}

// ------------------------------------------------------------ system search

/// Windows: a Start Menu shortcut. Linux: a .desktop entry with the icon.
/// macOS: nothing to do, Spotlight and Launchpad find .app bundles by themselves.
fn register(program: &Program, folder: &Path) -> Result<()> {
    let Some(exe) = executable(program, folder) else {
        return Ok(());
    };
    match os() {
        "windows" => {
            #[cfg(windows)]
            {
                let link = start_menu_link(program).context("no Start Menu folder")?;
                let mut lnk = mslnk::ShellLink::new(&exe)?;
                lnk.set_working_dir(Some(folder.display().to_string()));
                lnk.set_name(Some(program.description.clone()));
                lnk.create_lnk(&link)?;
            }
        }
        "linux" => {
            let file = desktop_file(program).context("no applications folder")?;
            std::fs::create_dir_all(file.parent().unwrap_or(folder))?;
            let icon = folder.join("relauncher-icon.png");
            if let Some(url) = &program.icon
                && let Ok(png) = crate::http::get_bytes(url)
            {
                std::fs::write(&icon, png)?;
            }
            std::fs::write(
                &file,
                format!(
                    "[Desktop Entry]\nType=Application\nName={}\nComment={}\nExec=\"{}\" %f\nPath={}\nIcon={}\nTerminal=false\nCategories=Graphics;\n",
                    program.name,
                    program.description.lines().next().unwrap_or_default(),
                    exe.display(),
                    folder.display(),
                    icon.display(),
                ),
            )?;
            let _ = Command::new("update-desktop-database")
                .arg(file.parent().unwrap_or(folder))
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        _ => {}
    }
    Ok(())
}

fn unregister(program: &Program) {
    #[cfg(windows)]
    if let Some(link) = start_menu_link(program) {
        let _ = std::fs::remove_file(link);
    }
    if let Some(file) = desktop_file(program) {
        let _ = std::fs::remove_file(file);
    }
}

#[cfg(windows)]
fn start_menu_link(program: &Program) -> Option<PathBuf> {
    // %APPDATA%\Microsoft\Windows\Start Menu\Programs: searchable from Start.
    Some(
        dirs::data_dir()?
            .join(r"Microsoft\Windows\Start Menu\Programs")
            .join(format!("{}.lnk", program.name)),
    )
}

fn desktop_file(program: &Program) -> Option<PathBuf> {
    (os() == "linux").then_some(())?;
    Some(
        dirs::data_dir()?
            .join("applications")
            .join(format!("relauncher-{}.desktop", program.id)),
    )
}

// ------------------------------------------------------------ requirements

/// Is the requirement met on this system?
pub fn check(req: &Requirement) -> bool {
    req.check
        .files
        .iter()
        .any(|f| Path::new(&expand(f)).exists())
        || req.check.commands.iter().any(|c| {
            let Some((exe, args)) = c.split_first() else {
                return false;
            };
            quiet(Command::new(exe).args(args))
                .status()
                .is_ok_and(|s| s.success())
        })
}

/// Runs the requirement's installer (asking for administrator rights if it needs
/// them). Requirements without one have a page to open instead (`InstallStep::open`).
pub fn install_requirement(req: &Requirement) -> Result<()> {
    let Some(url) = &req.install.run else {
        bail!("{} has to be installed by hand", req.name);
    };
    let name = url.rsplit('/').next().unwrap_or("installer");
    let file = std::env::temp_dir().join(format!("relauncher-{name}"));
    crate::http::download(url, &file, |_, _| {})?;
    let status = if os() == "windows" && req.install.elevated {
        // Start-Process -Verb RunAs shows the UAC prompt; a plain spawn would fail
        // with "requires elevation".
        let quote = |s: &str| format!("'{}'", s.replace('\'', "''"));
        let args = req
            .install
            .args
            .iter()
            .map(|a| quote(a))
            .collect::<Vec<_>>()
            .join(",");
        let mut script = format!(
            "$p = Start-Process -FilePath {} -Verb RunAs -Wait -PassThru",
            quote(&file.display().to_string())
        );
        if !args.is_empty() {
            script += &format!(" -ArgumentList {args}");
        }
        script += "; exit $p.ExitCode";
        quiet(Command::new("powershell").args(["-NoProfile", "-Command", &script])).status()
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755))?;
        }
        Command::new(&file).args(&req.install.args).status()
    }
    .with_context(|| format!("running the {} installer", req.name))?;
    let _ = std::fs::remove_file(&file);
    // vc_redist: 3010 = installed, restart needed; 1638 = a newer one is present.
    match status.code() {
        Some(0 | 3010 | 1638) => Ok(()),
        _ => bail!("the {} installer did not finish ({status})", req.name),
    }
}

/// No console window flashing up from a GUI app on Windows.
fn quiet(cmd: &mut Command) -> &mut Command {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

/// Expands `%VAR%`, `$VAR` and a leading `~`.
fn expand(path: &str) -> String {
    let mut out = path.to_owned();
    if let Some(rest) = out.strip_prefix('~')
        && let Some(home) = dirs::home_dir()
    {
        out = format!("{}{rest}", home.display());
    }
    for (k, v) in std::env::vars() {
        out = out
            .replace(&format!("%{k}%"), &v)
            .replace(&format!("${k}"), &v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn zip_with(path: &Path, files: &[&str]) {
        let mut z = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for f in files {
            z.start_file(*f, zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(b"x").unwrap();
        }
        z.finish().unwrap();
    }

    #[test]
    fn archives_unpack_flat_or_from_one_folder() {
        let tmp = std::env::temp_dir().join(format!("relauncher-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let flat = tmp.join("flat.zip");
        zip_with(&flat, &["app.exe", "lib.dll"]);
        let root = unzip(&flat, &tmp.join("a"), Some("app.exe")).unwrap();
        assert_eq!(root, tmp.join("a"));

        let nested = tmp.join("nested.zip");
        zip_with(&nested, &["App-1.0/app.exe", "App-1.0/lib.dll"]);
        let root = unzip(&nested, &tmp.join("b"), Some("app.exe")).unwrap();
        assert!(root.join("lib.dll").is_file());

        let wrong = tmp.join("wrong.zip");
        zip_with(&wrong, &["other.exe"]);
        assert!(unzip(&wrong, &tmp.join("c"), Some("app.exe")).is_err());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Serves each of `bodies` once over HTTP on localhost; returns the URL.
    fn serve(bodies: Vec<Vec<u8>>) -> String {
        use std::io::{BufRead, BufReader};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/app.zip", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for body in bodies {
                let (mut s, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(s.try_clone().unwrap());
                let mut line = String::new();
                while reader.read_line(&mut line).unwrap() > 2 {
                    line.clear();
                }
                write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                s.write_all(&body).unwrap();
            }
        });
        url
    }

    #[test]
    fn install_update_and_uninstall() {
        let tmp = std::env::temp_dir().join(format!("relauncher-flow-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let zip_bytes = |version: &str| {
            let path = tmp.join(format!("{version}.zip"));
            zip_with(
                &path,
                &[
                    &format!("TestApp-{version}/app.exe"),
                    &format!("TestApp-{version}/v{version}"),
                ],
            );
            std::fs::read(path).unwrap()
        };
        let url = serve(vec![zip_bytes("1"), zip_bytes("2")]);
        let program: Program = serde_json::from_value(serde_json::json!({
            "id": "testapp",
            "name": "TestApp",
            "repo": "https://github.com/x/y",
            "launch": {"windows": "app.exe", "macos": "app.exe", "linux": "app.exe"}
        }))
        .unwrap();
        let release = |tag: &str| Release {
            tag: tag.into(),
            notes: String::new(),
            date: String::new(),
            page: String::new(),
            asset: Some(crate::github::Asset {
                url: url.clone(),
                size: 0,
            }),
        };
        let dir = tmp.join("programs");
        let mut stages = Vec::new();
        let folder = install(&program, &release("v1"), &dir, &mut |s, _| {
            stages.push(s.to_owned())
        })
        .unwrap();
        assert_eq!(folder, dir.join("TestApp"));
        assert!(folder.join("app.exe").is_file() && folder.join("v1").is_file());
        assert!(folder.join(MARKER).is_file());
        assert!(stages.iter().any(|s| s == "Downloading"));

        // An update replaces the old files and leaves no staging folders behind.
        install(&program, &release("v2"), &dir, &mut |_, _| {}).unwrap();
        assert!(folder.join("v2").is_file() && !folder.join("v1").exists());
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(leftovers, ["TestApp"]);

        // Uninstall refuses folders ReLauncher didn't make.
        let foreign = tmp.join("foreign");
        std::fs::create_dir_all(&foreign).unwrap();
        assert!(uninstall(&program, &foreign).is_err() && foreign.exists());
        uninstall(&program, &folder).unwrap();
        assert!(!folder.exists());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn checks_expand_variables() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand("~/x"), format!("{}/x", home.display()));
        let req: Requirement = serde_json::from_value(serde_json::json!({
            "name": "home",
            "check": {"files": ["~"]}
        }))
        .unwrap();
        assert!(check(&req));
        let missing: Requirement = serde_json::from_value(serde_json::json!({
            "name": "nope",
            "check": {"files": ["~/definitely-not-here-relauncher"], "commands": [["definitely-not-a-command"]]}
        }))
        .unwrap();
        assert!(!check(&missing));
    }
}
