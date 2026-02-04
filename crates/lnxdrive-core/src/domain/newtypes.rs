//! Domain newtypes with validation
//!
//! This module provides strongly-typed wrappers for domain identifiers and values.
//! Each newtype ensures data validity at construction time.

use std::fmt::{self, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::errors::DomainError;

// ============================================================================
// UUID-based ID types
// ============================================================================

/// A generic unique identifier wrapper around UUID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UniqueId(Uuid);

impl UniqueId {
    /// Create a new random UniqueId
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a UniqueId from an existing UUID
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the inner UUID value
    #[must_use]
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Create a nil (all zeros) UniqueId
    #[must_use]
    pub const fn nil() -> Self {
        Self(Uuid::nil())
    }
}

impl Default for UniqueId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for UniqueId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for UniqueId {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|e| DomainError::InvalidId(format!("Invalid UUID: {e}")))
    }
}

impl From<Uuid> for UniqueId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// Identifier for Account entities
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AccountId(Uuid);

impl AccountId {
    /// Create a new random AccountId
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create an AccountId from an existing UUID
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the inner UUID value
    #[must_use]
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Create a nil (all zeros) AccountId
    #[must_use]
    pub const fn nil() -> Self {
        Self(Uuid::nil())
    }
}

impl Default for AccountId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for AccountId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for AccountId {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|e| DomainError::InvalidId(format!("Invalid AccountId: {e}")))
    }
}

impl From<Uuid> for AccountId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// Identifier for SyncSession entities
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(Uuid);

impl SessionId {
    /// Create a new random SessionId
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a SessionId from an existing UUID
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the inner UUID value
    #[must_use]
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Create a nil (all zeros) SessionId
    #[must_use]
    pub const fn nil() -> Self {
        Self(Uuid::nil())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for SessionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for SessionId {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|e| DomainError::InvalidId(format!("Invalid SessionId: {e}")))
    }
}

impl From<Uuid> for SessionId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// Identifier for Conflict entities
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConflictId(Uuid);

impl ConflictId {
    /// Create a new random ConflictId
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a ConflictId from an existing UUID
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the inner UUID value
    #[must_use]
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Create a nil (all zeros) ConflictId
    #[must_use]
    pub const fn nil() -> Self {
        Self(Uuid::nil())
    }
}

impl Default for ConflictId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for ConflictId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for ConflictId {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|e| DomainError::InvalidId(format!("Invalid ConflictId: {e}")))
    }
}

impl From<Uuid> for ConflictId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// Identifier for audit log entries (database row ID)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AuditId(i64);

impl AuditId {
    /// Create an AuditId from an i64 value
    #[must_use]
    pub const fn new(id: i64) -> Self {
        Self(id)
    }

    /// Get the inner i64 value
    #[must_use]
    pub const fn as_i64(&self) -> i64 {
        self.0
    }
}

impl Display for AuditId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for AuditId {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<i64>()
            .map(Self)
            .map_err(|e| DomainError::InvalidId(format!("Invalid AuditId: {e}")))
    }
}

impl From<i64> for AuditId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}

// ============================================================================
// Path types
// ============================================================================

/// A validated absolute path within the sync root directory
///
/// SyncPath ensures the path is:
/// - Absolute (starts with /)
/// - Normalized (no . or .. components)
/// - Within the sync root when validated against one
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "PathBuf", into = "PathBuf")]
pub struct SyncPath(PathBuf);

impl SyncPath {
    /// Create a new SyncPath, validating it is absolute
    ///
    /// # Errors
    /// Returns `DomainError::InvalidPath` if the path is not absolute
    pub fn new(path: PathBuf) -> Result<Self, DomainError> {
        if !path.is_absolute() {
            return Err(DomainError::InvalidPath(format!(
                "Path must be absolute: {}",
                path.display()
            )));
        }

        // Normalize the path by converting to canonical form conceptually
        // We don't use fs::canonicalize() as the path might not exist yet
        let normalized = Self::normalize_path(&path)?;
        Ok(Self(normalized))
    }

