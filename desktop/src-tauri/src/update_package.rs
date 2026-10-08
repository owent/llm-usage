use crate::updates::Candidate;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const MARKER: &str = "llmusage-package.json";
const WORK: &str = ".llmusage-update";
const MAX_EXPANDED: u64 = 2 * 1024 * 1024 * 1024;
const MAX_ENTRIES: usize = 50_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct PackageIdentity {
    pub schema: u32,
    pub kind: String,
    pub platform: String,
    pub arch: String,
    pub version: String,
    pub executable: String,
    pub entrypoint: String,
    pub managed: Vec<String>,
    #[serde(default)]
    pub root: PathBuf,
}

impl PackageIdentity {
    pub(crate) fn asset_name(&self, version: &str) -> Result<String, String> {
        if !["windows", "linux", "macos"].contains(&self.platform.as_str())
            || !["x64", "arm64"].contains(&self.arch.as_str())
        {
            return Err("unsupported_update_platform".into());
        }
        match self.kind.as_str() {
            "portable" => Ok(format!(
                "LLMUsage-{version}-{}-{}-portable.tar.zst",
                self.platform, self.arch
            )),
            "installer" if self.platform == "windows" && self.arch == "x64" => {
                Ok(format!("LLMUsage_{version}_x64-setup.exe"))
            }
            _ => Err("unsupported_update_package_kind".into()),
        }
    }

    fn validate_marker(&self) -> Result<(), String> {
        if self.schema != 1
            || self.kind != "portable"
            || self.managed.is_empty()
            || self.managed.len() > 100
            || !self.managed.iter().any(|name| name == MARKER)
        {
            return Err("invalid_portable_identity".into());
        }
        self.asset_name(&self.version)?;
        let version =
            semver::Version::parse(&self.version).map_err(|_| "invalid_package_version")?;
        if !version.pre.is_empty() || !version.build.is_empty() {
            return Err("invalid_package_version".into());
        }
        let mut names = BTreeSet::new();
        for name in &self.managed {
            let path = safe_relative(name)?;
            if path.components().count() != 1
                || name.starts_with(WORK)
                || !names.insert(name.to_lowercase())
            {
                return Err("invalid_managed_package_entry".into());
            }
        }
        for name in [&self.executable, &self.entrypoint] {
            let path = safe_relative(name)?;
            let top = path
                .components()
                .next()
                .ok_or("invalid_package_entrypoint")?
                .as_os_str()
                .to_string_lossy();
            if !self.managed.iter().any(|name| name == &top) {
                return Err("unmanaged_package_entrypoint".into());
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn test_portable(platform: &str, arch: &str) -> Self {
        Self {
            schema: 1,
            kind: "portable".into(),
            platform: platform.into(),
            arch: arch.into(),
            version: "0.2.2".into(),
            executable: "LLMUsage.exe".into(),
            entrypoint: "LLMUsage.exe".into(),
            managed: vec!["LLMUsage.exe".into(), MARKER.into()],
            root: PathBuf::new(),
        }
    }
}

fn platform() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unsupported"
    }
}

fn arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86_64") {
        "x64"
    } else {
        "unsupported"
    }
}

pub(crate) fn read_small(path: &Path) -> Result<Vec<u8>, String> {
    regular(path)?;
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("update_metadata_too_large".into());
    }
    Ok(bytes)
}

fn regular(path: &Path) -> Result<(), String> {
    let info = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !info.is_file() || info.file_type().is_symlink() {
        return Err("update_file_is_not_regular".into());
    }
    Ok(())
}

pub(crate) fn create_file(path: &Path) -> Result<File, String> {
    if path.try_exists().map_err(|e| e.to_string())? {
        regular(path)?;
    }
    File::options()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .map_err(|e| e.to_string())
}

pub(crate) fn replace_cache_file(source: &Path, destination: &Path) -> Result<(), String> {
    if destination.try_exists().map_err(|e| e.to_string())? {
        regular(destination)?;
    }
    fs::rename(source, destination).map_err(|e| e.to_string())
}

pub(crate) fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    let mut file = create_file(&tmp)?;
    file.write_all(&serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    replace_cache_file(&tmp, path)
}

pub(crate) fn file_hash(path: &Path) -> Result<String, String> {
    regular(path)?;
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

pub(crate) fn verify_file(path: &Path, size: u64, hash: &str) -> Result<(), String> {
    regular(path)?;
    if fs::metadata(path).map_err(|e| e.to_string())?.len() != size || file_hash(path)? != hash {
        return Err("update_package_digest_mismatch".into());
    }
    Ok(())
}

pub(crate) fn require_space(path: &Path, bytes: u64) -> Result<(), String> {
    if fs4::available_space(path).map_err(|e| e.to_string())?
        < bytes.saturating_add(128 * 1024 * 1024)
    {
        return Err("insufficient_update_disk_space".into());
    }
    Ok(())
}

pub(crate) fn cache_directory(db: &Path) -> PathBuf {
    let executable = std::env::current_exe()
        .and_then(|path| path.canonicalize())
        .unwrap_or_default();
    let identity = executable.to_string_lossy();
    let normalized = if cfg!(windows) {
        identity
            .strip_prefix(r"\\?\")
            .unwrap_or(&identity)
            .to_lowercase()
    } else {
        identity.to_string()
    };
    let hash = format!("{:x}", Sha256::digest(normalized.as_bytes()));
    db.parent()
        .unwrap_or_else(|| Path::new("."))
        .join("update-cache")
        .join(&hash[..24])
}

pub(crate) fn detect_identity() -> Result<PackageIdentity, String> {
    let executable = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    #[cfg(windows)]
    if let Some((root, version)) = installed_registration(&executable)? {
        if version != env!("CARGO_PKG_VERSION") {
            return Err("installed_version_mismatch".into());
        }
        return Ok(PackageIdentity {
            schema: 1,
            kind: "installer".into(),
            platform: platform().into(),
            arch: arch().into(),
            version: env!("CARGO_PKG_VERSION").into(),
            executable: "LLMUsage.exe".into(),
            entrypoint: "LLMUsage.exe".into(),
            managed: Vec::new(),
            root,
        });
    }
    for root in executable.ancestors().skip(1).take(8) {
        let marker = root.join(MARKER);
        if !marker.try_exists().map_err(|e| e.to_string())? {
            continue;
        }
        regular(&marker)?;
        let mut identity: PackageIdentity = serde_json::from_slice(&read_small(&marker)?)
            .map_err(|_| "invalid_portable_identity")?;
        identity.validate_marker()?;
        if identity.platform != platform()
            || identity.arch != arch()
            || identity.version != env!("CARGO_PKG_VERSION")
            || root
                .join(&identity.executable)
                .canonicalize()
                .map_err(|e| e.to_string())?
                != executable
        {
            return Err("portable_identity_mismatch".into());
        }
        identity.root = root.to_path_buf();
        return Ok(identity);
    }
    Err("update_package_identity_unknown".into())
}

#[cfg(windows)]
fn installed_registration(executable: &Path) -> Result<Option<(PathBuf, String)>, String> {
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND};
    use windows::Win32::System::Registry::{
        RegGetValueW, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RRF_SUBKEY_WOW6464KEY,
    };
    for key in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let mut values = Vec::new();
        for name in [
            w!("InstallLocation"),
            w!("MainBinaryName"),
            w!("DisplayVersion"),
        ] {
            let mut buffer = [0u16; 32768];
            let mut bytes = (buffer.len() * 2) as u32;
            let result = unsafe {
                RegGetValueW(
                    key,
                    w!("Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\LLMUsage"),
                    name,
                    RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY,
                    None,
                    Some(buffer.as_mut_ptr().cast()),
                    Some(&mut bytes),
                )
            };
            if result == ERROR_FILE_NOT_FOUND || result == ERROR_PATH_NOT_FOUND {
                break;
            }
            result
                .ok()
                .map_err(|_| "update_installation_registry_unreadable")?;
            let text = unsafe { PCWSTR(buffer.as_ptr()).to_string() }
                .map_err(|_| "invalid_installation_registry_value")?;
            values.push(text);
        }
        if values.len() == 3 {
            let root = Path::new(values[0].trim_matches('"'));
            if let Ok(found) = root.join(&values[1]).canonicalize() {
                if crate::installation::same_executable(
                    &found.to_string_lossy(),
                    &executable.to_string_lossy(),
                ) {
                    return Ok(Some((
                        root.canonicalize().map_err(|e| e.to_string())?,
                        values[2].clone(),
                    )));
                }
            }
        }
    }
    Ok(None)
}

