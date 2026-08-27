//! GitHub release 更新检测与资产下载，供 CLI `update` 子命令与桌面端
//! （经边车）共享。
//!
//! 职责边界：本模块只承载纯业务——版本比较、release 响应解析、资产
//! 挑选、带进度的流式下载、字节原子落盘。检测时机调度（GUI 启动检查、
//! 24h 节流）与安装动作（运行 NSIS 向导）留在端侧。
//!
//! 通道说明：release 元数据与安装包字节都走 [`UpdateClient`] 抽象——
//! 检测用 10s 短超时 agent，下载用独立的长超时 agent（15s 连接 + 600s
//! 总超时），两者都由 [`UreqUpdateClient`] 提供；代理从环境变量读取。
//!
//! 版本语义：本项目用 CalVer（如 `26.8.4`），与三段数字比较器天然
//! 兼容；tag 解析失败按「无更新」处理，宁可不提示也不误报。

use std::convert::TryFrom;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::Deserialize;

/// 当前程序版本（与 CLI `--version` / 桌面端版本同源）。
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// release 所在仓库（owner/repo）。
pub const GITHUB_REPO: &str = "ONEGAYI/obsidian-export-desktop";

/// 下载大小上限（512MB）：CLI 压缩包为 MB 级、桌面 NSIS 安装包为几十
/// MB，超限视为远端异常，防御性拒绝。
const MAX_DOWNLOAD_BYTES: usize = 512 * 1024 * 1024;

/// 进度回调的最小间隔：再快的下载也至多每 200ms 上报一帧。
const PROGRESS_INTERVAL: Duration = Duration::from_millis(200);

/// 下载缓冲的预分配钳制：Content-Length 是远端声明、可被谎报，超出
/// 部分交给 Vec 倍增策略，避免异常声明一次兑现为巨额内存预留。
const PREALLOC_CAP: u64 = 64 * 1024 * 1024;

// ---- 类型 -----------------------------------------------------------------

/// 资产挑选意图：CLI 自更新挑当前平台的 cargo-dist 产物，桌面端经边车
/// 挑 NSIS 安装包。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AssetTarget {
    /// `obsidian-export-{target-triple}` 前缀的压缩包产物。
    Cli,
    /// 含 `setup` 的 `.exe` 安装包（NSIS，Windows 桌面端）。
    Desktop,
}

/// 一次检测的结果。
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::module_name_repetitions)]
#[non_exhaustive]
pub enum UpdateStatus {
    /// 仓库还没有任何 release（GitHub API 404）。
    NoRelease,
    /// 已是最新版本（含 tag 不规范无法比较——宁可不提示，不误报更新）。
    UpToDate,
    /// 有新版本。`asset` 为 `None` 表示该 release 没有匹配意图的资产，
    /// 端侧应引导去发布页（`html_url`）手动下载。
    Available {
        /// 去掉 v 前缀的版本号（如 "26.9.0"）。
        version: String,
        /// 发布页地址。
        html_url: String,
        /// release 说明（CHANGELOG 正文，可能为空）。
        notes: Option<String>,
        /// 选中的资产。
        asset: Option<ReleaseAsset>,
    },
}

/// release 附带的可下载资产。
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[allow(clippy::module_name_repetitions)]
#[non_exhaustive]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

/// 安装包下载进度。`total_bytes` 为 `None` 表示服务器未返回
/// Content-Length，调用方应展示不定总量进度；速率为从本次下载开始
/// 计算的平均字节/秒。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[allow(clippy::exhaustive_structs)]
pub struct DownloadProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub bytes_per_second: u64,
}

/// 下载进度接收端。回调应快速返回：CLI 刷新终端行，边车事件模式直接
/// 落 stdout（[`crate::main` 侧的 `print_line`] 保证管道安全）。
pub trait DownloadProgressReporter {
    fn report(&self, progress: DownloadProgress);
}

/// 检测/下载错误。
///
/// `Network` 与 `HttpStatus` 为瞬时类（自动场景可静默、手动场景提示后
/// 由用户择机重试——限流配额会随窗口滚动恢复）；`Parse` 为确定性
/// （远端响应结构异常，重试无意义）。
#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
#[non_exhaustive]
pub enum UpdateError {
    /// 网络层失败（连接/超时/TLS）。
    Network(String),
    /// HTTP 状态异常（非 200/404，典型如 GitHub 对共享出口 IP 的限流
    /// 403）：`status_text` 为主文案（"HTTP 403"），`detail` 为响应体
    /// message（限流原因等，`None` = 无可解析详情）。
    HttpStatus {
        status_text: String,
        detail: Option<String>,
    },
    /// release 响应解析失败（确定性）。
    Parse(String),
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network(message) => write!(f, "network error: {message}"),
            Self::HttpStatus { status_text, .. } => write!(f, "HTTP error: {status_text}"),
            Self::Parse(message) => write!(f, "failed to parse release info: {message}"),
        }
    }
}

impl std::error::Error for UpdateError {}

impl UpdateError {
    /// 是否瞬时（可静默重试）。
    #[must_use]
    pub const fn is_transient(&self) -> bool {
        !matches!(self, Self::Parse(_))
    }