    /// Create a SyncPath validated against a sync root
    ///
    /// # Errors
    /// Returns error if path is not within the sync root
    pub fn new_within_root(path: PathBuf, sync_root: &SyncPath) -> Result<Self, DomainError> {
        let sync_path = Self::new(path)?;

        if !sync_path.0.starts_with(&sync_root.0) {
            return Err(DomainError::PathNotInSyncRoot(format!(
                "{} is not within sync root {}",
                sync_path.0.display(),
                sync_root.0.display()
            )));
        }

        Ok(sync_path)
    }

    /// Get the inner PathBuf reference
    #[must_use]
    pub fn as_path(&self) -> &PathBuf {
        &self.0
    }

    /// Convert to owned PathBuf
    #[must_use]
    pub fn into_path_buf(self) -> PathBuf {
        self.0
    }

    /// Get the path relative to a root
    ///
    /// # Errors
    /// Returns error if this path is not within the root
    pub fn relative_to(&self, root: &SyncPath) -> Result<PathBuf, DomainError> {
        self.0
            .strip_prefix(&root.0)
            .map(|p| p.to_path_buf())
            .map_err(|_| {
                DomainError::PathNotInSyncRoot(format!(
                    "{} is not within {}",
                    self.0.display(),
                    root.0.display()
                ))
            })
    }

    /// Join a relative path to this SyncPath
    ///
    /// # Errors
    /// Returns error if the component contains invalid sequences
    pub fn join(&self, component: &str) -> Result<Self, DomainError> {
        // Prevent path traversal
        if component.contains("..") || component.starts_with('/') {
            return Err(DomainError::InvalidPath(format!(
                "Invalid path component: {component}"
            )));
        }

        let new_path = self.0.join(component);
        Self::new(new_path)
    }

    /// Normalize a path by resolving . and .. components
    fn normalize_path(path: &PathBuf) -> Result<PathBuf, DomainError> {
        use std::path::Component;

        let mut normalized = PathBuf::new();

        for component in path.components() {
            match component {
                Component::Prefix(p) => normalized.push(p.as_os_str()),
                Component::RootDir => normalized.push("/"),
                Component::CurDir => {}
                Component::ParentDir => {
                    if !normalized.pop() {
                        return Err(DomainError::InvalidPath(
                            "Path escapes root via ..".to_string(),
                        ));
                    }
                }
                Component::Normal(c) => normalized.push(c),
            }
        }

        Ok(normalized)
    }
}

impl Display for SyncPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

impl TryFrom<PathBuf> for SyncPath {
    type Error = DomainError;

    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        Self::new(path)
    }
}

impl From<SyncPath> for PathBuf {
    fn from(sync_path: SyncPath) -> Self {
        sync_path.0
    }
}

impl AsRef<std::path::Path> for SyncPath {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}

/// A OneDrive remote path (must start with /)
///
/// Represents paths in OneDrive format, e.g., "/Documents/file.txt"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RemotePath(String);

impl RemotePath {
    /// Create a new RemotePath
    ///
    /// # Errors
    /// Returns error if path doesn't start with /
    pub fn new(path: String) -> Result<Self, DomainError> {
        if !path.starts_with('/') {
            return Err(DomainError::InvalidRemotePath(format!(
                "Remote path must start with '/': {path}"
            )));
        }

        // Validate no double slashes (except root)
        if path.len() > 1 && path.contains("//") {
            return Err(DomainError::InvalidRemotePath(format!(
                "Remote path contains invalid double slashes: {path}"
            )));
        }

        // Validate no path traversal
        if path.contains("..") {
            return Err(DomainError::InvalidRemotePath(format!(
                "Remote path contains invalid traversal: {path}"
            )));
        }

        Ok(Self(path))
    }

    /// Create the root path "/"
    #[must_use]
    pub fn root() -> Self {
        Self("/".to_string())
    }

