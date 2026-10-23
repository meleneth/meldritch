//! Control-thread sample resolution, persistent caching, and audio preparation.

use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fmt, fs,
    io::Cursor,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};

const MAX_MANIFEST_BYTES: usize = 2 * 1024 * 1024;
const MAX_SAMPLE_BYTES: usize = 32 * 1024 * 1024;
pub const STRUDEL_MANIFEST_VERSION: &str = "9eacfc86ec4393e68a463ff52b01c19cfaa77f38";
pub const STRUDEL_ASSET_VERSION: &str = "6577395a4d05031728ced2ec3e5637fa89d8be48";
const STRUDEL_MANIFEST_SHA256: &str =
    "0651f9335be81b75347ef4392ad3e20520c9935681d91092927f41669f240c94";
const STRUDEL_MANIFEST_URL: &str = "https://raw.githubusercontent.com/felixroos/dough-samples/9eacfc86ec4393e68a463ff52b01c19cfaa77f38/tidal-drum-machines.json";
const STRUDEL_ASSET_BASE: &str = "https://raw.githubusercontent.com/geikha/tidal-drum-machines/6577395a4d05031728ced2ec3e5637fa89d8be48/machines/";

static CACHE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone, Debug)]
pub struct PreparedSample {
    pub identity: String,
    pub source_version: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub left: Arc<[f64]>,
    pub right: Arc<[f64]>,
}

impl PreparedSample {
    pub fn frames(&self) -> usize {
        self.left.len()
    }
}

#[derive(Clone, Debug)]
pub struct SampleBank {
    source: SampleSource,
    bank: String,
    cache: bool,
    project_root: PathBuf,
}

#[derive(Clone, Debug)]
enum SampleSource {
    BuiltinStrudel,
    LocalManifest(PathBuf),
    RemoteManifest {
        manifest: PathBuf,
        base_url: String,
        version: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SampleError {
    MissingSample(String),
    CorruptCache(String),
    UnsupportedFormat(String),
    Network(String),
    InvalidManifest(String),
    UnsafePath(String),
    Io(String),
}

impl fmt::Display for SampleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSample(value) => write!(f, "missing sample: {value}"),
            Self::CorruptCache(value) => write!(f, "corrupt sample cache: {value}"),
            Self::UnsupportedFormat(value) => write!(f, "unsupported sample format: {value}"),
            Self::Network(value) => write!(f, "sample network failure: {value}"),
            Self::InvalidManifest(value) => write!(f, "invalid sample manifest: {value}"),
            Self::UnsafePath(value) => write!(f, "unsafe sample path: {value}"),
            Self::Io(value) => write!(f, "sample I/O failure: {value}"),
        }
    }
}

impl std::error::Error for SampleError {}

#[derive(Debug, Deserialize)]
struct Manifest(BTreeMap<String, serde_json::Value>);

#[derive(Debug, Serialize, Deserialize)]
struct CacheMetadata {
    source_url: String,
    source_version: String,
    sha256: String,
    byte_len: usize,
}

impl SampleBank {
    pub fn strudel(bank: String, cache: bool, project_root: PathBuf) -> Result<Self, SampleError> {
        validate_name(&bank, "bank")?;
        Ok(Self {
            source: SampleSource::BuiltinStrudel,
            bank,
            cache,
            project_root,
        })
    }

    pub fn local(
        bank: String,
        manifest: PathBuf,
        cache: bool,
        project_root: PathBuf,
    ) -> Result<Self, SampleError> {
        validate_name(&bank, "bank")?;
        let path = confined_path(&project_root, &manifest)?;
        Ok(Self {
            source: SampleSource::LocalManifest(path),
            bank,
            cache,
            project_root,
        })
    }