fn safe_relative(value: &str) -> Result<PathBuf, String> {
    if value.is_empty() || value.contains(['\\', ':', '\0']) || value.starts_with('/') {
        return Err("unsafe_update_archive_path".into());
    }
    for part in value.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.ends_with(['.', ' ']) {
            return Err("unsafe_update_archive_path".into());
        }
        let stem = part.split('.').next().unwrap_or("").to_uppercase();
        if [
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ]
        .contains(&stem.as_str())
        {
            return Err("unsafe_update_archive_path".into());
        }
    }
    let path = PathBuf::from(value);
    if path
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("unsafe_update_archive_path".into());
    }
    Ok(path)
}

fn safe_link(member: &Path, value: &str) -> Result<(), String> {
    if value.is_empty() || value.starts_with('/') || value.contains(['\\', ':', '\0']) {
        return Err("unsafe_update_archive_link".into());
    }
    let mut depth = member.parent().map_or(0, |p| p.components().count());
    for part in value.split('/') {
        match part {
            ".." => {
                depth = depth.checked_sub(1).ok_or("unsafe_update_archive_link")?;
            }
            "." => {}
            "" => return Err("unsafe_update_archive_link".into()),
            _ => {
                safe_relative(part)?;
                depth += 1;
            }
        }
    }
    Ok(())
}