    /// Get the inner string reference
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Join a path component
    ///
    /// # Errors
    /// Returns error if component is invalid
    pub fn join(&self, component: &str) -> Result<Self, DomainError> {
        if component.is_empty() || component.contains('/') || component.contains("..") {
            return Err(DomainError::InvalidRemotePath(format!(
                "Invalid path component: {component}"
            )));
        }

        let new_path = if self.0 == "/" {
            format!("/{component}")
        } else {
            format!("{}/{component}", self.0)
        };

        Self::new(new_path)
    }

    /// Get the parent path
    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        if self.0 == "/" {
            return None;
        }

        match self.0.rfind('/') {
            Some(0) => Some(Self::root()),
            Some(idx) => Some(Self(self.0[..idx].to_string())),
            None => None,
        }
    }

    /// Get the file name component
    #[must_use]
    pub fn file_name(&self) -> Option<&str> {
        if self.0 == "/" {
            return None;
        }

        self.0.rsplit('/').next()
    }
}

impl Display for RemotePath {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for RemotePath {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s.to_string())
    }
}

impl TryFrom<String> for RemotePath {
    type Error = DomainError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl From<RemotePath> for String {
    fn from(path: RemotePath) -> Self {
        path.0
    }
}

// ============================================================================
// OneDrive-specific types
// ============================================================================

/// OneDrive item ID (alphanumeric identifier)
///
/// Format: Alphanumeric string, typically like "01BYE5RZ6QN3ZWBTUFOFD3GSPGOHDJD36K"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RemoteId(String);

impl RemoteId {
    /// Create a new RemoteId
    ///
    /// # Errors
    /// Returns error if the ID format is invalid
    pub fn new(id: String) -> Result<Self, DomainError> {
        if id.is_empty() {
            return Err(DomainError::InvalidRemoteId(
                "Remote ID cannot be empty".to_string(),
            ));
        }

        // OneDrive IDs are alphanumeric with possible special chars
        // They are typically base64-like or hex strings
        if !id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '!' || c == '-' || c == '_')
        {
            return Err(DomainError::InvalidRemoteId(format!(
                "Remote ID contains invalid characters: {id}"
            )));
        }

        Ok(Self(id))
    }

    /// Get the inner string reference
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for RemoteId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for RemoteId {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s.to_string())
    }
}

impl TryFrom<String> for RemoteId {
    type Error = DomainError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl From<RemoteId> for String {
    fn from(id: RemoteId) -> Self {
        id.0
    }
}

/// OneDrive quickXorHash in Base64 format
///
/// This is the hash algorithm used by OneDrive for file integrity.
/// Format: Base64-encoded 20-byte hash
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct FileHash(String);

impl FileHash {
    /// Expected decoded length of quickXorHash (20 bytes)
    const EXPECTED_DECODED_LEN: usize = 20;

    /// Create a new FileHash
    ///
    /// # Errors
    /// Returns error if the hash is not valid Base64 or wrong length
    pub fn new(hash: String) -> Result<Self, DomainError> {
        if hash.is_empty() {
            return Err(DomainError::InvalidHash("Hash cannot be empty".to_string()));
        }

        // Validate Base64 format
        // Base64 uses A-Z, a-z, 0-9, +, /, and = for padding
        if !hash
            .chars()
            .all(|c| c.is_alphanumeric() || c == '+' || c == '/' || c == '=')
        {
            return Err(DomainError::InvalidHash(format!(
                "Hash is not valid Base64: {hash}"
            )));
        }

        // Validate length (20 bytes in Base64 = ceil(20 * 4/3) = 28 chars with padding)
        // quickXorHash produces 20 bytes, which encodes to 28 Base64 characters
        let decoded_len = Self::base64_decoded_len(&hash);
        if decoded_len != Self::EXPECTED_DECODED_LEN {
            return Err(DomainError::InvalidHash(format!(
                "Hash has wrong length: expected {} bytes, got {} bytes",
                Self::EXPECTED_DECODED_LEN,
                decoded_len
            )));
        }

        Ok(Self(hash))
    }