    pub fn remote(
        bank: String,
        manifest: PathBuf,
        base_url: String,
        version: String,
        cache: bool,
        project_root: PathBuf,
    ) -> Result<Self, SampleError> {
        validate_name(&bank, "bank")?;
        let manifest = confined_path(&project_root, &manifest)?;
        if !base_url.starts_with("https://") || !base_url.ends_with('/') {
            return Err(SampleError::InvalidManifest(
                "remote sample base_url must be an https URL ending in '/'".into(),
            ));
        }
        if version.len() != 40 || !version.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(SampleError::InvalidManifest(
                "remote sample version must be a pinned 40-character commit hash".into(),
            ));
        }
        Ok(Self {
            source: SampleSource::RemoteManifest {
                manifest,
                base_url,
                version,
            },
            bank,
            cache,
            project_root,
        })
    }

    pub fn load(&self, name: &str, variant: usize) -> Result<Arc<PreparedSample>, SampleError> {
        validate_name(name, "sample")?;
        let key = format!("{}_{}", self.bank, name);
        let (manifest, base, version, remote) = self.manifest()?;
        let entries = manifest_entries(&manifest, &key)?;
        let relative = entries
            .get(variant)
            .ok_or_else(|| SampleError::MissingSample(format!("{key}:{variant}")))?;
        validate_relative_asset(relative)?;
        let (bytes, identity) = if remote {
            let url = format!("{base}{}", encode_relative_url(relative));
            let bytes = self.remote_asset(&url, &version, relative)?;
            (bytes, url)
        } else {
            let root = Path::new(&base);
            let path = confined_path(root, Path::new(relative))?;
            let bytes = fs::read(&path).map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    SampleError::MissingSample(path.display().to_string())
                } else {
                    SampleError::Io(format!("{}: {error}", path.display()))
                }
            })?;
            (bytes, path.display().to_string())
        };
        decode_audio(&bytes, relative, identity, version).map(Arc::new)
    }

    fn manifest(&self) -> Result<(Manifest, String, String, bool), SampleError> {
        match &self.source {
            SampleSource::BuiltinStrudel => {
                let bytes = cached_remote_bytes(
                    STRUDEL_MANIFEST_URL,
                    STRUDEL_MANIFEST_VERSION,
                    "manifests/tidal-drum-machines.json",
                    MAX_MANIFEST_BYTES,
                    Some(STRUDEL_MANIFEST_SHA256),
                )?;
                let manifest = parse_manifest(&bytes)?;
                Ok((
                    manifest,
                    STRUDEL_ASSET_BASE.into(),
                    STRUDEL_ASSET_VERSION.into(),
                    true,
                ))
            }
            SampleSource::LocalManifest(path) => {
                let bytes = fs::read(path)
                    .map_err(|error| SampleError::Io(format!("{}: {error}", path.display())))?;
                if bytes.len() > MAX_MANIFEST_BYTES {
                    return Err(SampleError::InvalidManifest("manifest is too large".into()));
                }
                let manifest = parse_manifest(&bytes)?;
                let parent = path.parent().unwrap_or(&self.project_root);
                let base = manifest
                    .0
                    .get("_base")
                    .and_then(serde_json::Value::as_str)
                    .map_or_else(|| parent.to_path_buf(), |value| parent.join(value));
                let base = confined_path(&self.project_root, &base)?;
                let hash = sha256(&bytes);
                Ok((manifest, base.display().to_string(), hash, false))
            }
            SampleSource::RemoteManifest {
                manifest: path,
                base_url,
                version,
            } => {
                let bytes = fs::read(path)
                    .map_err(|error| SampleError::Io(format!("{}: {error}", path.display())))?;
                if bytes.len() > MAX_MANIFEST_BYTES {
                    return Err(SampleError::InvalidManifest("manifest is too large".into()));
                }
                Ok((
                    parse_manifest(&bytes)?,
                    base_url.clone(),
                    version.clone(),
                    true,
                ))
            }
        }
    }

    fn remote_asset(
        &self,
        url: &str,
        version: &str,
        relative: &str,
    ) -> Result<Vec<u8>, SampleError> {
        if !self.cache {
            return download(url, MAX_SAMPLE_BYTES);
        }
        let cache_key = format!("assets/{version}/{}", safe_cache_relative(relative));
        cached_remote_bytes(url, version, &cache_key, MAX_SAMPLE_BYTES, None)
    }
}

fn parse_manifest(bytes: &[u8]) -> Result<Manifest, SampleError> {
    serde_json::from_slice(bytes).map_err(|error| SampleError::InvalidManifest(error.to_string()))
}

