use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use tauri::WebviewUrl;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const ODYSSEUS_PORT: u16 = 7000;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopStatus {
    running: bool,
    repo_dir: Option<String>,
    local_python_available: bool,
    message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StartResult {
    ok: bool,
    message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LogResult {
    ok: bool,
    content: String,
    message: String,
}

fn backend_is_running() -> bool {
    let address = ("127.0.0.1", ODYSSEUS_PORT)
        .to_socket_addrs()
        .ok()
        .and_then(|mut addrs| addrs.next());
    let Some(address) = address else {
        return false;
    };

    TcpStream::connect_timeout(&address, Duration::from_millis(500)).is_ok()
}

fn has_project_markers(dir: &Path) -> bool {
    dir.join("app.py").is_file()
        && dir.join("docker-compose.yml").is_file()
        && dir.join("requirements.txt").is_file()
}

fn candidate_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Ok(path) = std::env::var("ODYSSEUS_REPO_DIR") {
        dirs.push(PathBuf::from(path));
    }

    if let Ok(current) = std::env::current_dir() {
        dirs.push(current.clone());
        if let Some(parent) = current.parent() {
            dirs.push(parent.to_path_buf());
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors() {
            dirs.push(ancestor.to_path_buf());
            if let Some(parent) = ancestor.parent() {
                dirs.push(parent.to_path_buf());
            }
        }
    }

    dirs
}

fn find_repo_dir() -> Option<PathBuf> {
    candidate_dirs()
        .into_iter()
        .find(|dir| has_project_markers(dir))
}

fn command_available(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn venv_python(repo_dir: &Path) -> Option<PathBuf> {
    let candidates = [
        repo_dir.join(".venv").join("Scripts").join("python.exe"),
        repo_dir.join("venv").join("Scripts").join("python.exe"),
        repo_dir.join(".venv").join("bin").join("python"),
        repo_dir.join("venv").join("bin").join("python"),
    ];

    candidates.into_iter().find(|path| path.is_file())
}

fn system_python() -> Option<PathBuf> {
    if command_available("python", &["--version"]) {
        Some(PathBuf::from("python"))
    } else if command_available("python3", &["--version"]) {
        Some(PathBuf::from("python3"))
    } else {
        None
    }
}

fn local_python(repo_dir: &Path) -> Option<PathBuf> {
    venv_python(repo_dir).or_else(system_python)
}

fn shell_command(script: String) -> Command {
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("cmd");
        command.arg("/C").raw_arg(script);
        command
    }

    #[cfg(not(target_os = "windows"))]
    {
        let mut command = Command::new("sh");
        command.args(["-c", &script]);
        command
    }
}

fn shell_quote(path: &Path) -> String {
    if cfg!(target_os = "windows") {
        format!("\"{}\"", path.display())
    } else {
        format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
    }
}

fn requirements_stamp_key(repo_dir: &Path) -> Option<String> {
    let metadata = fs::metadata(repo_dir.join("requirements.txt")).ok()?;
    let modified = metadata.modified().ok()?;
    let modified = modified
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(format!("{}:{modified}", metadata.len()))
}

fn requirements_stamp_path(repo_dir: &Path) -> PathBuf {
    repo_dir
        .join("venv")
        .join(".odysseus-desktop-requirements.stamp")
}

fn requirements_are_current(repo_dir: &Path) -> bool {
    let Some(expected) = requirements_stamp_key(repo_dir) else {
        return false;
    };
    fs::read_to_string(requirements_stamp_path(repo_dir))
        .map(|actual| actual.trim() == expected)
        .unwrap_or(false)
}

fn stamp_requirements_command(repo_dir: &Path) -> String {
    let key = requirements_stamp_key(repo_dir).unwrap_or_else(|| "unknown".to_string());
    if cfg!(target_os = "windows") {
        format!(
            "> {} echo {key}",
            shell_quote(&requirements_stamp_path(repo_dir))
        )
    } else {
        format!(
            "printf '%s\\n' '{}' > {}",
            key.replace('\'', "'\\''"),
            shell_quote(&requirements_stamp_path(repo_dir))
        )
    }
}

fn uvicorn_command(venv_python: &Path) -> Command {
    let mut command = Command::new(venv_python);
    command.args([
        "-m",
        "uvicorn",
        "app:app",
        "--host",
        "127.0.0.1",
        "--port",
        "7000",
    ]);
    command
}

fn local_setup_and_run_command(repo_dir: &Path, system_python: &Path) -> Command {
    let python = shell_quote(system_python);
    let stamp = stamp_requirements_command(repo_dir);
    let script = if cfg!(target_os = "windows") {
        format!(
            "call {python} -m venv venv && call venv\\Scripts\\python.exe -m pip install --disable-pip-version-check -r requirements.txt && call venv\\Scripts\\python.exe setup.py && {stamp} && call venv\\Scripts\\python.exe -m uvicorn app:app --host 127.0.0.1 --port 7000"
        )
    } else {
        format!(
            "{python} -m venv venv && venv/bin/python -m pip install --disable-pip-version-check -r requirements.txt && venv/bin/python setup.py && {stamp} && venv/bin/python -m uvicorn app:app --host 127.0.0.1 --port 7000"
        )
    };

    shell_command(script)
}

fn local_refresh_and_run_command(repo_dir: &Path, venv_python: &Path) -> Command {
    if requirements_are_current(repo_dir) {
        return uvicorn_command(venv_python);
    }

    let python = shell_quote(venv_python);
    let stamp = stamp_requirements_command(repo_dir);
    let script = if cfg!(target_os = "windows") {
        format!(
            "call {python} -m pip install --disable-pip-version-check -r requirements.txt && {stamp} && call {python} -m uvicorn app:app --host 127.0.0.1 --port 7000"
        )
    } else {
        format!(
            "{python} -m pip install --disable-pip-version-check -r requirements.txt && {stamp} && {python} -m uvicorn app:app --host 127.0.0.1 --port 7000"
        )
    };

    shell_command(script)
}

fn apply_python_launch_env(command: &mut Command, repo_dir: &Path) {
    let shim_dir = repo_dir.join("src-tauri").join("python-shims");
    if shim_dir.is_dir() {
        let existing = std::env::var("PYTHONPATH").unwrap_or_default();
        let separator = if cfg!(target_os = "windows") {
            ";"
        } else {
            ":"
        };
        let pythonpath = if existing.is_empty() {
            shim_dir.display().to_string()
        } else {
            format!("{}{}{}", shim_dir.display(), separator, existing)
        };
        command.env("PYTHONPATH", pythonpath);
    }
}

fn log_file(repo_dir: &Path, name: &str) -> Result<std::fs::File, String> {
    let logs = repo_dir.join("logs");
    fs::create_dir_all(&logs)
        .map_err(|error| format!("Could not create logs directory: {error}"))?;
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(logs.join(name))
        .map_err(|error| format!("Could not open desktop log file: {error}"))
}

fn log_path(repo_dir: &Path, name: &str) -> PathBuf {
    repo_dir.join("logs").join(name)
}

fn read_log_tail(path: &Path, max_bytes: u64) -> Result<String, String> {
    let mut file = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|error| format!("Could not open desktop log file: {error}"))?;
    let len = file
        .metadata()
        .map_err(|error| format!("Could not read desktop log metadata: {error}"))?
        .len();
    let start = len.saturating_sub(max_bytes);
    file.seek(SeekFrom::Start(start))
        .map_err(|error| format!("Could not seek desktop log file: {error}"))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| format!("Could not read desktop log file: {error}"))?;
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

fn spawn_hidden(command: &mut Command) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Failed to start process: {error}"))
}