    /// 完整错误文案：`HttpStatus` 有详情时以括号追加响应体 message，
    /// 其余变体同 `Display`。
    #[must_use]
    pub fn full_message(&self) -> String {
        match self {
            Self::HttpStatus {
                status_text,
                detail: Some(detail),
            } => format!("HTTP error: {status_text} ({detail})"),
            _ => self.to_string(),
        }
    }
}

/// 更新通道抽象：检测（GET 文本）与下载（流式字节），生产实现为
/// [`UreqUpdateClient`]，测试注入 mock。
pub trait UpdateClient {
    /// GET 一个 URL 并返回 `(HTTP 状态码, 响应体文本)`。网络层失败
    /// （连接/超时/TLS）返回 [`UpdateError::Network`]；任何 HTTP 状态
    /// （含 4xx/5xx）都正常返回，由调用方解释。
    fn get_text(&self, url: &str, headers: &[(&str, &str)]) -> Result<(u16, String), UpdateError>;

    /// 流式下载字节，进度经 `reporter` 回调（含首帧与终态）。
    fn download(
        &self,
        url: &str,
        reporter: &dyn DownloadProgressReporter,
    ) -> Result<Vec<u8>, UpdateError>;
}

// ---- 版本比较（手写三段比较，不引 semver 依赖） ---------------------------

/// 解析 `vX.Y.Z` / `X.Y.Z` 为三段数字；忽略 `-rc.1` / `+build` 等前后
/// 缀；任何一段非数字或段数不是 3 → `None`。
#[must_use]
pub fn parse_version(tag: &str) -> Option<(u64, u64, u64)> {
    let s = tag.trim().trim_start_matches(['v', 'V']);
    let s = s.split(['-', '+']).next()?.trim();
    let mut parts = s.split('.');
    let major: u64 = parts.next()?.parse().ok()?;
    let minor: u64 = parts.next()?.parse().ok()?;
    let patch: u64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// `remote` 是否严格新于 `current`；任一解析失败返回 `None`（不提示）。
#[must_use]
pub fn is_newer(remote: &str, current: &str) -> Option<bool> {
    Some(parse_version(remote)? > parse_version(current)?)
}

// ---- 资产挑选 -------------------------------------------------------------

/// 编译期平台 triple（与 cargo-dist 产物命名一致）。运行时无法探测的
/// 差异（如 libc 变体）在编译期由 `cfg!(target_env)` 区分。
#[must_use]
pub const fn current_target_triple() -> &'static str {
    if cfg!(target_os = "windows") {
        "x86_64-pc-windows-msvc"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "aarch64-apple-darwin"
    } else if cfg!(target_os = "macos") {
        "x86_64-apple-darwin"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        if cfg!(target_env = "musl") {
            "x86_64-unknown-linux-musl"
        } else {
            "x86_64-unknown-linux-gnu"
        }
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "aarch64") {
        if cfg!(target_env = "musl") {
            "aarch64-unknown-linux-musl"
        } else {
            "aarch64-unknown-linux-gnu"
        }
    } else {
        // 不在 cargo-dist 发布矩阵内的平台：匹配不到任何 CLI 资产，
        // 自然落入「引导发布页」分支。
        "unsupported"
    }
}

/// 按 [`AssetTarget`] 挑选资产。
///
/// `Cli`：名字以 `obsidian-export-{triple}.` 开头且不以 `.sha256` 结尾
/// （排除校验和副产物），不限定压缩扩展名（Windows 为 `.zip`，macOS/
/// Linux 为 `.tar.gz`，以实际 cargo-dist 产物为准）。
///
/// `Desktop`：名字含 `setup` 的 `.exe`（NSIS 安装包，大小写不敏感）。
#[must_use]
// 比较前已 to_ascii_lowercase 归一，不存在大小写漏配
#[allow(clippy::case_sensitive_file_extension_comparisons)]
pub fn pick_asset(assets: &[ReleaseAsset], target: AssetTarget) -> Option<ReleaseAsset> {
    match target {
        AssetTarget::Cli => {
            let prefix = format!("obsidian-export-{}.", current_target_triple());
            assets
                .iter()
                .find(|a| {
                    a.name.starts_with(&prefix) && !a.name.to_ascii_lowercase().ends_with(".sha256")
                })
                .cloned()
        }
        AssetTarget::Desktop => assets
            .iter()
            .find(|a| {
                let lower = a.name.to_ascii_lowercase();
                lower.ends_with(".exe") && lower.contains("setup")
            })
            .cloned(),
    }
}

// ---- 检测 -----------------------------------------------------------------

/// GitHub `releases/latest` 的完整 URL。
///
/// debug 构建（测试与本地开发）可用 `OBSIDIAN_EXPORT_UPDATE_API_BASE`
/// 环境变量把 API 指到本地 mock 服务；release 构建不含此读取路径，
/// 不存在被环境劫持的面。
#[must_use]
pub fn releases_latest_url() -> String {
    #[cfg(debug_assertions)]
    {
        if let Ok(base) = std::env::var("OBSIDIAN_EXPORT_UPDATE_API_BASE") {
            return releases_latest_url_with_base(Some(&base));
        }
    }
    releases_latest_url_with_base(None)
}

