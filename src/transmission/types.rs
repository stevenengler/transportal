use serde::{Deserialize, Serialize};

use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug, Serialize)]
pub struct Request {
    #[serde(flatten)]
    pub request: RequestInner,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<u32>,
}

impl Request {
    pub fn session_get(keys: Vec<SessionGetKey>) -> Self {
        let request = RequestInner::SessionGet { fields: keys };
        Self { request, tag: None }
    }

    pub fn torrent_get(
        format: TorrentGetFormat,
        keys: Vec<TorrentGetKey>,
        ids: Option<Vec<String>>,
    ) -> Self {
        let request = RequestInner::TorrentGet {
            format,
            ids,
            fields: keys,
        };
        Self { request, tag: None }
    }

    pub fn torrent_start(ids: Option<Vec<String>>) -> Self {
        let request = RequestInner::TorrentStart { ids };
        Self { request, tag: None }
    }

    pub fn torrent_stop(ids: Option<Vec<String>>) -> Self {
        let request = RequestInner::TorrentStop { ids };
        Self { request, tag: None }
    }

    pub fn torrent_verify(ids: Option<Vec<String>>) -> Self {
        let request = RequestInner::TorrentVerify { ids };
        Self { request, tag: None }
    }

    pub fn torrent_add(required: TorrentAddRequired, paused: bool) -> Self {
        let request = RequestInner::TorrentAdd {
            required,
            cookies: None,
            download_dir: None,
            labels: None,
            paused: Some(paused),
            peer_limit: None,
            bandwidth_priority: None,
            files_wanted: None,
            files_unwanted: None,
            priority_high: None,
            priority_low: None,
            priority_normal: None,
        };
        Self { request, tag: None }
    }