fn archive_reader(path: &Path) -> Result<impl Read, String> {
    let mut decoder =
        zstd::stream::read::Decoder::new(File::open(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    decoder.window_log_max(27).map_err(|e| e.to_string())?;
    Ok(decoder.take(MAX_EXPANDED + 64 * 1024 * 1024))
}

fn extract_archive(
    path: &Path,
    destination: &Path,
    root_name: &str,
    cancel: &impl Fn() -> Result<(), String>,
) -> Result<(), String> {
    let mut total = 0u64;
    let mut count = 0;
    for entry in tar::Archive::new(archive_reader(path)?)
        .entries()
        .map_err(|e| e.to_string())?
        .raw(true)
    {
        cancel()?;
        let entry = entry.map_err(|e| e.to_string())?;
        count += 1;
        let size = entry.size();
        total = total.checked_add(size).ok_or("expanded_update_too_large")?;
        if total > MAX_EXPANDED || count > MAX_ENTRIES {
            return Err("expanded_update_too_large".into());
        }
        if !entry.header().entry_type().is_file() && size > 16 * 1024 {
            return Err("update_archive_metadata_too_large".into());
        }
    }
    require_space(destination, total)?;
    let mut seen = BTreeSet::new();
    let mut links = Vec::new();
    let mut archive = tar::Archive::new(archive_reader(path)?);
    for entry in archive.entries().map_err(|e| e.to_string())? {
        cancel()?;
        let mut entry = entry.map_err(|e| e.to_string())?;
        let text = entry
            .path()
            .map_err(|e| e.to_string())?
            .to_str()
            .ok_or("non_utf8_update_archive_path")?
            .trim_end_matches('/')
            .to_string();
        if text == root_name && entry.header().entry_type().is_dir() {
            continue;
        }
        let relative = text
            .strip_prefix(&format!("{root_name}/"))
            .ok_or("unexpected_update_archive_root")?;
        let member = safe_relative(relative)?;
        if relative.starts_with(WORK)
            || !seen.insert(if cfg!(windows) {
                relative.to_lowercase()
            } else {
                relative.to_string()
            })
        {
            return Err("duplicate_or_reserved_update_archive_member".into());
        }
        let target = destination.join(&member);
        if entry.header().entry_type().is_symlink() {
            if cfg!(windows) {
                return Err("windows_update_archive_symlink".into());
            }
            let value = entry
                .link_name()
                .map_err(|e| e.to_string())?
                .ok_or("invalid_update_archive_link")?
                .to_str()
                .ok_or("non_utf8_update_archive_link")?
                .to_string();
            safe_link(&member, &value)?;
            links.push((member, value));
        } else if entry.header().entry_type().is_file() || entry.header().entry_type().is_dir() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            entry.unpack(&target).map_err(|e| e.to_string())?;
        } else {
            return Err("unsupported_update_archive_member".into());
        }
    }
    #[cfg(unix)]
    for (member, value) in &links {
        let target = destination.join(member);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::os::unix::fs::symlink(value, target).map_err(|e| e.to_string())?;
    }
    #[cfg(unix)]
    for (member, _) in links {
        if !destination
            .join(member)
            .canonicalize()
            .map_err(|_| "dangling_or_cyclic_update_link")?
            .starts_with(destination.canonicalize().map_err(|e| e.to_string())?)
        {
            return Err("unsafe_update_archive_link".into());
        }
    }
    Ok(())
}

fn binary_arch(path: &Path, os: &str) -> Result<&'static str, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    (&mut file)
        .take(64 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if os == "windows" && bytes.starts_with(b"MZ") && bytes.len() >= 64 {
        let offset = u32::from_le_bytes(bytes[60..64].try_into().unwrap()) as usize;
        if offset <= bytes.len().saturating_sub(6) && bytes[offset..offset + 4] == *b"PE\0\0" {
            return match u16::from_le_bytes(bytes[offset + 4..offset + 6].try_into().unwrap()) {
                0x8664 => Ok("x64"),
                0xaa64 => Ok("arm64"),
                _ => Err("invalid_update_architecture".into()),
            };
        }
    }
    if os == "linux" && bytes.len() >= 20 && bytes.starts_with(&[0x7f, b'E', b'L', b'F', 2, 1]) {
        return match u16::from_le_bytes(bytes[18..20].try_into().unwrap()) {
            62 => Ok("x64"),
            183 => Ok("arm64"),
            _ => Err("invalid_update_architecture".into()),
        };
    }
    if os == "macos" && bytes.len() >= 8 && bytes[..4] == [0xcf, 0xfa, 0xed, 0xfe] {
        return match u32::from_le_bytes(bytes[4..8].try_into().unwrap()) {
            0x01000007 => Ok("x64"),
            0x0100000c => Ok("arm64"),
            _ => Err("invalid_update_architecture".into()),
        };
    }
    Err("invalid_update_executable".into())
}

fn read_staged_identity(
    root: &Path,
    old: &PackageIdentity,
    candidate: &Candidate,
) -> Result<PackageIdentity, String> {
    regular(&root.join(MARKER))?;
    let mut new: PackageIdentity = serde_json::from_slice(&read_small(&root.join(MARKER))?)
        .map_err(|_| "invalid_portable_identity")?;
    new.validate_marker()?;
    if new.version != candidate.version
        || new.platform != old.platform
        || new.arch != old.arch
        || new.executable != old.executable
        || new.entrypoint != old.entrypoint
    {
        return Err("updated_package_identity_mismatch".into());
    }
    let actual: BTreeSet<String> = fs::read_dir(root)
        .map_err(|e| e.to_string())?
        .map(|e| e.map(|v| v.file_name().to_string_lossy().to_string()))
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    if actual != new.managed.iter().cloned().collect() {
        return Err("updated_package_inventory_mismatch".into());
    }
    if !root
        .join(&new.executable)
        .canonicalize()
        .map_err(|e| e.to_string())?
        .starts_with(root.canonicalize().map_err(|e| e.to_string())?)
        || binary_arch(&root.join(&new.executable), &new.platform)? != new.arch
    {
        return Err("updated_package_architecture_mismatch".into());
    }
    new.root = root.to_path_buf();
    Ok(new)
}

pub(crate) fn validate_download(
    identity: &PackageIdentity,
    candidate: &Candidate,
    cache: &Path,
    cancel: impl Fn() -> Result<(), String>,
) -> Result<(), String> {
    if identity.kind == "installer" {
        return Ok(());
    }
    let stage = cache.join("validation");
    if stage.exists() {
        remove_owned(&stage, cache)?;
    }
    fs::create_dir(&stage).map_err(|e| e.to_string())?;
    let result = extract_archive(
        &cache.join(&candidate.name),
        &stage,
        candidate.name.trim_end_matches(".tar.zst"),
        &cancel,
    )
    .and_then(|_| read_staged_identity(&stage, identity, candidate).map(|_| ()));
    let cleanup = remove_owned(&stage, cache);
    result.and(cleanup)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Swap {
    name: String,
    old_hash: Option<String>,
    new_hash: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApplyPlan {
    schema: u32,
    identity: PackageIdentity,
    candidate: Candidate,
    executable_hash: String,
    db_path: PathBuf,
    parent_pid: u32,
    phase: String,
    swaps: Vec<Swap>,
    error: Option<String>,
}

fn path_exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn tree_hash(path: &Path) -> Result<String, String> {
    let info = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    if info.file_type().is_symlink() {
        hash.update(b"link:");
        hash.update(
            fs::read_link(path)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .as_bytes(),
        );
    } else if info.is_file() {
        hash.update(b"file:");
        hash.update(file_hash(path)?.as_bytes());
    } else if info.is_dir() {
        hash.update(b"directory:");
        let mut entries = fs::read_dir(path)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            hash.update(entry.file_name().to_string_lossy().as_bytes());
            hash.update([0]);
            hash.update(tree_hash(&entry.path())?.as_bytes());
        }
    } else {
        return Err("unsupported_update_target_entry".into());
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn remove_owned(path: &Path, parent: &Path) -> Result<(), String> {
    if path.parent() != Some(parent)
        || path.file_name().is_none()
        || path
            .parent()
            .ok_or("invalid_update_work_path")?
            .canonicalize()
            .map_err(|e| e.to_string())?
            != parent.canonicalize().map_err(|e| e.to_string())?
    {
        return Err("invalid_update_work_path".into());
    }
    let info = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if info.is_dir() && !info.file_type().is_symlink() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
    .map_err(|e| e.to_string())
}

fn target_lock(root: &Path) -> Result<File, String> {
    let lock = root.join(".llmusage-update.lock");
    let file = if path_exists(&lock) {
        regular(&lock)?;
        File::options().read(true).write(true).open(&lock)
    } else {
        File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&lock)
    }
    .map_err(|e| e.to_string())?;
    file.try_lock().map_err(|_| "another_update_is_running")?;
    Ok(file)
}

fn reject_unmanaged_collisions(old: &PackageIdentity, new: &PackageIdentity) -> Result<(), String> {
    for name in &new.managed {
        if !old.managed.contains(name) && path_exists(&old.root.join(name)) {
            return Err("update_would_overwrite_unmanaged_file".into());
        }
    }
    Ok(())
}

pub(crate) fn prepare_and_launch(
    identity: &PackageIdentity,
    candidate: &Candidate,
    cache: &Path,
    db_path: &Path,
    cancel: impl Fn() -> Result<(), String>,
) -> Result<(), String> {
    if detect_identity()? != *identity {
        return Err("update_installation_changed".into());
    }
    crate::updates::validate_candidate(candidate, identity)?;
    verify_file(
        &cache.join(&candidate.name),
        candidate.size,
        &candidate.sha256,
    )?;
    let lock = target_lock(&identity.root)?;
    let work = identity.root.join(WORK);
    if path_exists(&work) {
        if fs::symlink_metadata(&work)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("unsafe_update_work_directory".into());
        }
        let mut previous: ApplyPlan =
            serde_json::from_slice(&read_small(&work.join("apply.json"))?)
                .map_err(|_| "invalid_previous_update_journal")?;
        validate_plan(&previous, &work)?;
        if previous.phase == "prepared"
            && previous.parent_pid == std::process::id()
            && identity.kind == "portable"
        {
            rollback(&mut previous, &work)?;
        }
        if previous.phase == "failed"
            && previous.error.as_deref() != Some("update_installer_result_unknown")
            && file_hash(&identity.root.join(&identity.executable))? == previous.executable_hash
        {
            previous.phase = "rolled_back".into();
        }
        if !["applied", "rolled_back", "installer_finished"].contains(&previous.phase.as_str()) {
            return Err("previous_update_requires_recovery".into());
        }
        remove_owned(&work, &identity.root)?;
    }
    fs::create_dir(&work).map_err(|e| e.to_string())?;
    let prepare = (|| {
        require_space(&work, candidate.size.saturating_mul(4))?;
        let incoming = work.join("new");
        fs::create_dir(&incoming).map_err(|e| e.to_string())?;
        let mut swaps = Vec::new();
        if identity.kind == "portable" {
            extract_archive(
                &cache.join(&candidate.name),
                &incoming,
                candidate.name.trim_end_matches(".tar.zst"),
                &cancel,
            )?;
            let new = read_staged_identity(&incoming, identity, candidate)?;
            reject_unmanaged_collisions(identity, &new)?;
            let mut names: BTreeSet<_> = identity
                .managed
                .iter()
                .chain(new.managed.iter())
                .cloned()
                .collect();
            names.remove(MARKER);
            let mut names: Vec<_> = names.into_iter().collect();
            names.push(MARKER.into());
            for name in names {
                cancel()?;
                let original = identity.root.join(&name);
                swaps.push(Swap {
                    name: name.clone(),
                    old_hash: path_exists(&original)
                        .then(|| tree_hash(&original))
                        .transpose()?,
                    new_hash: path_exists(&incoming.join(&name))
                        .then(|| tree_hash(&incoming.join(&name)))
                        .transpose()?,
                });
            }
        } else {
            fs::copy(cache.join(&candidate.name), incoming.join(&candidate.name))
                .map_err(|e| e.to_string())?;
            verify_file(
                &incoming.join(&candidate.name),
                candidate.size,
                &candidate.sha256,
            )?;
        }
        let executable = identity.root.join(&identity.executable);
        let executable_hash = file_hash(&executable)?;
        #[cfg(windows)]
        {
            fs::copy(&executable, work.join("helper.exe")).map_err(|e| e.to_string())?;
        }
        let plan = ApplyPlan {
            schema: 1,
            identity: identity.clone(),
            candidate: candidate.clone(),
            executable_hash,
            db_path: db_path.to_path_buf(),
            parent_pid: std::process::id(),
            phase: "prepared".into(),
            swaps,
            error: None,
        };
        write_json(&work.join("apply.json"), &plan)?;
        cancel()?;
        Ok::<_, String>(())
    })();
    if let Err(error) = prepare {
        let _ = remove_owned(&work, &identity.root);
        return Err(error);
    }
    drop(lock);
    launch_helper(&work, false)
}

fn validate_plan(plan: &ApplyPlan, work: &Path) -> Result<(), String> {
    if plan.schema != 1
        || plan.identity.root.join(WORK) != work
        || !plan.db_path.is_absolute()
        || plan.parent_pid == 0
        || plan.identity.platform != platform()
        || plan.identity.arch != arch()
        || plan.executable_hash.len() != 64
        || plan.swaps.len() > 200
    {
        return Err("invalid_update_journal".into());
    }
    crate::updates::validate_candidate(&plan.candidate, &plan.identity)?;
    if semver::Version::parse(&plan.candidate.version).map_err(|_| "invalid_update_version")?
        <= semver::Version::parse(&plan.identity.version)
            .map_err(|_| "invalid_installed_version")?
    {
        return Err("update_version_is_not_newer".into());
    }
    if plan.identity.kind == "portable" {
        plan.identity.validate_marker()?;
    } else if plan.identity.kind != "installer" || plan.identity.executable != "LLMUsage.exe" {
        return Err("invalid_update_journal".into());
    }
    let mut names = BTreeSet::new();
    for swap in &plan.swaps {
        let relative = safe_relative(&swap.name)?;
        if relative.components().count() != 1
            || swap.name.starts_with(WORK)
            || !names.insert(swap.name.to_lowercase())
            || [swap.old_hash.as_ref(), swap.new_hash.as_ref()]
                .into_iter()
                .flatten()
                .any(|v| v.len() != 64 || !v.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err("invalid_update_journal".into());
        }
    }
    if plan
        .identity
        .root
        .canonicalize()
        .map_err(|e| e.to_string())?
        != plan.identity.root
        || work.canonicalize().map_err(|e| e.to_string())? != work
    {
        return Err("unsafe_update_journal_location".into());
    }
    Ok(())
}

fn hidden(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    #[cfg(not(windows))]
    let _ = command;
}

fn launch_helper(work: &Path, recover: bool) -> Result<(), String> {
    let plan: ApplyPlan = serde_json::from_slice(&read_small(&work.join("apply.json"))?)
        .map_err(|_| "invalid_update_journal")?;
    let helper = if cfg!(windows) {
        work.join("helper.exe")
    } else {
        plan.identity.root.join(&plan.identity.executable)
    };
    if cfg!(windows) && file_hash(&helper)? != plan.executable_hash {
        return Err("update_helper_digest_mismatch".into());
    }
    let ready = work.join("ready");
    if ready.exists() {
        fs::remove_file(&ready).map_err(|e| e.to_string())?;
    }
    let mut command = Command::new(helper);
    command
        .arg(if recover {
            "--recover-update"
        } else {
            "--apply-update"
        })
        .arg(work.join("apply.json"));
    hidden(&mut command);
    let mut child = command.spawn().map_err(|_| "update_helper_launch_failed")?;
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if ready.exists() {
            return Ok(());
        }
        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
            return Err("update_helper_failed_before_exit".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    let _ = child.wait();
    Err("update_helper_readiness_timeout".into())
}

fn rollback(plan: &mut ApplyPlan, work: &Path) -> Result<(), String> {
    let root = &plan.identity.root;
    for swap in plan.swaps.iter().rev() {
        let destination = root.join(&swap.name);
        let backup = work.join("backup").join(&swap.name);
        if path_exists(&backup) {
            if Some(tree_hash(&backup)?) != swap.old_hash {
                return Err("update_backup_modified".into());
            }
            if path_exists(&destination) {
                let current = Some(tree_hash(&destination)?);
                if current == swap.old_hash {
                    remove_owned(&backup, &work.join("backup"))?;
                    continue;
                }
                if current != swap.new_hash {
                    return Err("update_target_modified_recovery_refused".into());
                }
                remove_owned(&destination, root)?;
            }
            fs::rename(&backup, &destination).map_err(|e| e.to_string())?;
        } else if swap.old_hash.is_none() && path_exists(&destination) {
            if Some(tree_hash(&destination)?) != swap.new_hash {
                return Err("update_target_modified_recovery_refused".into());
            }
            remove_owned(&destination, root)?;
        } else if swap.old_hash.is_some()
            && (!path_exists(&destination) || Some(tree_hash(&destination)?) != swap.old_hash)
        {
            return Err("update_original_or_backup_missing".into());
        }
    }
    plan.phase = "rolled_back".into();
    write_json(&work.join("apply.json"), plan)
}

fn replace_with_backup(original: &Path, incoming: &Path, backup: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::{ReplaceFileW, REPLACE_FILE_FLAGS};
        let wide = |path: &Path| {
            path.as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<_>>()
        };
        let original = wide(original);
        let incoming = wide(incoming);
        let backup = wide(backup);
        unsafe {
            ReplaceFileW(
                PCWSTR(original.as_ptr()),
                PCWSTR(incoming.as_ptr()),
                PCWSTR(backup.as_ptr()),
                REPLACE_FILE_FLAGS(0),
                None,
                None,
            )
            .map_err(|e| format!("update_file_replacement_failed: {e}"))
        }
    }
    #[cfg(unix)]
    {
        fs::hard_link(original, backup).map_err(|e| e.to_string())?;
        fs::rename(incoming, original).map_err(|e| e.to_string())
    }
}

fn apply_swaps(
    plan: &mut ApplyPlan,
    work: &Path,
    before: impl Fn(usize) -> Result<(), String>,
) -> Result<(), String> {
    fs::create_dir_all(work.join("backup")).map_err(|e| e.to_string())?;
    plan.phase = "applying".into();
    write_json(&work.join("apply.json"), plan)?;
    for (index, swap) in plan.swaps.iter().enumerate() {
        before(index)?;
        let original = plan.identity.root.join(&swap.name);
        if path_exists(&original)
            .then(|| tree_hash(&original))
            .transpose()?
            != swap.old_hash
        {
            return Err("update_target_changed".into());
        }
        let incoming = work.join("new").join(&swap.name);
        if path_exists(&incoming)
            .then(|| tree_hash(&incoming))
            .transpose()?
            != swap.new_hash
        {
            return Err("update_staging_changed".into());
        }
        if swap.old_hash.is_some()
            && swap.new_hash.is_some()
            && fs::symlink_metadata(&original)
                .map_err(|e| e.to_string())?
                .is_file()
            && fs::symlink_metadata(&incoming)
                .map_err(|e| e.to_string())?
                .is_file()
        {
            replace_with_backup(&original, &incoming, &work.join("backup").join(&swap.name))?;
            continue;
        }
        if swap.old_hash.is_some() {
            fs::rename(&original, work.join("backup").join(&swap.name))
                .map_err(|e| e.to_string())?;
        }
        if swap.new_hash.is_some() {
            fs::rename(&incoming, &original).map_err(|e| e.to_string())?;
        }
    }
    plan.phase = "applied".into();
    write_json(&work.join("apply.json"), plan)
}

#[cfg(windows)]
struct Parent(windows::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl Drop for Parent {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
#[cfg(windows)]
fn open_parent(pid: u32) -> Result<Parent, String> {
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE};
    unsafe {
        OpenProcess(PROCESS_SYNCHRONIZE, false, pid)
            .map(Parent)
            .map_err(|error| {
                if error.code()
                    == windows::core::HRESULT::from_win32(
                        windows::Win32::Foundation::ERROR_INVALID_PARAMETER.0,
                    )
                {
                    "update_parent_process_exited".into()
                } else {
                    "update_parent_process_unavailable".into()
                }
            })
    }
}
#[cfg(windows)]
fn wait_parent(parent: Parent) -> Result<(), String> {
    use windows::Win32::System::Threading::WaitForSingleObject;
    if unsafe { WaitForSingleObject(parent.0, 60_000) } == windows::Win32::Foundation::WAIT_OBJECT_0
    {
        Ok(())
    } else {
        Err("update_parent_exit_timeout".into())
    }
}
#[cfg(unix)]
fn open_parent(pid: u32) -> Result<u32, String> {
    if pid == 0 || pid > i32::MAX as u32 {
        Err("invalid_update_parent".into())
    } else {
        Ok(pid)
    }
}
#[cfg(unix)]
fn wait_parent(pid: u32) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        if unsafe { libc::kill(pid as i32, 0) } == -1
            && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
        {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err("update_parent_exit_timeout".into())
}

fn run_helper(journal: &Path, recover: bool) -> Result<(), String> {
    if journal.file_name().and_then(|v| v.to_str()) != Some("apply.json") {
        return Err("invalid_update_journal_path".into());
    }
    regular(journal)?;
    let journal_path = journal.canonicalize().map_err(|e| e.to_string())?;
    let journal = journal_path.as_path();
    let work = journal.parent().ok_or("invalid_update_journal_path")?;
    let mut plan: ApplyPlan =
        serde_json::from_slice(&read_small(journal)?).map_err(|_| "invalid_update_journal")?;
    validate_plan(&plan, work)?;
    let expected_helper = if cfg!(windows) {
        work.join("helper.exe")
    } else {
        plan.identity.root.join(&plan.identity.executable)
    };
    if std::env::current_exe()
        .map_err(|e| e.to_string())?
        .canonicalize()
        .map_err(|e| e.to_string())?
        != expected_helper
    {
        return Err("invalid_update_helper_location".into());
    }
    #[cfg(windows)]
    if file_hash(&expected_helper)? != plan.executable_hash {
        return Err("update_helper_digest_mismatch".into());
    }
    if plan.parent_pid == std::process::id() {
        return Err("invalid_update_parent".into());
    }
    let target_guard = target_lock(&plan.identity.root)?;
    let parent = match open_parent(plan.parent_pid) {
        Ok(parent) => Some(parent),
        Err(error) if recover && error == "update_parent_process_exited" => None,
        Err(error) => return Err(error),
    };
    create_file(&work.join("ready"))?
        .sync_all()
        .map_err(|e| e.to_string())?;
    let owner = match parent.map_or(Ok(()), wait_parent).and_then(|_| {
        crate::process_guard::acquire(&plan.db_path)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "update_database_is_in_use".into())
    }) {
        Ok(owner) => owner,
        Err(error) => {
            plan.error = Some(error.clone());
            write_json(journal, &plan)?;
            return Err(error);
        }
    };
    let operation = (|| {
        if recover {
            plan.error
                .get_or_insert_with(|| "update_interrupted_and_restored".into());
            return rollback(&mut plan, work);
        }
        if plan.phase != "prepared"
            || file_hash(&plan.identity.root.join(&plan.identity.executable))?
                != plan.executable_hash
        {
            return Err("update_target_changed".into());
        }
        if plan.identity.kind == "portable" {
            apply_swaps(&mut plan, work, |_| Ok(()))
        } else {
            let installer = work.join("new").join(&plan.candidate.name);
            verify_file(&installer, plan.candidate.size, &plan.candidate.sha256)?;
            let mut command = Command::new(installer);
            command.arg("/UPDATE").arg("/P");
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                let native_root = plan.identity.root.to_string_lossy();
                command.raw_arg(format!(
                    "/D={}",
                    native_root.strip_prefix(r"\\?\").unwrap_or(&native_root)
                ));
            }
            let mut child = command
                .spawn()
                .map_err(|_| "update_installer_launch_failed")?;
            let deadline = Instant::now() + Duration::from_secs(600);
            loop {
                if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                    if !status.success() {
                        return Err(format!(
                            "update_installer_exit_{}",
                            status.code().unwrap_or(-1)
                        ));
                    }
                    break;
                }
                if Instant::now() >= deadline {
                    return Err("update_installer_result_unknown".into());
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            #[cfg(windows)]
            if installed_registration(&plan.identity.root.join(&plan.identity.executable))?
                .is_none_or(|(root, version)| {
                    root != plan.identity.root || version != plan.candidate.version
                })
            {
                return Err("update_installed_version_mismatch".into());
            }
            plan.phase = "installer_finished".into();
            write_json(journal, &plan)
        }
    })();
    if let Err(error) = operation {
        plan.error = Some(error.clone());
        if plan.identity.kind == "portable"
            && ["applying", "applied"].contains(&plan.phase.as_str())
        {
            if let Err(recovery) = rollback(&mut plan, work) {
                plan.error = Some(format!("{error}; {recovery}"));
                let _ = write_json(journal, &plan);
                return Err(plan.error.unwrap());
            }
        } else {
            plan.phase = "failed".into();
            write_json(journal, &plan)?;
        }
    }
    drop(owner);
    drop(target_guard);
    let mut command = Command::new(plan.identity.root.join(&plan.identity.entrypoint));
    command
        .arg("--data-dir")
        .arg(plan.db_path.parent().ok_or("invalid_update_database")?);
    hidden(&mut command);
    if command.spawn().is_err() {
        plan.error = Some("update_relaunch_failed".into());
        if plan.identity.kind == "portable" && plan.phase == "applied" {
            let _guard = target_lock(&plan.identity.root)?;
            let _owner = crate::process_guard::acquire(&plan.db_path)
                .map_err(|e| e.to_string())?
                .ok_or("update_database_is_in_use")?;
            rollback(&mut plan, work)?;
        }
        write_json(journal, &plan)?;
        let _ = command.spawn();
        return Err("update_relaunch_failed".into());
    }
    Ok(())
}

pub(crate) fn helper_entry() -> bool {
    let args: Vec<_> = std::env::args_os().collect();
    #[cfg(windows)]
    if args.len() == 1 {
        if let Ok(executable) = std::env::current_exe() {
            if executable
                .file_name()
                .is_some_and(|name| name == "helper.exe")
                && executable
                    .parent()
                    .and_then(Path::file_name)
                    .is_some_and(|name| name == WORK)
            {
                let journal = executable.parent().unwrap().join("apply.json");
                let result = (|| {
                    let plan: ApplyPlan = serde_json::from_slice(&read_small(&journal)?)
                        .map_err(|_| "invalid_update_journal")?;
                    if !["prepared", "applying"].contains(&plan.phase.as_str())
                        || plan.identity.kind != "portable"
                    {
                        return Err("no_interrupted_portable_update".into());
                    }
                    run_helper(&journal, true)
                })();
                if let Err(error) = result {
                    eprintln!("{error}");
                    rfd::MessageDialog::new()
                        .set_title("LLM Usage update")
                        .set_description(format!("Update recovery failed / 更新恢复失败: {error}"))
                        .show();
                    std::process::exit(1);
                }
                return true;
            }
        }
    }
    if args.len() < 2
        || !["--apply-update", "--recover-update"]
            .iter()
            .any(|v| args[1] == *v)
    {
        return false;
    }
    let result = if args.len() == 3 {
        run_helper(Path::new(&args[2]), args[1] == "--recover-update")
    } else {
        Err("invalid_update_helper_arguments".into())
    };
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
    true
}

pub(crate) fn previous_error(identity: &PackageIdentity) -> Result<Option<String>, String> {
    let work = identity.root.join(WORK);
    let journal = work.join("apply.json");
    if !journal.try_exists().map_err(|e| e.to_string())? {
        return Ok(None);
    }
    let plan: ApplyPlan =
        serde_json::from_slice(&read_small(&journal)?).map_err(|_| "invalid_update_journal")?;
    validate_plan(&plan, &work)?;
    if plan.identity.root != identity.root || plan.identity.executable != identity.executable {
        return Err("update_journal_identity_mismatch".into());
    }
    Ok(plan.error)
}

pub(crate) fn recover_before_startup(headless: bool) -> Result<bool, String> {
    let executable = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    for root in executable.ancestors().skip(1).take(8) {
        let work = root.join(WORK);
        let journal = work.join("apply.json");
        if !journal.exists() {
            continue;
        }
        let mut plan: ApplyPlan =
            serde_json::from_slice(&read_small(&journal)?).map_err(|_| "invalid_update_journal")?;
        validate_plan(&plan, &work)?;
        if plan.identity.root.join(&plan.identity.executable) != executable {
            continue;
        }
        let guard = target_lock(root)?;
        if ["prepared", "applying"].contains(&plan.phase.as_str())
            && plan.identity.kind == "portable"
        {
            if headless {
                return Err("update_recovery_requires_gui_startup".into());
            }
            plan.parent_pid = std::process::id();
            plan.error
                .get_or_insert_with(|| "update_interrupted_and_restored".into());
            write_json(&journal, &plan)?;
            drop(guard);
            launch_helper(&work, true)?;
            return Ok(true);
        }
        return Ok(false);
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Workspace(PathBuf);
    impl Workspace {
        fn new() -> Self {
            let base =
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/application-updates/rust");
            fs::create_dir_all(&base).unwrap();
            let path = base.join(format!(
                "{}-{}-{}",
                std::process::id(),
                crate::scanner::now_ms(),
                NEXT.fetch_add(1, Ordering::SeqCst)
            ));
            fs::create_dir(&path).unwrap();
            Self(path.canonicalize().unwrap())
        }
    }
    impl Drop for Workspace {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn pe(arch: &str) -> Vec<u8> {
        let mut bytes = vec![0; 256];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[60..64].copy_from_slice(&128u32.to_le_bytes());
        bytes[128..132].copy_from_slice(b"PE\0\0");
        bytes[132..134]
            .copy_from_slice(&(if arch == "x64" { 0x8664u16 } else { 0xaa64 }).to_le_bytes());
        bytes
    }

    fn archive(work: &Path, entries: &[(&str, &[u8], tar::EntryType, Option<&str>)]) -> PathBuf {
        let path = work.join("package.tar.zst");
        let encoder = zstd::Encoder::new(File::create(&path).unwrap(), 1).unwrap();
        let mut builder = tar::Builder::new(encoder);
        for (name, bytes, kind, link) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_path(name).unwrap();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o755);
            header.set_entry_type(*kind);
            if let Some(link) = link {
                header.set_link_name(link).unwrap();
            }
            header.set_cksum();
            builder.append(&header, *bytes).unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap();
        path
    }

    #[test]
    fn archive_paths_links_duplicates_and_special_entries_are_rejected() {
        for value in [
            "../a", "/a", "C:/a", "a\\b", "a//b", "a/./b", "a/../b", "CON", "a:", "a.",
        ] {
            assert!(safe_relative(value).is_err(), "{value}");
        }
        assert!(safe_link(Path::new("usr/lib/link"), "../../bin/native").is_ok());
        assert!(safe_link(Path::new("usr/lib/link"), "../../../outside").is_err());
        assert!(safe_link(Path::new("link"), "/outside").is_err());
        for entries in [
            vec![
                ("pkg/file", &b"one"[..], tar::EntryType::Regular, None),
                ("pkg/file", &b"two"[..], tar::EntryType::Regular, None),
            ],
            vec![("other/file", &b"one"[..], tar::EntryType::Regular, None)],
            vec![(
                "pkg/.llmusage-update/plan",
                &b"one"[..],
                tar::EntryType::Regular,
                None,
            )],
            vec![(
                "pkg/link",
                &b""[..],
                tar::EntryType::Symlink,
                Some("../outside"),
            )],
            vec![("pkg/link", &b""[..], tar::EntryType::Link, Some("pkg/file"))],
            vec![("pkg/pipe", &b""[..], tar::EntryType::Fifo, None)],
        ] {
            let work = Workspace::new();
            let path = archive(&work.0, &entries);
            let output = work.0.join("out");
            fs::create_dir(&output).unwrap();
            assert!(extract_archive(&path, &output, "pkg", &|| Ok(())).is_err());
        }
    }

    #[test]
    fn portable_identity_architecture_inventory_and_cancellation_are_checked() {
        let work = Workspace::new();
        let mut old = PackageIdentity::test_portable("windows", "x64");
        old.root = work.0.clone();
        let mut new = old.clone();
        new.version = "0.3.0".into();
        new.root = PathBuf::new();
        let name = old.asset_name("0.3.0").unwrap();
        let root = name.trim_end_matches(".tar.zst");
        let bytes = pe("x64");
        let marker = serde_json::to_vec(&new).unwrap();
        let path = archive(
            &work.0,
            &[
                (
                    &format!("{root}/LLMUsage.exe"),
                    &bytes,
                    tar::EntryType::Regular,
                    None,
                ),
                (
                    &format!("{root}/{MARKER}"),
                    &marker,
                    tar::EntryType::Regular,
                    None,
                ),
            ],
        );
        let candidate = Candidate {
            version: "0.3.0".into(),
            name: name.clone(),
            size: fs::metadata(&path).unwrap().len(),
            sha256: file_hash(&path).unwrap(),
            url: format!("https://github.com/owent/llm-usage/releases/download/v0.3.0/{name}"),
        };
        fs::rename(path, work.0.join(&name)).unwrap();
        assert!(validate_download(&old, &candidate, &work.0, || Ok(())).is_ok());
        assert!(
            validate_download(&old, &candidate, &work.0, || Err("update_cancelled".into()))
                .is_err()
        );
        assert!(!work.0.join("validation").exists());
        assert!(verify_file(&work.0.join(&name), candidate.size + 1, &candidate.sha256).is_err());
        assert!(verify_file(&work.0.join(&name), candidate.size, &"a".repeat(64)).is_err());
        old.arch = "arm64".into();
        assert!(validate_download(&old, &candidate, &work.0, || Ok(())).is_err());
    }

    fn plan(work: &Path) -> ApplyPlan {
        let root = work.to_path_buf();
        let job = root.join(WORK);
        fs::create_dir_all(job.join("new")).unwrap();
        let mut identity = PackageIdentity::test_portable(platform(), arch());
        identity.root = root.clone();
        for (name, old, new) in [
            ("LLMUsage.exe", Some("old-binary"), Some("new-binary")),
            (MARKER, Some("old-marker"), Some("new-marker")),
            ("removed", Some("obsolete"), None),
            ("added", None, Some("introduced")),
        ] {
            if let Some(bytes) = old {
                fs::write(root.join(name), bytes).unwrap();
            }
            if let Some(bytes) = new {
                fs::write(job.join("new").join(name), bytes).unwrap();
            }
        }
        fs::write(root.join("personal.txt"), "retained").unwrap();
        let swaps = ["LLMUsage.exe", "removed", "added", MARKER]
            .iter()
            .map(|name| Swap {
                name: name.to_string(),
                old_hash: path_exists(&root.join(name))
                    .then(|| tree_hash(&root.join(name)).unwrap()),
                new_hash: path_exists(&job.join("new").join(name))
                    .then(|| tree_hash(&job.join("new").join(name)).unwrap()),
            })
            .collect();
        let candidate = Candidate {
            version: "0.3.0".into(),
            name: identity.asset_name("0.3.0").unwrap(),
            size: 100,
            sha256: "a".repeat(64),
            url: format!(
                "https://github.com/owent/llm-usage/releases/download/v0.3.0/{}",
                identity.asset_name("0.3.0").unwrap()
            ),
        };
        ApplyPlan {
            schema: 1,
            identity,
            candidate,
            executable_hash: file_hash(&root.join("LLMUsage.exe")).unwrap(),
            db_path: root.join("llm-usage.sqlite"),
            parent_pid: std::process::id(),
            phase: "prepared".into(),
            swaps,
            error: None,
        }
    }

    #[test]
    fn interrupted_file_swaps_roll_back_and_preserve_unmanaged_files() {
        for fail_at in 0..4 {
            let work = Workspace::new();
            let mut plan = plan(&work.0);
            let job = work.0.join(WORK);
            assert!(apply_swaps(&mut plan, &job, |index| if index == fail_at {
                Err("simulated_failure".into())
            } else {
                Ok(())
            })
            .is_err());
            let bytes = read_small(&job.join("apply.json")).unwrap();
            let mut persisted: ApplyPlan = serde_json::from_slice(&bytes).unwrap();
            rollback(&mut persisted, &job).unwrap();
            rollback(&mut persisted, &job).unwrap();
            assert_eq!(
                fs::read_to_string(work.0.join("LLMUsage.exe")).unwrap(),
                "old-binary"
            );
            assert_eq!(
                fs::read_to_string(work.0.join(MARKER)).unwrap(),
                "old-marker"
            );
            assert_eq!(
                fs::read_to_string(work.0.join("removed")).unwrap(),
                "obsolete"
            );
            assert!(!work.0.join("added").exists());
            assert_eq!(
                fs::read_to_string(work.0.join("personal.txt")).unwrap(),
                "retained"
            );
        }
    }

    #[test]
    fn successful_swaps_keep_backup_and_refuse_modified_recovery_targets() {
        let work = Workspace::new();
        let mut plan = plan(&work.0);
        let job = work.0.join(WORK);
        apply_swaps(&mut plan, &job, |_| Ok(())).unwrap();
        assert_eq!(
            fs::read_to_string(work.0.join("LLMUsage.exe")).unwrap(),
            "new-binary"
        );
        assert_eq!(
            fs::read_to_string(job.join("backup/LLMUsage.exe")).unwrap(),
            "old-binary"
        );
        assert_eq!(
            fs::read_to_string(work.0.join("added")).unwrap(),
            "introduced"
        );
        assert!(!work.0.join("removed").exists());
        fs::write(work.0.join(MARKER), "user-modified").unwrap();
        assert_eq!(
            rollback(&mut plan, &job).unwrap_err(),
            "update_target_modified_recovery_refused"
        );
        assert_eq!(
            fs::read_to_string(work.0.join(MARKER)).unwrap(),
            "user-modified"
        );
    }

    #[test]
    fn target_lock_and_journal_validation_prevent_wrong_paths_and_downgrades() {
        let work = Workspace::new();
        let guard = target_lock(&work.0).unwrap();
        assert!(target_lock(&work.0).is_err());
        drop(guard);
        assert!(target_lock(&work.0).is_ok());
        let mut plan = plan(&work.0);
        let job = work.0.join(WORK);
        validate_plan(&plan, &job).unwrap();
        plan.candidate.version = "0.2.2".into();
        plan.candidate.name = plan.identity.asset_name("0.2.2").unwrap();
        plan.candidate.url = format!(
            "https://github.com/owent/llm-usage/releases/download/v0.2.2/{}",
            plan.candidate.name
        );
        assert!(validate_plan(&plan, &job).is_err());
        plan.identity.root = work.0.join("wrong");
        assert!(validate_plan(&plan, &job).is_err());
    }

    #[test]
    fn previous_failures_survive_rollback_and_reject_foreign_journals() {
        let work = Workspace::new();
        let mut plan = plan(&work.0);
        let job = work.0.join(WORK);
        plan.error = Some("update_parent_exit_timeout".into());
        rollback(&mut plan, &job).unwrap();
        assert_eq!(
            previous_error(&plan.identity).unwrap().as_deref(),
            Some("update_parent_exit_timeout")
        );
        let mut foreign = plan.identity.clone();
        foreign.executable = "other.exe".into();
        assert_eq!(
            previous_error(&foreign).unwrap_err(),
            "update_journal_identity_mismatch"
        );
    }

    #[test]
    fn new_managed_entries_cannot_replace_existing_personal_files() {
        let work = Workspace::new();
        let plan = plan(&work.0);
        let mut next = plan.identity.clone();
        next.managed.push("brand-new.txt".into());
        reject_unmanaged_collisions(&plan.identity, &next).unwrap();
        next.managed.push("personal.txt".into());
        assert_eq!(
            reject_unmanaged_collisions(&plan.identity, &next).unwrap_err(),
            "update_would_overwrite_unmanaged_file"
        );
        assert_eq!(
            fs::read_to_string(work.0.join("personal.txt")).unwrap(),
            "retained"
        );
    }

    #[test]
    fn rollback_recovers_a_missing_original_and_a_duplicate_backup() {
        for missing in [false, true] {
            let work = Workspace::new();
            let mut plan = plan(&work.0);
            let job = work.0.join(WORK);
            fs::create_dir_all(job.join("backup")).unwrap();
            fs::copy(work.0.join("LLMUsage.exe"), job.join("backup/LLMUsage.exe")).unwrap();
            if missing {
                fs::remove_file(work.0.join("LLMUsage.exe")).unwrap();
            }
            rollback(&mut plan, &job).unwrap();
            assert_eq!(
                fs::read_to_string(work.0.join("LLMUsage.exe")).unwrap(),
                "old-binary"
            );
            assert!(!job.join("backup/LLMUsage.exe").exists());
        }
    }
}