/// [`releases_latest_url`] 的纯函数核心：非法 base（非 `http` 前缀）
/// 忽略并回退官方 API。
#[must_use]
fn releases_latest_url_with_base(base: Option<&str>) -> String {
    let official = || format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    base.map(str::trim)
        .filter(|b| b.starts_with("http"))
        .map_or_else(official, |b| {
            format!(
                "{}/repos/{GITHUB_REPO}/releases/latest",
                b.trim_end_matches('/')
            )
        })
}

/// GitHub `releases/latest` 响应中感兴趣的子集（多余字段忽略）。
#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    assets: Vec<ReleaseAsset>,
}

/// 查询 GitHub 最新 release 并与 [`VERSION`] 比较。
///
/// 请求 GitHub API 需带 User-Agent（API 硬性要求）与 vnd Accept；
/// 404 = 无 release（`releases/latest` 不含 draft/prerelease）。
pub fn check_update(
    client: &dyn UpdateClient,
    target: AssetTarget,
) -> Result<UpdateStatus, UpdateError> {
    let (status, body) = client.get_text(
        &releases_latest_url(),
        &[
            ("User-Agent", &format!("obsidian-export/{VERSION}")),
            ("Accept", "application/vnd.github+json"),
        ],
    )?;
    match status {
        200 => {}
        404 => return Ok(UpdateStatus::NoRelease),
        // 限流/5xx 等归为网络类（端侧按瞬时处理）；主文案只透状态码，
        // 响应体 message（如限流 403 的 "API rate limit exceeded for
        // IP..."）作为 detail 结构化携带。
        status => {
            return Err(UpdateError::HttpStatus {
                status_text: format!("HTTP {status}"),
                detail: extract_error_message(&body),
            });
        }
    }
    let release: GithubRelease =
        serde_json::from_str(&body).map_err(|e| UpdateError::Parse(e.to_string()))?;
    match is_newer(&release.tag_name, VERSION) {
        Some(true) => Ok(UpdateStatus::Available {
            version: release
                .tag_name
                .trim()
                .trim_start_matches(['v', 'V'])
                .to_owned(),
            html_url: release.html_url,
            notes: release.body.filter(|s| !s.trim().is_empty()),
            asset: pick_asset(&release.assets, target),
        }),
        // 解析失败（tag 不规范）也归入 UpToDate：不误报
        _ => Ok(UpdateStatus::UpToDate),
    }
}

/// 提取 GitHub 错误响应体的 `message` 字段；非 JSON / 无 message / 空白
/// → `None`。按字符截断到 200 并加省略号，防异常响应塞超长文案刷屏。
fn extract_error_message(body: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct ErrorBody {
        #[serde(default)]
        message: String,
    }
    let message = serde_json::from_str::<ErrorBody>(body).ok()?.message;
    let message = message.trim();
    if message.is_empty() {
        return None;
    }
    let truncated: String = message.chars().take(200).collect();
    Some(if truncated.len() < message.len() {
        format!("{truncated}\u{2026}")
    } else {
        truncated
    })
}

// ---- 下载 -----------------------------------------------------------------

/// ureq 实现的更新通道。
///
/// 要点：检测走 10s 总超时 agent（不可达快速失败）；下载走独立 agent
/// （15s 连接超时 + 600s 总超时，302 默认跟随——资产 URL 会跳转到
/// `objects.githubusercontent.com`）；代理从环境变量读取（ureq 默认
/// `Proxy::try_from_system`）；512MB 上限（Content-Length 预检 + 实际
/// 字节数复检）。
pub struct UreqUpdateClient {
    check_agent: ureq::Agent,
    download_agent: ureq::Agent,
}

impl Default for UreqUpdateClient {
    fn default() -> Self {
        Self::new()
    }
}

impl UreqUpdateClient {
    #[must_use]
    pub fn new() -> Self {
        let ua = format!("obsidian-export/{VERSION}");
        Self {
            check_agent: ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(10))
                .user_agent(&ua)
                .build(),
            download_agent: ureq::AgentBuilder::new()
                .timeout_connect(Duration::from_secs(15))
                .timeout(Duration::from_secs(600))
                .user_agent(&ua)
                .build(),
        }
    }
}

impl UpdateClient for UreqUpdateClient {
    fn get_text(&self, url: &str, headers: &[(&str, &str)]) -> Result<(u16, String), UpdateError> {
        let mut request = self.check_agent.get(url);
        for (name, value) in headers {
            request = request.set(name, value);
        }
        match request.call() {
            // ureq 2.x 把非 2xx 状态也归入 Err(Error::Status)，此处与
            // Ok 同路径展平为 (status, body) 让上层解释语义。body 读取
            // 失败（中断/超 10MB 上限）归网络类：重试可能成功，不得
            // 让空串流进 JSON 解析被误分类为确定性 Parse。
            Ok(resp) | Err(ureq::Error::Status(_, resp)) => {
                let status = resp.status();
                let body = resp
                    .into_string()
                    .map_err(|e| UpdateError::Network(format!("failed to read body: {e}")))?;
                Ok((status, body))
            }
            Err(e) => Err(UpdateError::Network(e.to_string())),
        }
    }