    /// Get the inner string reference
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Calculate the decoded length of a Base64 string
    fn base64_decoded_len(s: &str) -> usize {
        let len = s.len();
        let padding = s.chars().rev().take_while(|&c| c == '=').count();
        (len * 3 / 4) - padding
    }
}

impl Display for FileHash {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for FileHash {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s.to_string())
    }
}

impl TryFrom<String> for FileHash {
    type Error = DomainError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl From<FileHash> for String {
    fn from(hash: FileHash) -> Self {
        hash.0
    }
}

/// Microsoft Graph delta token (opaque string)
///
/// Used for incremental synchronization with OneDrive.
/// The token is opaque - we don't validate its contents, only that it's non-empty.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DeltaToken(String);

impl DeltaToken {
    /// Create a new DeltaToken
    ///
    /// # Errors
    /// Returns error if the token is empty
    pub fn new(token: String) -> Result<Self, DomainError> {
        if token.is_empty() {
            return Err(DomainError::InvalidDeltaToken(
                "Delta token cannot be empty".to_string(),
            ));
        }

        // Delta tokens can contain various characters including URL-encoded parts
        // We do minimal validation since they're opaque
        Ok(Self(token))
    }

    /// Get the inner string reference
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for DeltaToken {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for DeltaToken {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s.to_string())
    }
}

impl TryFrom<String> for DeltaToken {
    type Error = DomainError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl From<DeltaToken> for String {
    fn from(token: DeltaToken) -> Self {
        token.0
    }
}

// ============================================================================
// Email type
// ============================================================================

/// Validated email address (RFC 5322 basic validation)
///
/// Performs basic structural validation:
/// - Contains exactly one @
/// - Has non-empty local part
/// - Has non-empty domain with at least one dot
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Email(String);

impl Email {
    /// Create a new validated Email
    ///
    /// # Errors
    /// Returns error if the email format is invalid
    pub fn new(email: String) -> Result<Self, DomainError> {
        Self::validate(&email)?;
        // Store in lowercase for consistency
        Ok(Self(email.to_lowercase()))
    }

    /// Get the inner string reference
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Get the local part (before @)
    #[must_use]
    pub fn local_part(&self) -> &str {
        self.0.split('@').next().unwrap_or("")
    }

    /// Get the domain part (after @)
    #[must_use]
    pub fn domain(&self) -> &str {
        self.0.split('@').nth(1).unwrap_or("")
    }

    /// Validate email format
    fn validate(email: &str) -> Result<(), DomainError> {
        if email.is_empty() {
            return Err(DomainError::InvalidEmail(
                "Email cannot be empty".to_string(),
            ));
        }

        // Split by @
        let parts: Vec<&str> = email.split('@').collect();
        if parts.len() != 2 {
            return Err(DomainError::InvalidEmail(format!(
                "Email must contain exactly one '@': {email}"
            )));
        }

        let local = parts[0];
        let domain = parts[1];

        // Validate local part
        if local.is_empty() {
            return Err(DomainError::InvalidEmail(format!(
                "Email local part cannot be empty: {email}"
            )));
        }

        if local.len() > 64 {
            return Err(DomainError::InvalidEmail(format!(
                "Email local part too long (max 64 chars): {email}"
            )));
        }

        // Local part can contain alphanumeric, dots, hyphens, underscores, plus
        if !local
            .chars()
            .all(|c| c.is_alphanumeric() || ".+-_".contains(c))
        {
            return Err(DomainError::InvalidEmail(format!(
                "Email local part contains invalid characters: {email}"
            )));
        }

        // Validate domain
        if domain.is_empty() {
            return Err(DomainError::InvalidEmail(format!(
                "Email domain cannot be empty: {email}"
            )));
        }

        if domain.len() > 255 {
            return Err(DomainError::InvalidEmail(format!(
                "Email domain too long (max 255 chars): {email}"
            )));
        }

        // Domain must contain at least one dot
        if !domain.contains('.') {
            return Err(DomainError::InvalidEmail(format!(
                "Email domain must contain at least one dot: {email}"
            )));
        }

        // Domain can contain alphanumeric, dots, hyphens
        if !domain
            .chars()
            .all(|c| c.is_alphanumeric() || ".-".contains(c))
        {
            return Err(DomainError::InvalidEmail(format!(
                "Email domain contains invalid characters: {email}"
            )));
        }

        // Domain labels cannot start or end with hyphen
        for label in domain.split('.') {
            if label.is_empty() {
                return Err(DomainError::InvalidEmail(format!(
                    "Email domain contains empty label: {email}"
                )));
            }
            if label.starts_with('-') || label.ends_with('-') {
                return Err(DomainError::InvalidEmail(format!(
                    "Email domain label cannot start or end with hyphen: {email}"
                )));
            }
        }

        Ok(())
    }
}

impl Display for Email {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Email {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s.to_string())
    }
}

impl TryFrom<String> for Email {
    type Error = DomainError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl From<Email> for String {
    fn from(email: Email) -> Self {
        email.0
    }
}

// Custom Hash implementation for Email to ensure case-insensitive hashing
impl Hash for Email {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    mod unique_id_tests {
        use super::*;