#[tauri::command]
fn desktop_status() -> DesktopStatus {
    let running = backend_is_running();
    let repo_dir = find_repo_dir();
    let local_python_available = repo_dir
        .as_ref()
        .and_then(|path| local_python(path))
        .is_some();

    let message = if running {
        "Odysseus is running on localhost:7000.".to_string()
    } else if repo_dir.is_none() {
        "Odysseus is not running and the repo checkout was not found. Set ODYSSEUS_REPO_DIR to the checkout path.".to_string()
    } else {
        "Odysseus is not running yet.".to_string()
    };

    DesktopStatus {
        running,
        repo_dir: repo_dir.map(|path| path.display().to_string()),
        local_python_available,
        message,
    }
}

#[tauri::command]
fn start_local_backend() -> StartResult {
    start_local_backend_inner()
}

#[tauri::command]
fn backend_ready() -> bool {
    backend_is_running()
}

#[tauri::command]
fn read_desktop_log() -> LogResult {
    let Some(repo_dir) = find_repo_dir() else {
        return LogResult {
            ok: false,
            content: String::new(),
            message: "Repo checkout not found. Set ODYSSEUS_REPO_DIR to the checkout path."
                .to_string(),
        };
    };

    match read_log_tail(&log_path(&repo_dir, "desktop-local.log"), 32 * 1024) {
        Ok(content) => LogResult {
            ok: true,
            content,
            message: "Desktop log loaded.".to_string(),
        },
        Err(message) => LogResult {
            ok: false,
            content: String::new(),
            message,
        },
    }
}