    fn download(
        &self,
        url: &str,
        reporter: &dyn DownloadProgressReporter,
    ) -> Result<Vec<u8>, UpdateError> {
        let resp = self.download_agent.get(url).call().map_err(map_ureq_err)?;
        let total_bytes = resp
            .header("Content-Length")
            .and_then(|v| v.parse::<u64>().ok());
        let max_bytes = u64::try_from(MAX_DOWNLOAD_BYTES).unwrap_or(u64::MAX);
        if let Some(len) = total_bytes {
            if len > max_bytes {
                return Err(UpdateError::Network(format!(
                    "asset too large ({len} bytes, limit {MAX_DOWNLOAD_BYTES})"
                )));
            }
        }

        let mut reader = resp.into_reader();
        let cap = total_bytes
            .unwrap_or_default()
            .min(max_bytes)
            .min(PREALLOC_CAP);
        let mut bytes = Vec::with_capacity(usize::try_from(cap).unwrap_or_default());
        let mut chunk = vec![0_u8; 64 * 1024];
        let started = Instant::now();
        let mut last_report = started;
        reporter.report(DownloadProgress {
            downloaded_bytes: 0,
            total_bytes,
            bytes_per_second: 0,
        });

        loop {
            let n = reader
                .read(&mut chunk)
                .map_err(|e| UpdateError::Network(format!("download interrupted: {e}")))?;
            if n == 0 {
                break;
            }
            if bytes.len().saturating_add(n) > MAX_DOWNLOAD_BYTES {
                return Err(UpdateError::Network("asset exceeds size limit".to_owned()));
            }
            #[allow(clippy::indexing_slicing)]
            bytes.extend_from_slice(&chunk[..n]);
            let downloaded = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
            if last_report.elapsed() >= PROGRESS_INTERVAL {
                reporter.report(DownloadProgress {
                    downloaded_bytes: downloaded,
                    total_bytes,
                    bytes_per_second: bytes_per_second(downloaded, started.elapsed()),
                });
                last_report = Instant::now();
            }
        }

        // 完整性终检：服务器干净断流（FIN 而非 RST）会让 reader 提前
        // EOF 而非报错，截断的字节流不得被当作成功下载。
        if let Some(total) = total_bytes {
            if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != total {
                return Err(UpdateError::Network(format!(
                    "truncated download: got {} of {total} bytes",
                    bytes.len()
                )));
            }
        }

        let downloaded = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        reporter.report(DownloadProgress {
            downloaded_bytes: downloaded,
            total_bytes,
            bytes_per_second: bytes_per_second(downloaded, started.elapsed()),
        });
        Ok(bytes)
    }
}

/// 平均速率（字节/秒）。整数除法即可：进度展示精度到 KB/s 足够，
/// 不值得为此引入浮点；u128 乘法以 u64 计数上限衡量恒不溢出。
#[allow(clippy::integer_division, clippy::arithmetic_side_effects)]
fn bytes_per_second(downloaded_bytes: u64, elapsed: Duration) -> u64 {
    let millis = elapsed.as_millis();
    if millis == 0 {
        return 0;
    }
    let rate = (u128::from(downloaded_bytes) * 1_000) / millis;
    u64::try_from(rate).unwrap_or(u64::MAX)
}

fn map_ureq_err(e: ureq::Error) -> UpdateError {
    match e {
        // 下载 URL 的 4xx/5xx 不走 check 的 detail 提取路径：直连 CDN
        // 的失败以简短状态文案透出即可。
        ureq::Error::Status(code, _) => UpdateError::Network(format!("HTTP {code}")),
        ureq::Error::Transport(t) => UpdateError::Network(t.to_string()),
    }
}

// ---- 落盘 -----------------------------------------------------------------