        #[test]
        fn test_new_creates_unique_ids() {
            let id1 = UniqueId::new();
            let id2 = UniqueId::new();
            assert_ne!(id1, id2);
        }

        #[test]
        fn test_from_uuid() {
            let uuid = Uuid::new_v4();
            let id = UniqueId::from_uuid(uuid);
            assert_eq!(id.as_uuid(), &uuid);
        }

        #[test]
        fn test_from_str() {
            let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
            let id: UniqueId = uuid_str.parse().unwrap();
            assert_eq!(id.to_string(), uuid_str);
        }

        #[test]
        fn test_from_str_invalid() {
            let result: Result<UniqueId, _> = "not-a-uuid".parse();
            assert!(result.is_err());
        }

        #[test]
        fn test_nil() {
            let id = UniqueId::nil();
            assert_eq!(id.to_string(), "00000000-0000-0000-0000-000000000000");
        }

        #[test]
        fn test_serde_roundtrip() {
            let id = UniqueId::new();
            let json = serde_json::to_string(&id).unwrap();
            let parsed: UniqueId = serde_json::from_str(&json).unwrap();
            assert_eq!(id, parsed);
        }
    }

    mod account_id_tests {
        use super::*;

        #[test]
        fn test_new_creates_unique_ids() {
            let id1 = AccountId::new();
            let id2 = AccountId::new();
            assert_ne!(id1, id2);
        }

        #[test]
        fn test_display() {
            let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
            let id = AccountId::from_uuid(uuid);
            assert_eq!(id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
        }
    }

    mod sync_path_tests {
        use super::*;

        #[test]
        fn test_new_absolute_path() {
            let path = SyncPath::new(PathBuf::from("/home/user/sync")).unwrap();
            assert_eq!(path.to_string(), "/home/user/sync");
        }

        #[test]
        fn test_new_relative_path_fails() {
            let result = SyncPath::new(PathBuf::from("relative/path"));
            assert!(result.is_err());
        }

        #[test]
        fn test_join() {
            let root = SyncPath::new(PathBuf::from("/home/user/sync")).unwrap();
            let joined = root.join("subdir").unwrap();
            assert_eq!(joined.to_string(), "/home/user/sync/subdir");
        }

        #[test]
        fn test_join_traversal_fails() {
            let root = SyncPath::new(PathBuf::from("/home/user/sync")).unwrap();
            let result = root.join("../outside");
            assert!(result.is_err());
        }

        #[test]
        fn test_relative_to() {
            let root = SyncPath::new(PathBuf::from("/home/user/sync")).unwrap();
            let child = SyncPath::new(PathBuf::from("/home/user/sync/docs/file.txt")).unwrap();
            let relative = child.relative_to(&root).unwrap();
            assert_eq!(relative, PathBuf::from("docs/file.txt"));
        }