fn start_local_backend_inner() -> StartResult {
    if backend_is_running() {
        return StartResult {
            ok: true,
            message: "Odysseus is already running.".to_string(),
        };
    }

    let Some(repo_dir) = find_repo_dir() else {
        return StartResult {
            ok: false,
            message: "Repo checkout not found. Set ODYSSEUS_REPO_DIR before starting from the desktop wrapper.".to_string(),
        };
    };
    let existing_venv_python = venv_python(&repo_dir);
    let Some(python) = existing_venv_python.clone().or_else(system_python) else {
        return StartResult {
            ok: false,
            message: "Python was not found. Install Python 3.11+ and reopen the desktop wrapper."
                .to_string(),
        };
    };

    let stdout = match log_file(&repo_dir, "desktop-local.log") {
        Ok(file) => file,
        Err(message) => return StartResult { ok: false, message },
    };
    let stderr = match stdout.try_clone() {
        Ok(file) => file,
        Err(error) => {
            return StartResult {
                ok: false,
                message: format!("Could not clone desktop local log file: {error}"),
            }
        }
    };

    let mut command = if let Some(venv_python) = existing_venv_python.as_deref() {
        local_refresh_and_run_command(&repo_dir, venv_python)
    } else {
        local_setup_and_run_command(&repo_dir, &python)
    };
    apply_python_launch_env(&mut command, &repo_dir);
    command
        .current_dir(&repo_dir)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));

    match spawn_hidden(&mut command) {
        Ok(()) => StartResult {
            ok: true,
            message: if existing_venv_python.is_some() {
                "Local dependency check and start requested. Odysseus will open when localhost:7000 is ready."
                    .to_string()
            } else {
                "Local setup and start requested. This can take several minutes on first run."
                    .to_string()
            },
        },
        Err(message) => StartResult { ok: false, message },
    }
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            desktop_status,
            start_local_backend,
            backend_ready,
            read_desktop_log
        ])
        .setup(|app| {
            let url = if backend_is_running() {
                WebviewUrl::External(
                    format!("http://localhost:{ODYSSEUS_PORT}")
                        .parse()
                        .expect("valid local Odysseus URL"),
                )
            } else {
                WebviewUrl::App("index.html".into())
            };

            let mut window = tauri::WebviewWindowBuilder::new(app, "main", url)
                .title("Odysseus")
                .inner_size(1280.0, 860.0)
                .min_inner_size(960.0, 640.0)
                .resizable(true);

            if let Some(icon) = app.default_window_icon().cloned() {
                window = window.icon(icon)?;
            }

            window.build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Odysseus desktop wrapper");
}
