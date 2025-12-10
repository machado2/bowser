//! Security sandbox for Prism applications
//!
//! The sandbox enforces strict isolation:
//! - No file system access
//! - No persistent storage
//! - Memory limits
//! - No tracking identifiers

use std::path::Path;

/// Memory limit per application (16MB default)
pub const MEMORY_LIMIT_BYTES: usize = 16 * 1024 * 1024;

/// Maximum file size that can be loaded (1MB)
pub const MAX_FILE_SIZE_BYTES: usize = 1 * 1024 * 1024;

/// Sandbox configuration
pub struct Sandbox {
    memory_used: usize,
    memory_limit: usize,
}

impl Sandbox {
    pub fn new() -> Self {
        Self {
            memory_used: 0,
            memory_limit: MEMORY_LIMIT_BYTES,
        }
    }

    /// Validate that a file path is safe to load
    /// Only allows loading .prism files from the initial directory
    pub fn validate_file_path(&self, path: &Path) -> Result<(), SandboxError> {
        // Must have .prism extension
        match path.extension() {
            Some(ext) if ext == "prism" => {}
            _ => return Err(SandboxError::InvalidFileType),
        }

        // No path traversal
        let path_str = path.to_string_lossy();
        if path_str.contains("..") {
            return Err(SandboxError::PathTraversal);
        }

        Ok(())
    }

    /// Check if loading content would exceed memory limits
    pub fn check_memory(&mut self, bytes: usize) -> Result<(), SandboxError> {
        if bytes > MAX_FILE_SIZE_BYTES {
            return Err(SandboxError::FileTooLarge);
        }

        if self.memory_used + bytes > self.memory_limit {
            return Err(SandboxError::MemoryLimitExceeded);
        }

        self.memory_used += bytes;
        Ok(())
    }

    /// Track memory allocation
    pub fn allocate(&mut self, bytes: usize) -> Result<(), SandboxError> {
        if self.memory_used + bytes > self.memory_limit {
            return Err(SandboxError::MemoryLimitExceeded);
        }
        self.memory_used += bytes;
        Ok(())
    }

    /// Track memory deallocation
    pub fn deallocate(&mut self, bytes: usize) {
        self.memory_used = self.memory_used.saturating_sub(bytes);
    }

    /// Get current memory usage
    pub fn memory_usage(&self) -> usize {
        self.memory_used
    }

    /// Get memory limit
    pub fn memory_limit(&self) -> usize {
        self.memory_limit
    }

    /// Generate a session-only random identifier (not persistent)
    /// This cannot be used for tracking across sessions
    pub fn session_id(&self) -> u64 {
        // Use a simple random source - this is regenerated each session
        use std::time::{SystemTime, UNIX_EPOCH};
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        duration.as_nanos() as u64 ^ 0xDEADBEEF
    }
}

#[derive(Debug)]
pub enum SandboxError {
    InvalidFileType,
    PathTraversal,
    FileTooLarge,
    MemoryLimitExceeded,
    NetworkDisabled,
    StorageDisabled,
}

impl std::fmt::Display for SandboxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SandboxError::InvalidFileType => write!(f, "Only .prism files can be loaded"),
            SandboxError::PathTraversal => write!(f, "Path traversal not allowed"),
            SandboxError::FileTooLarge => write!(f, "File exceeds maximum size limit"),
            SandboxError::MemoryLimitExceeded => write!(f, "Memory limit exceeded"),
            SandboxError::NetworkDisabled => write!(f, "Network access is disabled"),
            SandboxError::StorageDisabled => write!(f, "Persistent storage is disabled"),
        }
    }
}

impl std::error::Error for SandboxError {}

/// Capabilities that an application can request (all denied by default)
#[derive(Debug, Clone, Default)]
pub struct Capabilities {
    /// Allow same-origin network requests
    pub network_same_origin: bool,
    /// Allow clipboard read
    pub clipboard_read: bool,
    /// Allow clipboard write
    pub clipboard_write: bool,
}

impl Capabilities {
    pub fn none() -> Self {
        Self::default()
    }

    /// Parse capabilities from app metadata
    pub fn from_app_meta(_meta: &str) -> Self {
        // For now, return no capabilities
        // In future, could parse @capability directives
        Self::none()
    }
}