        #[test]
        fn test_within_root() {
            let root = SyncPath::new(PathBuf::from("/home/user/sync")).unwrap();
            let child =
                SyncPath::new_within_root(PathBuf::from("/home/user/sync/docs"), &root).unwrap();
            assert!(child.as_path().starts_with(root.as_path()));
        }

        #[test]
        fn test_not_within_root_fails() {
            let root = SyncPath::new(PathBuf::from("/home/user/sync")).unwrap();
            let result = SyncPath::new_within_root(PathBuf::from("/home/other/docs"), &root);
            assert!(result.is_err());
        }
    }

    mod remote_path_tests {
        use super::*;

        #[test]
        fn test_new_valid() {
            let path = RemotePath::new("/Documents/file.txt".to_string()).unwrap();
            assert_eq!(path.as_str(), "/Documents/file.txt");
        }

        #[test]
        fn test_root() {
            let root = RemotePath::root();
            assert_eq!(root.as_str(), "/");
        }

        #[test]
        fn test_no_leading_slash_fails() {
            let result = RemotePath::new("Documents/file.txt".to_string());
            assert!(result.is_err());
        }

        #[test]
        fn test_double_slash_fails() {
            let result = RemotePath::new("/Documents//file.txt".to_string());
            assert!(result.is_err());
        }

        #[test]
        fn test_traversal_fails() {
            let result = RemotePath::new("/Documents/../file.txt".to_string());
            assert!(result.is_err());
        }

        #[test]
        fn test_join() {
            let path = RemotePath::root();
            let joined = path.join("Documents").unwrap();
            assert_eq!(joined.as_str(), "/Documents");

            let joined2 = joined.join("file.txt").unwrap();
            assert_eq!(joined2.as_str(), "/Documents/file.txt");
        }

        #[test]
        fn test_parent() {
            let path = RemotePath::new("/Documents/Sub/file.txt".to_string()).unwrap();
            let parent = path.parent().unwrap();
            assert_eq!(parent.as_str(), "/Documents/Sub");

            let grandparent = parent.parent().unwrap();
            assert_eq!(grandparent.as_str(), "/Documents");

            let root_parent = grandparent.parent().unwrap();
            assert_eq!(root_parent.as_str(), "/");

            assert!(root_parent.parent().is_none());
        }

        #[test]
        fn test_file_name() {
            let path = RemotePath::new("/Documents/file.txt".to_string()).unwrap();
            assert_eq!(path.file_name(), Some("file.txt"));

            let root = RemotePath::root();
            assert_eq!(root.file_name(), None);
        }
    }

    mod remote_id_tests {
        use super::*;

        #[test]
        fn test_valid_id() {
            let id = RemoteId::new("01BYE5RZ6QN3ZWBTUFOFD3GSPGOHDJD36K".to_string()).unwrap();
            assert_eq!(id.as_str(), "01BYE5RZ6QN3ZWBTUFOFD3GSPGOHDJD36K");
        }

        #[test]
        fn test_empty_fails() {
            let result = RemoteId::new(String::new());
            assert!(result.is_err());
        }

        #[test]
        fn test_invalid_chars_fails() {
            let result = RemoteId::new("invalid@id".to_string());
            assert!(result.is_err());
        }

        #[test]
        fn test_serde_roundtrip() {
            let id = RemoteId::new("ABC123".to_string()).unwrap();
            let json = serde_json::to_string(&id).unwrap();
            let parsed: RemoteId = serde_json::from_str(&json).unwrap();
            assert_eq!(id, parsed);
        }
    }

    mod file_hash_tests {
        use super::*;

        #[test]
        fn test_valid_hash() {
            // 20 bytes in Base64 = 28 chars (with padding)
            let hash = FileHash::new("AAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string()).unwrap();
            assert!(!hash.as_str().is_empty());
        }

        #[test]
        fn test_empty_fails() {
            let result = FileHash::new(String::new());
            assert!(result.is_err());
        }

        #[test]
        fn test_invalid_base64_fails() {
            let result = FileHash::new("not@valid#base64".to_string());
            assert!(result.is_err());
        }

        #[test]
        fn test_wrong_length_fails() {
            let result = FileHash::new("AAAA".to_string());
            assert!(result.is_err());
        }
    }