    #[allow(dead_code)]
    pub fn tag(&mut self, tag: Option<u32>) {
        self.tag = tag;
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
#[serde(tag = "method", content = "arguments")]
pub enum RequestInner {
    SessionGet {
        fields: Vec<SessionGetKey>,
    },
    TorrentGet {
        format: TorrentGetFormat,
        fields: Vec<TorrentGetKey>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ids: Option<Vec<String>>,
    },
    TorrentStart {
        #[serde(skip_serializing_if = "Option::is_none")]
        ids: Option<Vec<String>>,
    },
    TorrentStop {
        #[serde(skip_serializing_if = "Option::is_none")]
        ids: Option<Vec<String>>,
    },
    TorrentVerify {
        #[serde(skip_serializing_if = "Option::is_none")]
        ids: Option<Vec<String>>,
    },
    TorrentAdd {
        #[serde(flatten)]
        required: TorrentAddRequired,
        #[serde(skip_serializing_if = "Option::is_none")]
        cookies: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        download_dir: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        labels: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        paused: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        peer_limit: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "bandwidthPriority")]
        bandwidth_priority: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        files_wanted: Option<Vec<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        files_unwanted: Option<Vec<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        priority_high: Option<Vec<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        priority_low: Option<Vec<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        priority_normal: Option<Vec<u32>>,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TorrentAddRequired {
    Filename(String),
    #[allow(dead_code)]
    Metainfo(String),
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionGetKey {
    AltSpeedDown,
    AltSpeedEnabled,
    AltSpeedTimeBegin,
    AltSpeedTimeDay,
    AltSpeedTimeEnabled,
    AltSpeedTimeEnd,
    AltSpeedUp,
    BlocklistEnabled,
    BlocklistSize,
    BlocklistUrl,
    CacheSizeMb,
    ConfigDir,
    DefaultTrackers,
    DhtEnabled,
    DownloadDir,
    DownloadDirFreeSpace,
    DownloadQueueEnabled,
    DownloadQueueSize,
    Encryption,
    IdleSeedingLimitEnabled,
    IdleSeedingLimit,
    IncompleteDirEnabled,
    IncompleteDir,
    LpdEnabled,
    PeerLimitGlobal,
    PeerLimitPerTorrent,
    PeerPortRandomOnStart,
    PeerPort,
    PexEnabled,
    PortForwardingEnabled,
    QueueStalledEnabled,
    QueueStalledMinutes,
    RenamePartialFiles,
    RpcVersionMinimum,
    RpcVersionSemver,
    RpcVersion,
    ScriptTorrentAddedEnabled,
    ScriptTorrentAddedFilename,
    ScriptTorrentDoneEnabled,
    ScriptTorrentDoneFilename,
    ScriptTorrentDoneSeedingEnabled,
    ScriptTorrentDoneSeedingFilename,
    SeedQueueEnabled,
    SeedQueueSize,
    #[serde(rename = "seedRatioLimit")]
    SeedRatioLimit,
    #[serde(rename = "seedRatioLimited")]
    SeedRatioLimited,
    SessionId,
    SpeedLimitDownEnabled,
    SpeedLimitDown,
    SpeedLimitUpEnabled,
    SpeedLimitUp,
    StartAddedTorrents,
    TrashOriginalTorrentFiles,
    Units,
    UtpEnabled,
    Version,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TorrentGetFormat {
    Objects,
    #[allow(dead_code)]
    Table,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TorrentGetKey {
    ActivityDate,
    AddedDate,
    Availability,
    BandwidthPriority,
    Comment,
    CorruptEver,
    Creator,
    DateCreated,
    DesiredAvailable,
    DoneDate,
    DownloadDir,
    DownloadedEver,
    DownloadLimit,
    DownloadLimited,
    EditDate,
    Error,
    ErrorString,
    Eta,
    EtaIdle,
    #[serde(rename = "file-count")]
    FileCount,
    Files,
    FileStats,
    Group,
    HashString,
    HaveUnchecked,
    HaveValid,
    HonorsSessionLimits,
    /// Don't use; the ID can change at any time and is not persistent.
    Id,
    IsFinished,
    IsPrivate,
    IsStalled,
    Labels,
    LeftUntilDone,
    MagnetLink,
    ManualAnnounceTime,
    MaxConnectedPeers,
    MetadataPercentComplete,
    Name,
    #[serde(rename = "peer-limit")]
    PeerLimit,
    Peers,
    PeersConnected,
    PeersFrom,
    PeersGettingFromUs,
    PeersSendingToUs,
    PercentComplete,
    PercentDone,
    Pieces,
    PieceCount,
    PieceSize,
    Priorities,
    #[serde(rename = "primary-mime-type")]
    PrimaryMimeType,
    QueuePosition,
    RateDownload,
    RateUpload,
    RecheckProgress,
    SecondsDownloading,
    SecondsSeeding,
    SeedIdleLimit,
    SeedIdleMode,
    SeedRatioLimit,
    SeedRatioMode,
    SequentialDownload,
    SizeWhenDone,
    StartDate,
    Status,
    Trackers,
    TrackerList,
    TrackerStats,
    TotalSize,
    TorrentFile,
    UploadedEver,
    UploadLimit,
    UploadLimited,
    UploadRatio,
    Wanted,
    Webseeds,
    WebseedsSendingToUs,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum TorrentStatus {
    /// Torrent is stopped.
    Stopped = 0,
    /// Torrent is queued to verify local data.
    VerifyQueued = 1,
    /// Torrent is verifying local data.
    Verifying = 2,
    /// Torrent is queued to download.
    DownloadQueued = 3,
    /// Torrent is downloading.
    Downloading = 4,
    /// Torrent is queued to seed.
    SeedQueued = 5,
    /// Torrent is seeding.
    Seeding = 6,
}

impl TorrentStatus {
    pub fn ui(&self) -> &'static str {
        match self {
            Self::Stopped => "Paused",
            Self::VerifyQueued => "Queued for verification",
            Self::Verifying => "Verifying",
            Self::DownloadQueued => "Queued for download",
            Self::Downloading => "Downloading",
            Self::SeedQueued => "Queued for seeding",
            Self::Seeding => "Seeding",
        }
    }
}

impl TryFrom<u64> for TorrentStatus {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            x if x == Self::Stopped as u64 => Ok(Self::Stopped),
            x if x == Self::VerifyQueued as u64 => Ok(Self::VerifyQueued),
            x if x == Self::Verifying as u64 => Ok(Self::Verifying),
            x if x == Self::DownloadQueued as u64 => Ok(Self::DownloadQueued),
            x if x == Self::Downloading as u64 => Ok(Self::Downloading),
            x if x == Self::SeedQueued as u64 => Ok(Self::SeedQueued),
            x if x == Self::Seeding as u64 => Ok(Self::Seeding),
            _ => Err(()),
        }
    }
}

impl TryFrom<&u64> for TorrentStatus {
    type Error = <TorrentStatus as TryFrom<u64>>::Error;
    fn try_from(x: &u64) -> Result<Self, Self::Error> {
        Self::try_from(*x)
    }
}

impl std::fmt::Display for TorrentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stopped => write!(f, "stopped"),
            Self::VerifyQueued => write!(f, "verify-queued"),
            Self::Verifying => write!(f, "verifying"),
            Self::DownloadQueued => write!(f, "download-queued"),
            Self::Downloading => write!(f, "downloading"),
            Self::SeedQueued => write!(f, "seed-queued"),
            Self::Seeding => write!(f, "seeding"),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Response<T> {
    pub result: String,
    pub arguments: T,
    pub tag: Option<u32>,
}

impl<T> Response<T> {
    pub fn is_success(&self) -> bool {
        self.result == "success"
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct SessionGetResponse(pub HashMap<SessionGetKey, serde_json::Value>);

#[derive(Clone, Debug, Deserialize)]
pub struct TorrentGetResponse {
    pub torrents: Vec<BTreeMap<TorrentGetKey, serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
pub struct TorrentAddResponse {
    #[serde(flatten)]
    pub added_or_duplicate: TorrentAddedOrDuplicate,
}

impl TorrentAddResponse {
    pub fn hash_string(&self) -> &str {
        match &self.added_or_duplicate {
            TorrentAddedOrDuplicate::TorrentAdded(x) => &x.hash_string,
            TorrentAddedOrDuplicate::TorrentDuplicate(x) => &x.hash_string,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TorrentAddedOrDuplicate {
    TorrentAdded(TorrentAdded),
    TorrentDuplicate(TorrentDuplicate),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentAdded {
    pub hash_string: String,
    pub name: String,
    pub id: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentDuplicate {
    pub hash_string: String,
    pub name: String,
    pub id: u32,
}