fn manifest_entries(manifest: &Manifest, key: &str) -> Result<Vec<String>, SampleError> {
    let value = manifest
        .0
        .get(key)
        .ok_or_else(|| SampleError::MissingSample(key.into()))?;
    match value {
        serde_json::Value::String(path) => Ok(vec![path.clone()]),
        serde_json::Value::Array(values) => values
            .iter()
            .map(|value| {
                value.as_str().map(str::to_owned).ok_or_else(|| {
                    SampleError::InvalidManifest(format!("sample '{key}' is not a path array"))
                })
            })
            .collect(),
        _ => Err(SampleError::InvalidManifest(format!(
            "sample '{key}' is not a path or path array"
        ))),
    }
}

fn validate_name(value: &str, kind: &str) -> Result<(), SampleError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(SampleError::UnsafePath(format!(
            "invalid {kind} name '{value}'"
        )));
    }
    Ok(())
}

fn validate_relative_asset(value: &str) -> Result<(), SampleError> {
    let path = Path::new(value);
    if value.is_empty()
        || value.contains('\\')
        || path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(SampleError::UnsafePath(value.into()));
    }
    if path
        .extension()
        .and_then(|value| value.to_str())
        .is_none_or(|extension| {
            !matches!(
                extension.to_ascii_lowercase().as_str(),
                "wav" | "flac" | "ogg"
            )
        })
    {
        return Err(SampleError::UnsupportedFormat(value.into()));
    }
    Ok(())
}

fn encode_relative_url(value: &str) -> String {
    value
        .split('/')
        .map(|segment| utf8_percent_encode(segment, NON_ALPHANUMERIC).to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn safe_cache_relative(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_') {
                char::from(byte)
            } else {
                '_'
            }
        })
        .collect()
}

fn cache_root() -> Result<PathBuf, SampleError> {
    let root = if let Some(value) = std::env::var_os("XDG_CACHE_HOME") {
        PathBuf::from(value)
    } else {
        PathBuf::from(
            std::env::var_os("HOME")
                .ok_or_else(|| SampleError::Io("HOME and XDG_CACHE_HOME are unset".into()))?,
        )
        .join(".cache")
    };
    Ok(root.join("meldritch/samples"))
}

fn confined_path(root: &Path, relative: &Path) -> Result<PathBuf, SampleError> {
    if relative.is_absolute() {
        if relative.starts_with(root) {
            return Ok(relative.to_path_buf());
        }
        return Err(SampleError::UnsafePath(relative.display().to_string()));
    }
    if relative
        .components()
        .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
    {
        return Err(SampleError::UnsafePath(relative.display().to_string()));
    }
    Ok(root.join(relative))
}

fn cached_remote_bytes(
    url: &str,
    version: &str,
    relative: &str,
    maximum: usize,
    expected_hash: Option<&str>,
) -> Result<Vec<u8>, SampleError> {
    cached_remote_bytes_at(
        &cache_root()?,
        url,
        version,
        relative,
        maximum,
        expected_hash,
    )
}