    mod delta_token_tests {
        use super::*;

        #[test]
        fn test_valid_token() {
            let token =
                DeltaToken::new("aHR0cHM6Ly9ncmFwaC5taWNyb3NvZnQuY29t".to_string()).unwrap();
            assert!(!token.as_str().is_empty());
        }

        #[test]
        fn test_empty_fails() {
            let result = DeltaToken::new(String::new());
            assert!(result.is_err());
        }

        #[test]
        fn test_serde_roundtrip() {
            let token = DeltaToken::new("test-token-123".to_string()).unwrap();
            let json = serde_json::to_string(&token).unwrap();
            let parsed: DeltaToken = serde_json::from_str(&json).unwrap();
            assert_eq!(token, parsed);
        }
    }

    mod email_tests {
        use super::*;

        #[test]
        fn test_valid_email() {
            let email = Email::new("user@example.com".to_string()).unwrap();
            assert_eq!(email.as_str(), "user@example.com");
        }

        #[test]
        fn test_case_normalization() {
            let email = Email::new("User@EXAMPLE.COM".to_string()).unwrap();
            assert_eq!(email.as_str(), "user@example.com");
        }

        #[test]
        fn test_local_and_domain_parts() {
            let email = Email::new("user@example.com".to_string()).unwrap();
            assert_eq!(email.local_part(), "user");
            assert_eq!(email.domain(), "example.com");
        }

        #[test]
        fn test_complex_valid_email() {
            let email = Email::new("user.name+tag@sub.example.com".to_string()).unwrap();
            assert_eq!(email.local_part(), "user.name+tag");
            assert_eq!(email.domain(), "sub.example.com");
        }

        #[test]
        fn test_empty_fails() {
            let result = Email::new(String::new());
            assert!(result.is_err());
        }

        #[test]
        fn test_no_at_fails() {
            let result = Email::new("userexample.com".to_string());
            assert!(result.is_err());
        }

        #[test]
        fn test_multiple_at_fails() {
            let result = Email::new("user@name@example.com".to_string());
            assert!(result.is_err());
        }

        #[test]
        fn test_no_domain_dot_fails() {
            let result = Email::new("user@localhost".to_string());
            assert!(result.is_err());
        }

        #[test]
        fn test_empty_local_fails() {
            let result = Email::new("@example.com".to_string());
            assert!(result.is_err());
        }

        #[test]
        fn test_empty_domain_fails() {
            let result = Email::new("user@".to_string());
            assert!(result.is_err());
        }

        #[test]
        fn test_domain_hyphen_edge_fails() {
            let result = Email::new("user@-example.com".to_string());
            assert!(result.is_err());

            let result = Email::new("user@example-.com".to_string());
            assert!(result.is_err());
        }

        #[test]
        fn test_serde_roundtrip() {
            let email = Email::new("test@example.com".to_string()).unwrap();
            let json = serde_json::to_string(&email).unwrap();
            let parsed: Email = serde_json::from_str(&json).unwrap();
            assert_eq!(email, parsed);
        }
    }

    mod audit_id_tests {
        use super::*;

        #[test]
        fn test_new() {
            let id = AuditId::new(42);
            assert_eq!(id.as_i64(), 42);
        }

        #[test]
        fn test_display() {
            let id = AuditId::new(123);
            assert_eq!(id.to_string(), "123");
        }

        #[test]
        fn test_from_str() {
            let id: AuditId = "456".parse().unwrap();
            assert_eq!(id.as_i64(), 456);
        }

        #[test]
        fn test_from_str_invalid() {
            let result: Result<AuditId, _> = "not-a-number".parse();
            assert!(result.is_err());
        }

        #[test]
        fn test_from_i64() {
            let id: AuditId = 789i64.into();
            assert_eq!(id.as_i64(), 789);
        }
    }
}