/// 字节原子落盘：tmp（pid + 进程内序号防并发互踩）+ rename，失败清理
/// tmp。
pub fn write_atomic_bytes(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file_name = path.file_name().map_or_else(
        || "download".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    let tmp = dir.join(format!(
        "{}.{}.{}.tmp",
        file_name,
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    if let Err(e) = std::fs::write(&tmp, bytes) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// release 资产名写入侧校验：必须是「形态正常」的纯文件名。
///
/// 具体规则：不含路径分隔符/盘符冒号（杜绝 `..\` 上跳与 NTFS ADS 形
/// 态）、不含控制字符、基名不是 Windows 保留设备名（`NUL`/`CON` 等，
/// 落盘会失败）、不以点或空格结尾（Win32 会静默剥除，导致实际文件名
/// 与上报的 path 错位）。扩展名不在此限定（CLI 意图下载 `.zip`/
/// `.tar.gz`，桌面意图下载 `.exe`，运行安装器侧另有路径校验）。
#[must_use]
pub fn validate_asset_name(name: &str) -> bool {
    if name.is_empty()
        || name.contains(['/', '\\', ':'])
        || name.chars().any(char::is_control)
        || name.ends_with(['.', ' '])
    {
        return false;
    }
    // 保留设备名按基名比较（大小写不敏感，不带扩展名形态）。
    let stem = name.split('.').next().unwrap_or_default();
    !matches!(
        stem.to_ascii_uppercase().as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

// ---- 测试 -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::io::Write as _;
    use std::net::TcpListener;
    use std::sync::Mutex;

    use super::*;

    /// 按 (status, body) 应答的 mock；fail=true 模拟网络层失败。
    /// 捕获请求 URL 与 headers 供契约断言。
    type CapturedRequests = Vec<(String, Vec<(String, String)>)>;

    struct MockClient {
        status: u16,
        body: String,
        fail: bool,
        captured: Mutex<CapturedRequests>,
    }

    impl MockClient {
        fn ok(body: &str) -> Self {
            Self {
                status: 200,
                body: body.to_owned(),
                fail: false,
                captured: Mutex::new(Vec::new()),
            }
        }

        fn status_with_body(status: u16, body: &str) -> Self {
            Self {
                status,
                body: body.to_owned(),
                fail: false,
                captured: Mutex::new(Vec::new()),
            }
        }

        fn failing() -> Self {
            Self {
                status: 200,
                body: String::new(),
                fail: true,
                captured: Mutex::new(Vec::new()),
            }
        }
    }

    impl UpdateClient for MockClient {
        fn get_text(
            &self,
            url: &str,
            headers: &[(&str, &str)],
        ) -> Result<(u16, String), UpdateError> {
            let owned_headers: Vec<(String, String)> = headers
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect();
            self.captured
                .lock()
                .unwrap()
                .push((url.to_owned(), owned_headers));
            if self.fail {
                return Err(UpdateError::Network("mock network down".to_owned()));
            }
            Ok((self.status, self.body.clone()))
        }

        fn download(
            &self,
            _url: &str,
            _reporter: &dyn DownloadProgressReporter,
        ) -> Result<Vec<u8>, UpdateError> {
            Err(UpdateError::Network("not used in check tests".to_owned()))
        }
    }

    #[derive(Default)]
    struct RecordingProgress(Mutex<Vec<DownloadProgress>>);

    impl DownloadProgressReporter for RecordingProgress {
        fn report(&self, progress: DownloadProgress) {
            self.0.lock().unwrap().push(progress);
        }
    }

    // cli 意图的资产按编译期平台 triple 挑选，占位符 @TRIPLE@ 在
    // release_json() 里替换为当前 triple——测试在任意宿主平台都命中。
    const RELEASE_JSON: &str = r#"{
        "tag_name": "v99.0.0",
        "html_url": "https://github.com/ONEGAYI/obsidian-export-desktop/releases/v99.0.0",
        "body": "changelog body",
        "assets": [
            {"name": "obsidian-export-@TRIPLE@.zip", "browser_download_url": "https://x/cli.zip", "size": 1},
            {"name": "obsidian-export-@TRIPLE@.zip.sha256", "browser_download_url": "https://x/cli.zip.sha256", "size": 2},
            {"name": "Obsidian.Export_99.0.0_x64-setup.exe", "browser_download_url": "https://x/setup.exe", "size": 3},
            {"name": "Obsidian.Export_99.0.0_x64_en-US.msi", "browser_download_url": "https://x/app.msi", "size": 4},
            {"name": "obsidian-export-installer.ps1", "browser_download_url": "https://x/installer.ps1", "size": 5}
        ]
    }"#;

    fn release_json() -> String {
        RELEASE_JSON.replace("@TRIPLE@", current_target_triple())
    }

    // ---- 版本比较 ----

    #[test]
    fn parse_version_accepts_common_forms() {
        assert_eq!(parse_version("26.8.4"), Some((26, 8, 4)));
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("V2.0.0"), Some((2, 0, 0)));
        assert_eq!(parse_version("1.2.3-rc.1"), Some((1, 2, 3)));
        assert_eq!(parse_version("1.2.3+build.7"), Some((1, 2, 3)));
        assert_eq!(parse_version(" 1.2.3 "), Some((1, 2, 3)));
        // 非法形态
        assert_eq!(parse_version("1.2"), None, "段数不足");
        assert_eq!(parse_version("1.2.3.4"), None, "段数过多");
        assert_eq!(parse_version("a.b.c"), None, "非数字");
        assert_eq!(parse_version(""), None);
        assert_eq!(parse_version("latest"), None);
    }

    #[test]
    fn is_newer_compares_and_tolerates_bad_tags() {
        assert_eq!(is_newer("v26.9.0", "26.8.9"), Some(true));
        assert_eq!(is_newer("26.8.4", "26.8.4"), Some(false), "相同版本不算新");
        assert_eq!(is_newer("v26.8.9", "26.9.0"), Some(false));
        assert_eq!(
            is_newer("nightly", "26.8.4"),
            None,
            "远端 tag 不规范 → 不提示"
        );
        assert_eq!(is_newer("26.9.0", "dev"), None, "本地版本不规范 → 不提示");
    }

    // ---- 资产挑选 ----

    fn asset_named(name: &str) -> ReleaseAsset {
        ReleaseAsset {
            name: name.to_owned(),
            browser_download_url: format!("https://x/{name}"),
            size: 1,
        }
    }

    #[test]
    fn pick_asset_cli_matches_triple_prefix_and_skips_checksums() {
        let triple = current_target_triple();
        // 当前平台（Windows 测试环境）应命中 zip 而非其 sha256 副产物
        let name = format!("obsidian-export-{triple}.zip");
        let assets = vec![
            asset_named("obsidian-export-installer.ps1"),
            asset_named(&name),
            asset_named(&format!("{name}.sha256")),
        ];
        let picked = pick_asset(&assets, AssetTarget::Cli).expect("应命中本平台产物");
        assert_eq!(picked.name, name);

        // 无本平台产物 → None（引导发布页）
        let foreign = vec![asset_named(
            "obsidian-export-riscv64-unknown-linux-gnu.tar.gz",
        )];
        assert!(triple == "unsupported" || pick_asset(&foreign, AssetTarget::Cli).is_none());
    }

    #[test]
    fn pick_asset_desktop_prefers_setup_exe() {
        let assets = vec![
            asset_named("Obsidian.Export_99.0.0_x64_en-US.msi"),
            asset_named("Obsidian.Export_99.0.0_x64-setup.exe"),
            asset_named("obsidian-export-x86_64-pc-windows-msvc.zip"),
        ];
        let picked = pick_asset(&assets, AssetTarget::Desktop).expect("应命中 NSIS 安装包");
        assert_eq!(picked.name, "Obsidian.Export_99.0.0_x64-setup.exe");

        // 无 setup exe → None（不兜底 msi：MSI 覆盖安装语义与退出时序
        // 与 NSIS 不同，留给端侧引导发布页）
        let no_setup = vec![asset_named("Obsidian.Export_99.0.0_x64_en-US.msi")];
        assert_eq!(pick_asset(&no_setup, AssetTarget::Desktop), None);
    }

    // ---- check_update（mock 分支矩阵 + 请求头契约） ----

    #[test]
    fn check_update_available_with_both_targets() {
        for target in [AssetTarget::Cli, AssetTarget::Desktop] {
            let http = MockClient::ok(&release_json());
            let status = check_update(&http, target).unwrap();
            match status {
                UpdateStatus::Available {
                    version,
                    html_url,
                    notes,
                    asset,
                } => {
                    assert_eq!(version, "99.0.0", "版本号去掉 v 前缀");
                    assert_eq!(
                        html_url,
                        "https://github.com/ONEGAYI/obsidian-export-desktop/releases/v99.0.0"
                    );
                    assert_eq!(notes.as_deref(), Some("changelog body"));
                    let asset = asset.expect("RELEASE_JSON 两意图都有资产");
                    match target {
                        AssetTarget::Cli => assert_eq!(
                            asset.name,
                            format!("obsidian-export-{}.zip", current_target_triple())
                        ),
                        AssetTarget::Desktop => {
                            assert_eq!(asset.name, "Obsidian.Export_99.0.0_x64-setup.exe");
                        }
                    }
                }
                other => panic!("应为 Available：{:?}", other),
            }
        }
    }

    #[test]
    fn check_update_sends_required_headers() {
        let http = MockClient::ok(&release_json());
        check_update(&http, AssetTarget::Cli).unwrap();
        let (url, headers) = {
            let captured = http.captured.lock().unwrap();
            assert_eq!(captured.len(), 1, "应恰好捕获一个请求");
            captured.first().expect("断言已保证非空").clone()
        };
        assert!(
            url.contains(&format!("repos/{GITHUB_REPO}/releases/latest")),
            "URL 应指向本仓库 latest：{}",
            url
        );
        assert!(
            headers
                .iter()
                .any(|(k, v)| k.eq_ignore_ascii_case("User-Agent")
                    && v == &format!("obsidian-export/{VERSION}")),
            "GitHub API 必需 UA：{:?}",
            headers
        );
        assert!(
            headers.iter().any(
                |(k, v)| k.eq_ignore_ascii_case("Accept") && v == "application/vnd.github+json"
            ),
            "vnd Accept 头缺失：{:?}",
            headers
        );
    }

    #[test]
    fn check_update_404_means_no_release() {
        let status =
            check_update(&MockClient::status_with_body(404, ""), AssetTarget::Cli).unwrap();
        assert_eq!(status, UpdateStatus::NoRelease, "当前仓库无 release");
    }

    #[test]
    fn check_update_same_or_bad_tag_is_up_to_date() {
        let same = MockClient::ok(r#"{"tag_name":"v26.8.4","assets":[]}"#);
        assert_eq!(
            check_update(&same, AssetTarget::Cli).unwrap(),
            UpdateStatus::UpToDate
        );
        // tag 不规范：宁可不提示
        let bad = MockClient::ok(r#"{"tag_name":"latest-hotfix","assets":[]}"#);
        assert_eq!(
            check_update(&bad, AssetTarget::Cli).unwrap(),
            UpdateStatus::UpToDate
        );
    }

    #[test]
    fn check_update_available_without_asset_still_reports() {
        // release 只有其他平台产物：Available + asset=None（端侧引导）
        let http = MockClient::ok(
            r#"{"tag_name":"v99.0.0","html_url":"u","assets":[{"name":"other.tar.gz","browser_download_url":"d","size":1}]}"#,
        );
        match check_update(&http, AssetTarget::Desktop).unwrap() {
            UpdateStatus::Available { asset, .. } => assert_eq!(asset, None),
            other => panic!("应为 Available：{:?}", other),
        }
    }

    #[test]
    fn check_update_network_error_is_transient() {
        let err = check_update(&MockClient::failing(), AssetTarget::Cli).unwrap_err();
        assert!(err.is_transient(), "网络错误应归瞬时：{}", err);
    }

    /// 契约：限流 403——主文案 "HTTP error: HTTP 403"，响应体 message
    /// 结构化为 detail；完整文案括号追加；归瞬时。
    #[test]
    fn check_update_status_error_carries_detail() {
        let http = MockClient::status_with_body(
            403,
            r#"{"message":"API rate limit exceeded for 1.2.3.4. (But here's the good news: Authenticated requests get a higher rate limit.)","documentation_url":"https://docs.github.com"}"#,
        );
        let err = check_update(&http, AssetTarget::Cli).unwrap_err();
        let UpdateError::HttpStatus {
            status_text,
            detail,
        } = &err
        else {
            panic!("非 200/404 应为 HttpStatus 变体：{:?}", err);
        };
        assert_eq!(status_text, "HTTP 403");
        assert!(
            detail
                .as_deref()
                .unwrap_or_default()
                .contains("API rate limit exceeded"),
            "detail 应透出限流原因：{:?}",
            detail
        );
        assert_eq!(err.to_string(), "HTTP error: HTTP 403");
        assert!(
            err.full_message()
                .starts_with("HTTP error: HTTP 403 (API rate limit exceeded"),
            "完整文案括号追加详情：{}",
            err.full_message()
        );
        assert!(err.is_transient(), "限流类状态异常归瞬时");
    }

    /// 契约：非 JSON / 空 message 的错误响应体 → detail None。
    #[test]
    fn check_update_status_error_without_message_falls_back() {
        for body in ["", "<html>blocked</html>", r#"{"message":"  "}"#] {
            let err = check_update(&MockClient::status_with_body(403, body), AssetTarget::Cli)
                .unwrap_err();
            let UpdateError::HttpStatus { detail, .. } = &err else {
                panic!("应仍为 HttpStatus 变体：{:?}", err);
            };
            assert_eq!(detail, &None, "body={body:?} 不应解析出 detail");
            assert_eq!(err.full_message(), err.to_string());
        }
    }

    #[test]
    fn check_update_bad_json_is_deterministic_error() {
        let err = check_update(&MockClient::ok("not json"), AssetTarget::Cli).unwrap_err();
        assert!(!err.is_transient(), "解析失败是确定性错误：{}", err);
        assert!(matches!(err, UpdateError::Parse(_)));
    }

    /// 契约：detail 超长截断（200 字符 + 省略号），多字节字符不产生
    /// 半截字符。
    #[test]
    fn extract_error_message_truncates_on_char_boundary() {
        let long = "x".repeat(500);
        let detail = extract_error_message(&format!(r#"{{"message":"{long}"}}"#))
            .expect("长 message 应被提取（截断后）");
        assert!(detail.chars().count() <= 201, "截断到 200 字符加省略号");
        assert!(detail.ends_with('\u{2026}'));
        // 恰好不超长时不加省略号
        let exact = "y".repeat(200);
        let exact_detail = extract_error_message(&format!(r#"{{"message":"{exact}"}}"#)).unwrap();
        assert_eq!(exact_detail, exact);
        assert!(!exact_detail.ends_with('\u{2026}'));
        // 中文不 panic 且无替换字符
        let cjk = "错".repeat(300);
        let cjk_detail = extract_error_message(&format!(r#"{{"message":"{cjk}"}}"#)).unwrap();
        let chars: Vec<char> = cjk_detail.chars().collect();
        assert!(chars.len() <= 201);
        assert!(chars.iter().all(|c| *c != '\u{FFFD}'));
    }

    // ---- ureq 下载（本地 TCP 服务覆盖进度与字节闭环） ----

    #[test]
    fn ureq_download_reports_progress_and_returns_bytes() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nDATA")
                .unwrap();
        });

        let reporter = RecordingProgress::default();
        let bytes = UreqUpdateClient::new()
            .download(&format!("http://{addr}/setup.exe"), &reporter)
            .unwrap();
        server.join().unwrap();

        assert_eq!(bytes, b"DATA", "字节原样");
        let reports = reporter.0.lock().unwrap();
        let first = reports.first().copied();
        let last = reports.last().copied();
        drop(reports);
        assert_eq!(
            first,
            Some(DownloadProgress {
                downloaded_bytes: 0,
                total_bytes: Some(4),
                bytes_per_second: 0,
            }),
            "首帧：0 字节 + 已知总量"
        );
        // 终帧的字段契约：字节数与总量；速率取决于真实耗时（覆盖率
        // 工具的插桩会让 4 字节也耗时数毫秒），不对其断言。
        let last = last.expect("终帧恒发");
        assert_eq!(last.downloaded_bytes, 4);
        assert_eq!(last.total_bytes, Some(4));
    }

    /// 契约：服务器声明 Content-Length 大于实发字节数且干净断流——
    /// reader 提前 EOF 不报错，完整性终检必须把截断流判为失败。
    #[test]
    fn ureq_download_rejects_truncated_stream() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            // 声明 10 字节，实发 4 字节后关闭
            stream
                .write_all(
                    b"HTTP/1.1 200 OK
Content-Length: 10
Connection: close

DATA",
                )
                .unwrap();
        });

        let err = UreqUpdateClient::new()
            .download(
                &format!("http://{addr}/setup.exe"),
                &RecordingProgress::default(),
            )
            .unwrap_err();
        server.join().unwrap();
        // ureq 可能以读错误暴露截断（interrupted），也可能干净 EOF 后由
        // 完整性终检抓住（truncated）——两者都必须是网络类失败。
        assert!(err.is_transient(), "截断属网络类：{}", err);
        let message = err.to_string();
        assert!(
            message.contains("truncated") || message.contains("interrupted"),
            "错误应说明截断：{}",
            message
        );
    }

    #[test]
    fn ureq_download_http_error_is_transient_network() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            stream
                .write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .unwrap();
        });

        let err = UreqUpdateClient::new()
            .download(
                &format!("http://{addr}/missing.exe"),
                &RecordingProgress::default(),
            )
            .unwrap_err();
        server.join().unwrap();
        assert!(err.is_transient());
        assert!(
            err.to_string().contains("404"),
            "状态失败应含状态码：{}",
            err
        );
    }

    #[test]
    fn ureq_get_text_returns_status_and_body() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .unwrap();
        });

        let (status, body) = UreqUpdateClient::new()
            .get_text(&format!("http://{addr}/api"), &[])
            .unwrap();
        server.join().unwrap();
        assert_eq!(status, 200);
        assert_eq!(body, "ok");
    }

    #[test]
    fn speed_uses_elapsed_time_and_handles_zero_duration() {
        assert_eq!(bytes_per_second(1_500, Duration::from_millis(500)), 3_000);
        assert_eq!(bytes_per_second(1_500, Duration::ZERO), 0);
    }

    // ---- 落盘与资产名校验 ----

    #[test]
    fn write_atomic_bytes_roundtrip_and_no_tmp_left() {
        let dir = std::env::temp_dir().join(format!("oe-update-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("setup.zip");
        let payload = vec![0x50_u8, 0x4b, 0x00, 0xff, 0x01];
        write_atomic_bytes(&path, &payload).unwrap();
        assert_eq!(
            std::fs::read(&path).unwrap(),
            payload,
            "二进制原样（含 0x00）"
        );
        let has_tmp_leftover = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .any(|e| e.path().extension().is_some_and(|x| x == "tmp"));
        assert!(!has_tmp_leftover, "不留 tmp 残留");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn validate_asset_name_rejects_path_shapes() {
        assert!(validate_asset_name(
            "obsidian-export-x86_64-pc-windows-msvc.zip"
        ));
        assert!(validate_asset_name("Obsidian.Export_26.9.0_x64-setup.exe"));
        assert!(!validate_asset_name(""));
        assert!(!validate_asset_name("..\\..\\evil.exe"), "反斜杠上跳");
        assert!(!validate_asset_name("a/b.zip"), "POSIX 分隔符");
        assert!(!validate_asset_name("C:x.exe"), "盘符冒号");
        assert!(!validate_asset_name("setup.exe:ads"), "NTFS ADS 冒号");
        // 保留设备名按首段基名判（可带扩展名）；控制字符与尾点/尾空格
        // 会被 Win32 剥除或拒绝
        assert!(!validate_asset_name("CON.x.exe"), "多段保留名");
        assert!(!validate_asset_name("nul.zip"), "小写保留名");
        assert!(!validate_asset_name("com1.exe"), "串口保留名");
        assert!(!validate_asset_name("setup.exe "), "尾空格");
        assert!(!validate_asset_name("setup.exe."), "尾点");
        assert!(!validate_asset_name("set\u{7}up.exe"), "控制字符");
        assert!(
            validate_asset_name("console.exe"),
            "console 非保留名，反向不误伤"
        );
    }

    // ---- debug API base 注入 ----

    /// 纯函数核心的契约：合法 base 生效、尾斜杠剪掉、非法 scheme 忽略
    /// 回退官方 API。env 读取层（debug-only）只有一行透传，不单测。
    #[test]
    fn releases_latest_url_base_semantics() {
        assert_eq!(
            releases_latest_url_with_base(Some("http://127.0.0.1:9999")),
            format!("http://127.0.0.1:9999/repos/{GITHUB_REPO}/releases/latest")
        );
        assert_eq!(
            releases_latest_url_with_base(Some("http://127.0.0.1:9999/")),
            format!("http://127.0.0.1:9999/repos/{GITHUB_REPO}/releases/latest"),
            "尾斜杠应被剪掉"
        );
        // 非 http 前缀的误配置被忽略（防 typo 指到 ftp:// 等）
        assert_eq!(
            releases_latest_url_with_base(Some("ftp://bad")),
            format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest")
        );
        assert_eq!(
            releases_latest_url_with_base(None),
            format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest")
        );
    }

    /// 编译期契约：trait 对象安全（CLI/端侧按 `&dyn` 注入）。
    #[test]
    fn update_client_is_object_safe() {
        let _client: &dyn UpdateClient = &UreqUpdateClient::new();
    }
}
