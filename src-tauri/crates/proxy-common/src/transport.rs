//! Cross-platform IPC transport for Brightwing daemon communication.
//!
//! Uses Unix domain sockets on macOS/Linux and named pipes on Windows.
//! Provides `IpcListener` (server side) and `DaemonClient` (client side).

use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};

use crate::ipc::{
    ClientType, HandshakeRequest, IpcRequest, IpcResponse, decode_message, encode_message,
};
use crate::IPC_PROTOCOL_VERSION;

// ─── Platform stream types ───────────────────────────────────────────────────

#[cfg(unix)]
type ReadHalf = tokio::io::ReadHalf<tokio::net::UnixStream>;
#[cfg(unix)]
type WriteHalf = tokio::io::WriteHalf<tokio::net::UnixStream>;

#[cfg(windows)]
type ReadHalf = tokio::io::ReadHalf<tokio::net::windows::named_pipe::NamedPipeClient>;
#[cfg(windows)]
type WriteHalf = tokio::io::WriteHalf<tokio::net::windows::named_pipe::NamedPipeClient>;

// Server-side stream types (authd uses these)
#[cfg(unix)]
type ServerReadHalf = tokio::io::ReadHalf<tokio::net::UnixStream>;
#[cfg(unix)]
type ServerWriteHalf = tokio::io::WriteHalf<tokio::net::UnixStream>;

#[cfg(windows)]
type ServerReadHalf = tokio::io::ReadHalf<tokio::net::windows::named_pipe::NamedPipeServer>;
#[cfg(windows)]
type ServerWriteHalf = tokio::io::WriteHalf<tokio::net::windows::named_pipe::NamedPipeServer>;

// ─── Default socket path ─────────────────────────────────────────────────────

/// Get the default IPC path for the auth daemon.
pub fn default_socket_path() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("Library/Application Support/com.brightwing.mcp-manager/authd.sock")
    }
    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"))
            .join("brightwing-authd.sock")
    }
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(r"\\.\pipe\brightwing-authd")
    }
}

/// Get the default data directory for the auth daemon.
pub fn default_data_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("Library/Application Support/com.brightwing.mcp-manager")
    }
    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"))
    }
    #[cfg(target_os = "windows")]
    {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("C:\\ProgramData"))
            .join("Brightwing")
    }
}

// ─── IpcListener (server side, used by authd) ────────────────────────────────

/// Cross-platform IPC listener for the auth daemon.
pub struct IpcListener {
    #[cfg(unix)]
    inner: tokio::net::UnixListener,
    #[cfg(windows)]
    pipe_name: String,
    #[cfg(windows)]
    server: tokio::net::windows::named_pipe::NamedPipeServer,
}

impl IpcListener {
    /// Bind to the given IPC path. On Unix, binds a socket. On Windows, creates a named pipe.
    pub fn bind(path: &Path) -> std::io::Result<Self> {
        #[cfg(unix)]
        {
            let listener = tokio::net::UnixListener::bind(path)?;
            Ok(Self { inner: listener })
        }
        #[cfg(windows)]
        {
            use tokio::net::windows::named_pipe::ServerOptions;
            let pipe_name = path.to_string_lossy().to_string();
            let server = ServerOptions::new()
                .first_pipe_instance(true)
                .create(&pipe_name)?;
            Ok(Self { pipe_name, server })
        }
    }

    /// Accept a new client connection. Returns a server stream that can be split
    /// into reader/writer halves.
    pub async fn accept(&mut self) -> std::io::Result<IpcServerStream> {
        #[cfg(unix)]
        {
            let (stream, _addr) = self.inner.accept().await?;
            Ok(IpcServerStream { inner: stream })
        }
        #[cfg(windows)]
        {
            use tokio::net::windows::named_pipe::ServerOptions;
            // Wait for a client to connect to the current pipe instance
            self.server.connect().await?;
            // Take the connected server instance
            let connected = std::mem::replace(
                &mut self.server,
                ServerOptions::new().create(&self.pipe_name)?,
            );
            Ok(IpcServerStream { inner: connected })
        }
    }
}

/// A connected server-side IPC stream.
pub struct IpcServerStream {
    #[cfg(unix)]
    inner: tokio::net::UnixStream,
    #[cfg(windows)]
    inner: tokio::net::windows::named_pipe::NamedPipeServer,
}

impl IpcServerStream {
    /// Split into reader and writer halves for concurrent use.
    pub fn into_split(self) -> (Lines<BufReader<ServerReadHalf>>, ServerWriteHalf) {
        let (reader, writer) = tokio::io::split(self.inner);
        let lines = BufReader::new(reader).lines();
        (lines, writer)
    }
}

// ─── DaemonClient (client side, used by proxy, bw, lib.rs) ──────────────────

/// Cross-platform IPC client for connecting to the auth daemon.
pub struct DaemonClient {
    writer: WriteHalf,
    reader: Lines<BufReader<ReadHalf>>,
}

impl DaemonClient {
    /// Connect to the daemon at the given path.
    pub async fn connect(path: &Path) -> Result<Self, String> {
        #[cfg(unix)]
        {
            let stream = tokio::net::UnixStream::connect(path)
                .await
                .map_err(|e| {
                    format!(
                        "Cannot connect to Brightwing daemon at {}: {}. Is brightwing-authd running?",
                        path.display(),
                        e
                    )
                })?;
            let (reader, writer) = tokio::io::split(stream);
            let reader = BufReader::new(reader).lines();
            Ok(Self { writer, reader })
        }
        #[cfg(windows)]
        {
            use tokio::net::windows::named_pipe::ClientOptions;
            let pipe_name = path.to_string_lossy();
            let client = ClientOptions::new().open(&*pipe_name).map_err(|e| {
                format!(
                    "Cannot connect to Brightwing daemon at {}: {}. Is brightwing-authd running?",
                    pipe_name, e
                )
            })?;
            let (reader, writer) = tokio::io::split(client);
            let reader = BufReader::new(reader).lines();
            Ok(Self { writer, reader })
        }
    }

    /// Connect to the daemon at the default socket path.
    pub async fn connect_default() -> Result<Self, String> {
        Self::connect(&default_socket_path()).await
    }

    /// Send an IPC request.
    pub async fn send(&mut self, request: &IpcRequest) -> Result<(), String> {
        let bytes = encode_message(request).map_err(|e| format!("Serialize error: {}", e))?;
        self.writer
            .write_all(&bytes)
            .await
            .map_err(|e| format!("Write error: {}", e))
    }

    /// Receive an IPC response.
    pub async fn recv(&mut self) -> Result<IpcResponse, String> {
        let line = self
            .reader
            .next_line()
            .await
            .map_err(|e| format!("Read error: {}", e))?
            .ok_or_else(|| "Daemon closed connection".to_string())?;
        decode_message(line.as_bytes()).map_err(|e| format!("Parse error: {}", e))
    }

    /// Perform the IPC handshake with the daemon.
    pub async fn handshake(&mut self, client_type: ClientType) -> Result<(), String> {
        self.send(&IpcRequest::Handshake(HandshakeRequest {
            client: client_type,
            version: IPC_PROTOCOL_VERSION.to_string(),
        }))
        .await?;

        match self.recv().await? {
            IpcResponse::HandshakeOk { .. } => Ok(()),
            IpcResponse::HandshakeError { message, .. } => Err(message),
            other => Err(format!("Unexpected handshake response: {:?}", other)),
        }
    }

    /// Send a request and receive the response.
    pub async fn request(&mut self, req: &IpcRequest) -> Result<IpcResponse, String> {
        self.send(req).await?;
        self.recv().await
    }
}