fn cached_remote_bytes_at(
    root: &Path,
    url: &str,
    version: &str,
    relative: &str,
    maximum: usize,
    expected_hash: Option<&str>,
) -> Result<Vec<u8>, SampleError> {
    validate_relative_asset_or_manifest(relative)?;
    let path = root.join(relative);
    let metadata_path = path.with_extension(format!(
        "{}.metadata.json",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("bin")
    ));
    let _guard = CACHE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| SampleError::Io("sample cache lock poisoned".into()))?;
    if path.exists() || metadata_path.exists() {
        let bytes = fs::read(&path)
            .map_err(|error| SampleError::CorruptCache(format!("{}: {error}", path.display())))?;
        let metadata: CacheMetadata =
            serde_json::from_slice(&fs::read(&metadata_path).map_err(|error| {
                SampleError::CorruptCache(format!("{}: {error}", metadata_path.display()))
            })?)
            .map_err(|error| SampleError::CorruptCache(error.to_string()))?;
        let actual = sha256(&bytes);
        if bytes.len() != metadata.byte_len
            || actual != metadata.sha256
            || metadata.source_url != url
            || metadata.source_version != version
            || expected_hash.is_some_and(|expected| expected != actual)
        {
            return Err(SampleError::CorruptCache(path.display().to_string()));
        }
        return Ok(bytes);
    }
    let bytes = download(url, maximum)?;
    let hash = sha256(&bytes);
    if expected_hash.is_some_and(|expected| expected != hash) {
        return Err(SampleError::CorruptCache(format!(
            "downloaded content hash for {url}"
        )));
    }
    let metadata = serde_json::to_vec_pretty(&CacheMetadata {
        source_url: url.into(),
        source_version: version.into(),
        sha256: hash,
        byte_len: bytes.len(),
    })
    .map_err(|error| SampleError::Io(error.to_string()))?;
    atomic_write(&path, &bytes)?;
    if let Err(error) = atomic_write(&metadata_path, &metadata) {
        let _ = fs::remove_file(&path);
        return Err(error);
    }
    Ok(bytes)
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        thread,
    };

    fn temp(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("meldritch-sample-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
    fn server(body: Vec<u8>, requests: Arc<AtomicUsize>) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut request = [0; 1024];
                let _ = stream.read(&mut request);
                requests.fetch_add(1, Ordering::SeqCst);
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(&body).unwrap();
            }
        });
        (format!("http://{address}/sample.wav"), handle)
    }
    #[test]
    fn first_request_downloads_and_second_request_is_offline() {
        let root = temp("offline");
        let count = Arc::new(AtomicUsize::new(0));
        let (url, thread) = server(b"audio".to_vec(), Arc::clone(&count));
        let first = cached_remote_bytes_at(&root, &url, "v1", "asset.wav", 100, None).unwrap();
        thread.join().unwrap();
        let second = cached_remote_bytes_at(&root, &url, "v1", "asset.wav", 100, None).unwrap();
        assert_eq!(first, b"audio");
        assert_eq!(second, first);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn corrupt_and_retargeted_cache_entries_are_rejected() {
        let root = temp("corrupt");
        let count = Arc::new(AtomicUsize::new(0));
        let (url, thread) = server(b"audio".to_vec(), count);
        cached_remote_bytes_at(&root, &url, "pinned", "asset.wav", 100, None).unwrap();
        thread.join().unwrap();
        fs::write(root.join("asset.wav"), b"changed").unwrap();
        assert!(matches!(
            cached_remote_bytes_at(&root, &url, "pinned", "asset.wav", 100, None),
            Err(SampleError::CorruptCache(_))
        ));
        fs::write(root.join("asset.wav"), b"audio").unwrap();
        assert!(matches!(
            cached_remote_bytes_at(&root, "http://different", "pinned", "asset.wav", 100, None),
            Err(SampleError::CorruptCache(_))
        ));
    }
    #[test]
    fn concurrent_requests_share_one_atomic_cache_entry() {
        let root = Arc::new(temp("concurrent"));
        let count = Arc::new(AtomicUsize::new(0));
        let (url, server) = server(vec![7; 4096], Arc::clone(&count));
        let url = Arc::new(url);
        let workers: Vec<_> = (0..4)
            .map(|_| {
                let root = Arc::clone(&root);
                let url = Arc::clone(&url);
                thread::spawn(move || {
                    cached_remote_bytes_at(&root, &url, "v1", "asset.wav", 8192, None).unwrap()
                })
            })
            .collect();
        for worker in workers {
            assert_eq!(worker.join().unwrap(), vec![7; 4096]);
        }
        server.join().unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn cache_paths_cannot_escape_and_formats_are_specific() {
        let root = temp("traversal");
        assert!(matches!(
            cached_remote_bytes_at(&root, "http://unused", "v", "../escape.wav", 10, None),
            Err(SampleError::UnsafePath(_))
        ));
        assert!(matches!(
            validate_relative_asset("sample.mp3"),
            Err(SampleError::UnsupportedFormat(_))
        ));
        assert!(matches!(
            manifest_entries(&Manifest(BTreeMap::new()), "missing"),
            Err(SampleError::MissingSample(_))
        ));
        assert!(validate_relative_asset("kit/hit.flac").is_ok());
        assert!(validate_relative_asset("field/air.ogg").is_ok());
    }
    #[test]
    fn remote_manifests_require_https_and_an_immutable_revision() {
        let root = temp("remote-validation");
        assert!(
            SampleBank::remote(
                "Bank".into(),
                "manifest.json".into(),
                "https://example.invalid/audio/".into(),
                "0123456789012345678901234567890123456789".into(),
                true,
                root.clone(),
            )
            .is_ok()
        );
        assert!(
            SampleBank::remote(
                "Bank".into(),
                "manifest.json".into(),
                "http://example.invalid/audio/".into(),
                "main".into(),
                true,
                root,
            )
            .is_err()
        );
    }
    #[test]
    fn wav_decoder_preserves_stereo_channels() {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(
                &mut cursor,
                hound::WavSpec {
                    channels: 2,
                    sample_rate: 24_000,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            writer.write_sample::<i16>(16384).unwrap();
            writer.write_sample::<i16>(-16384).unwrap();
            writer.finalize().unwrap();
        }
        let sample = decode_audio(
            &cursor.into_inner(),
            "fixture.wav",
            "fixture".into(),
            "v1".into(),
        )
        .unwrap();
        assert_eq!(sample.channels, 2);
        assert_eq!(sample.sample_rate, 24_000);
        assert!(sample.left[0] > 0.49);
        assert!(sample.right[0] < -0.49);
    }
    #[test]
    #[ignore = "optional real-upstream integration check"]
    fn pinned_upstream_manifest_and_909_sample_are_available() {
        let root = temp("upstream");
        let manifest = cached_remote_bytes_at(
            &root,
            STRUDEL_MANIFEST_URL,
            STRUDEL_MANIFEST_VERSION,
            "manifest.json",
            MAX_MANIFEST_BYTES,
            Some(STRUDEL_MANIFEST_SHA256),
        )
        .unwrap();
        assert!(
            !manifest_entries(&parse_manifest(&manifest).unwrap(), "RolandTR909_bd")
                .unwrap()
                .is_empty()
        );
    }
}

fn validate_relative_asset_or_manifest(value: &str) -> Result<(), SampleError> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(SampleError::UnsafePath(value.into()));
    }
    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), SampleError> {
    let parent = path
        .parent()
        .ok_or_else(|| SampleError::Io("cache path has no parent".into()))?;
    fs::create_dir_all(parent)
        .map_err(|error| SampleError::Io(format!("{}: {error}", parent.display())))?;
    let temporary = path.with_extension(format!(
        "{}.tmp-{}-{}",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("bin"),
        std::process::id(),
        std::thread::current().name().unwrap_or("thread")
    ));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = options
        .open(&temporary)
        .map_err(|error| SampleError::Io(format!("{}: {error}", temporary.display())))?;
    let result = (|| {
        use std::io::Write;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| SampleError::Io(error.to_string()))?;
        drop(file);
        fs::rename(&temporary, path)
            .map_err(|error| SampleError::Io(format!("{}: {error}", path.display())))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn download(url: &str, maximum: usize) -> Result<Vec<u8>, SampleError> {
    let mut response = ureq::get(url)
        .call()
        .map_err(|error| SampleError::Network(format!("{url}: {error}")))?;
    response
        .body_mut()
        .with_config()
        .limit(maximum as u64)
        .read_to_vec()
        .map_err(|error| SampleError::Network(format!("{url}: {error}")))
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn decode_audio(
    bytes: &[u8],
    relative: &str,
    identity: String,
    source_version: String,
) -> Result<PreparedSample, SampleError> {
    match Path::new(relative)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("wav") => decode_wav(bytes, identity, source_version),
        Some("flac") => decode_flac(bytes, identity, source_version),
        Some("ogg") => decode_ogg(bytes, identity, source_version),
        _ => Err(SampleError::UnsupportedFormat(relative.into())),
    }
}

fn prepared(
    identity: String,
    source_version: String,
    sample_rate: u32,
    channels: u16,
    values: Vec<f64>,
) -> Result<PreparedSample, SampleError> {
    if !matches!(channels, 1 | 2)
        || values.is_empty()
        || !values.len().is_multiple_of(usize::from(channels))
    {
        return Err(SampleError::UnsupportedFormat(format!(
            "{identity}: invalid {channels}-channel audio frames"
        )));
    }
    let (left, right) = if channels == 1 {
        (values.clone(), values)
    } else {
        let mut left = Vec::with_capacity(values.len() / 2);
        let mut right = Vec::with_capacity(values.len() / 2);
        for frame in values.as_chunks::<2>().0 {
            left.push(frame[0]);
            right.push(frame[1]);
        }
        (left, right)
    };
    Ok(PreparedSample {
        identity,
        source_version,
        sample_rate,
        channels,
        left: left.into(),
        right: right.into(),
    })
}

fn decode_wav(
    bytes: &[u8],
    identity: String,
    source_version: String,
) -> Result<PreparedSample, SampleError> {
    let mut reader = hound::WavReader::new(Cursor::new(bytes))
        .map_err(|error| SampleError::UnsupportedFormat(format!("{identity}: {error}")))?;
    let spec = reader.spec();
    if !matches!(spec.channels, 1 | 2) {
        return Err(SampleError::UnsupportedFormat(format!(
            "{identity}: {} channels",
            spec.channels
        )));
    }
    let values = match spec.sample_format {
        hound::SampleFormat::Float if spec.bits_per_sample == 32 => reader
            .samples::<f32>()
            .map(|value| value.map(f64::from))
            .collect::<Result<Vec<_>, _>>(),
        hound::SampleFormat::Int if (1..=32).contains(&spec.bits_per_sample) => {
            let scale = 2.0_f64.powi(i32::from(spec.bits_per_sample) - 1);
            reader
                .samples::<i32>()
                .map(|value| value.map(|sample| f64::from(sample) / scale))
                .collect::<Result<Vec<_>, _>>()
        }
        _ => {
            return Err(SampleError::UnsupportedFormat(format!(
                "{identity}: {:?} {}-bit WAV",
                spec.sample_format, spec.bits_per_sample
            )));
        }
    }
    .map_err(|error| SampleError::UnsupportedFormat(format!("{identity}: {error}")))?;
    prepared(
        identity,
        source_version,
        spec.sample_rate,
        spec.channels,
        values,
    )
}

fn decode_flac(
    bytes: &[u8],
    identity: String,
    source_version: String,
) -> Result<PreparedSample, SampleError> {
    let mut reader = claxon::FlacReader::new(Cursor::new(bytes))
        .map_err(|error| SampleError::UnsupportedFormat(format!("{identity}: {error}")))?;
    let info = reader.streaminfo();
    let channels = u16::try_from(info.channels)
        .map_err(|_| SampleError::UnsupportedFormat(format!("{identity}: channel count")))?;
    let scale = 2.0_f64.powi(i32::try_from(info.bits_per_sample.saturating_sub(1)).unwrap_or(31));
    let values = reader
        .samples()
        .map(|value| value.map(|sample| f64::from(sample) / scale))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| SampleError::UnsupportedFormat(format!("{identity}: {error}")))?;
    prepared(identity, source_version, info.sample_rate, channels, values)
}

fn decode_ogg(
    bytes: &[u8],
    identity: String,
    source_version: String,
) -> Result<PreparedSample, SampleError> {
    let mut reader = lewton::inside_ogg::OggStreamReader::new(Cursor::new(bytes))
        .map_err(|error| SampleError::UnsupportedFormat(format!("{identity}: {error}")))?;
    let channels = u16::from(reader.ident_hdr.audio_channels);
    let sample_rate = reader.ident_hdr.audio_sample_rate;
    let mut values = Vec::new();
    while let Some(packet) = reader
        .read_dec_packet_itl()
        .map_err(|error| SampleError::UnsupportedFormat(format!("{identity}: {error}")))?
    {
        values.extend(packet.into_iter().map(|sample| f64::from(sample) / 32768.0));
    }
    prepared(identity, source_version, sample_rate, channels, values)
}
