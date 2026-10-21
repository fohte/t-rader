use std::collections::HashSet;

use core_application::kata_exec::{ExecFile, ExecRequest, ExecResult, KataExecError};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::config::{
    DEFAULT_FILE_CHUNK_BYTES, DEFAULT_MAX_FILE_BYTES, DEFAULT_MAX_FILE_COUNT,
    DEFAULT_MAX_TOTAL_FILE_BYTES, FileTransferLimits, MAX_FILE_TRANSFER_OUTPUT_BYTES,
};

const STREAM_MAGIC: &[u8] = b"T-RADER-FILE-TRANSFER-1\n";
const MAX_MANIFEST_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileDescriptor {
    path: String,
    bytes: u64,
}

#[derive(Serialize)]
struct TransferManifest<'a> {
    version: u32,
    code: &'a str,
    stdin: &'a str,
    files: Vec<FileDescriptor>,
    limits: TransferLimitsWire,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct TransferLimitsWire {
    max_files: usize,
    max_file_bytes: usize,
    max_total_bytes: usize,
    chunk_bytes: usize,
    max_output_bytes: usize,
}

impl From<(FileTransferLimits, usize)> for TransferLimitsWire {
    fn from((limits, max_output_bytes): (FileTransferLimits, usize)) -> Self {
        Self {
            max_files: limits.max_files,
            max_file_bytes: limits.max_file_bytes,
            max_total_bytes: limits.max_total_bytes,
            chunk_bytes: limits.chunk_bytes,
            max_output_bytes,
        }
    }
}

#[derive(Deserialize)]
struct TransferResponse {
    stdout_bytes: u64,
    stderr_bytes: u64,
    exit_code: i32,
    #[serde(default)]
    files: Vec<FileDescriptor>,
    #[serde(default)]
    error: Option<String>,
}

pub(crate) fn validate_input_files(
    files: &[ExecFile],
    limits: FileTransferLimits,
) -> Result<(), KataExecError> {
    validate_limits(limits)?;
    validate_descriptors(
        &files
            .iter()
            .map(|file| FileDescriptor {
                path: file.path.clone(),
                bytes: file.bytes.len() as u64,
            })
            .collect::<Vec<_>>(),
        limits,
    )?;
    Ok(())
}

fn validate_limits(limits: FileTransferLimits) -> Result<(), KataExecError> {
    if limits.max_files > DEFAULT_MAX_FILE_COUNT
        || limits.max_file_bytes > DEFAULT_MAX_FILE_BYTES
        || limits.max_total_bytes > DEFAULT_MAX_TOTAL_FILE_BYTES
        || limits.chunk_bytes == 0
        || limits.chunk_bytes > DEFAULT_FILE_CHUNK_BYTES
    {
        return Err(KataExecError::FileTransfer(
            "invalid configured chunk size".to_string(),
        ));
    }
    Ok(())
}

fn validate_descriptors(
    files: &[FileDescriptor],
    limits: FileTransferLimits,
) -> Result<usize, KataExecError> {
    if files.len() > limits.max_files {
        return Err(KataExecError::FileTransfer(format!(
            "file count exceeds {}",
            limits.max_files
        )));
    }

    let mut total = 0usize;
    let mut paths = HashSet::with_capacity(files.len());
    for file in files {
        validate_relative_path(&file.path)?;
        if !paths.insert(file.path.as_str()) {
            return Err(KataExecError::FileTransfer(
                "duplicate file path".to_string(),
            ));
        }
        let file_bytes = usize::try_from(file.bytes)
            .map_err(|_| KataExecError::FileTransfer("file size is not supported".to_string()))?;
        if file_bytes > limits.max_file_bytes {
            return Err(KataExecError::FileTransfer(format!(
                "file exceeds {} bytes",
                limits.max_file_bytes
            )));
        }
        total = total
            .checked_add(file_bytes)
            .ok_or_else(|| KataExecError::FileTransfer("total file size overflow".to_string()))?;
        if total > limits.max_total_bytes {
            return Err(KataExecError::FileTransfer(format!(
                "total file bytes exceed {}",
                limits.max_total_bytes
            )));
        }
    }
    for file in files {
        let parts: Vec<_> = file.path.split('/').collect();
        let mut parent = String::new();
        for part in parts.iter().take(parts.len() - 1) {
            if !parent.is_empty() {
                parent.push('/');
            }
            parent.push_str(part);
            if paths.contains(parent.as_str()) {
                return Err(KataExecError::FileTransfer(
                    "conflicting file paths".to_string(),
                ));
            }
        }
    }
    Ok(total)
}

pub(crate) fn validate_relative_path(path: &str) -> Result<(), KataExecError> {
    let parts: Vec<_> = path.split('/').collect();
    if path.is_empty()
        || path.len() > 4096
        || path.contains('\\')
        || path.contains('\0')
        || parts
            .iter()
            .any(|part| part.is_empty() || *part == "." || *part == ".." || part.len() > 255)
    {
        return Err(KataExecError::FileTransfer(
            "file path must be a normalized relative path".to_string(),
        ));
    }
    Ok(())
}

pub(crate) async fn transfer<W, R>(
    stdin: &mut W,
    stdout: &mut R,
    request: &ExecRequest,
    limits: FileTransferLimits,
    max_output_bytes: usize,
) -> Result<ExecResult, KataExecError>
where
    W: AsyncWrite + Unpin,
    R: AsyncRead + Unpin,
{
    validate_limits(limits)?;
    if max_output_bytes > MAX_FILE_TRANSFER_OUTPUT_BYTES {
        return Err(KataExecError::FileTransfer(
            "configured text output limit exceeds the transfer ceiling".to_string(),
        ));
    }
    let files = request.input_files.as_deref().ok_or_else(|| {
        KataExecError::FileTransfer("file transfer was not requested".to_string())
    })?;
    validate_input_files(files, limits)?;

    let manifest = TransferManifest {
        version: 1,
        code: &request.code,
        stdin: request.stdin.as_deref().unwrap_or_default(),
        files: files
            .iter()
            .map(|file| FileDescriptor {
                path: file.path.clone(),
                bytes: file.bytes.len() as u64,
            })
            .collect(),
        limits: (limits, max_output_bytes).into(),
    };
    write_header(stdin, &manifest).await?;
    for file in files {
        for chunk in file.bytes.chunks(limits.chunk_bytes) {
            write_chunk(stdin, chunk).await?;
        }
        write_chunk(stdin, &[]).await?;
    }
    stdin
        .shutdown()
        .await
        .map_err(|error| transfer_error("failed to close input stream", error))?;

    read_response(stdout, limits, max_output_bytes).await
}

async fn write_header<W, T>(writer: &mut W, header: &T) -> Result<(), KataExecError>
where
    W: AsyncWrite + Unpin,
    T: Serialize,
{
    let encoded = serde_json::to_vec(header)
        .map_err(|error| KataExecError::FileTransfer(error.to_string()))?;
    if encoded.len() > MAX_MANIFEST_BYTES || encoded.len() > u32::MAX as usize {
        return Err(KataExecError::FileTransfer(
            "transfer manifest exceeds the limit".to_string(),
        ));
    }
    writer
        .write_all(STREAM_MAGIC)
        .await
        .map_err(|error| transfer_error("failed to write stream header", error))?;
    writer
        .write_u32(encoded.len() as u32)
        .await
        .map_err(|error| transfer_error("failed to write manifest length", error))?;
    writer
        .write_all(&encoded)
        .await
        .map_err(|error| transfer_error("failed to write manifest", error))
}

async fn write_chunk<W>(writer: &mut W, chunk: &[u8]) -> Result<(), KataExecError>
where
    W: AsyncWrite + Unpin,
{
    let length = u32::try_from(chunk.len())
        .map_err(|_| KataExecError::FileTransfer("chunk size is not supported".to_string()))?;
    writer
        .write_u32(length)
        .await
        .map_err(|error| transfer_error("failed to write chunk length", error))?;
    writer
        .write_all(chunk)
        .await
        .map_err(|error| transfer_error("failed to write chunk", error))
}

async fn read_response<R>(
    stdout: &mut R,
    limits: FileTransferLimits,
    max_output_bytes: usize,
) -> Result<ExecResult, KataExecError>
where
    R: AsyncRead + Unpin,
{
    let mut magic = vec![0; STREAM_MAGIC.len()];
    stdout
        .read_exact(&mut magic)
        .await
        .map_err(|error| transfer_error("stream closed before a response", error))?;
    if magic != STREAM_MAGIC {
        return Err(KataExecError::FileTransfer(
            "invalid stream header".to_string(),
        ));
    }
    let header_bytes = stdout
        .read_u32()
        .await
        .map_err(|error| transfer_error("stream closed before the response manifest", error))?
        as usize;
    if header_bytes > MAX_MANIFEST_BYTES {
        return Err(KataExecError::FileTransfer(
            "response manifest exceeds the limit".to_string(),
        ));
    }
    let mut encoded = vec![0; header_bytes];
    stdout
        .read_exact(&mut encoded)
        .await
        .map_err(|error| transfer_error("response manifest was truncated", error))?;
    let response: TransferResponse = serde_json::from_slice(&encoded)
        .map_err(|error| KataExecError::FileTransfer(error.to_string()))?;
    if let Some(error) = response.error {
        return Err(KataExecError::FileTransfer(error));
    }
    let stdout_bytes = usize::try_from(response.stdout_bytes)
        .map_err(|_| KataExecError::FileTransfer("stdout size is not supported".to_string()))?;
    let stderr_bytes = usize::try_from(response.stderr_bytes)
        .map_err(|_| KataExecError::FileTransfer("stderr size is not supported".to_string()))?;
    if stdout_bytes
        .checked_add(stderr_bytes)
        .is_none_or(|bytes| bytes > max_output_bytes)
    {
        return Err(KataExecError::OutputTooLarge {
            limit: max_output_bytes,
        });
    }
    let stdout_text =
        read_chunked_bytes(stdout, stdout_bytes, limits.chunk_bytes, "stdout").await?;
    let stderr_text =
        read_chunked_bytes(stdout, stderr_bytes, limits.chunk_bytes, "stderr").await?;

    let total_bytes = validate_descriptors(&response.files, limits)?;
    let mut result_files = Vec::with_capacity(response.files.len());
    for descriptor in response.files {
        let expected_bytes = usize::try_from(descriptor.bytes)
            .map_err(|_| KataExecError::FileTransfer("file size is not supported".to_string()))?;
        let bytes =
            read_chunked_bytes(stdout, expected_bytes, limits.chunk_bytes, "output file").await?;
        result_files.push(ExecFile {
            path: descriptor.path,
            bytes,
        });
    }
    debug_assert!(total_bytes <= limits.max_total_bytes);

    let mut extra = [0; 1];
    let bytes_read = stdout
        .read(&mut extra)
        .await
        .map_err(|error| transfer_error("failed to finish output stream", error))?;
    if bytes_read != 0 {
        return Err(KataExecError::FileTransfer(
            "unexpected data after the response".to_string(),
        ));
    }

    Ok(ExecResult {
        stdout: String::from_utf8_lossy(&stdout_text).into_owned(),
        stderr: String::from_utf8_lossy(&stderr_text).into_owned(),
        exit_code: response.exit_code,
        files: result_files,
    })
}

async fn read_chunked_bytes<R>(
    reader: &mut R,
    expected_bytes: usize,
    chunk_limit: usize,
    context: &str,
) -> Result<Vec<u8>, KataExecError>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = Vec::with_capacity(expected_bytes);
    loop {
        let chunk_bytes = reader
            .read_u32()
            .await
            .map_err(|error| transfer_error(&format!("{context} chunk was truncated"), error))?
            as usize;
        if chunk_bytes == 0 {
            break;
        }
        if chunk_bytes > chunk_limit {
            return Err(KataExecError::FileTransfer(format!(
                "{context} chunk exceeds the configured limit"
            )));
        }
        let next_len = bytes
            .len()
            .checked_add(chunk_bytes)
            .ok_or_else(|| KataExecError::FileTransfer(format!("{context} size overflow")))?;
        if next_len > expected_bytes {
            return Err(KataExecError::FileTransfer(format!(
                "{context} exceeds its declared size"
            )));
        }
        let start = bytes.len();
        bytes.resize(next_len, 0);
        reader
            .read_exact(&mut bytes[start..])
            .await
            .map_err(|error| transfer_error(&format!("{context} was truncated"), error))?;
    }
    if bytes.len() != expected_bytes {
        return Err(KataExecError::FileTransfer(format!(
            "{context} size did not match its manifest"
        )));
    }
    Ok(bytes)
}

pub(crate) async fn drain_stderr<R>(reader: &mut R) -> Result<(), KataExecError>
where
    R: AsyncRead + Unpin,
{
    let mut buffer = [0; 4096];
    loop {
        let bytes_read = reader
            .read(&mut buffer)
            .await
            .map_err(|error| transfer_error("failed to read attach stderr", error))?;
        if bytes_read == 0 {
            return Ok(());
        }
    }
}

fn transfer_error(context: &str, error: std::io::Error) -> KataExecError {
    KataExecError::FileTransfer(format!("{context}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::io::duplex;

    fn limits() -> FileTransferLimits {
        FileTransferLimits {
            max_files: 2,
            max_file_bytes: 8,
            max_total_bytes: 12,
            chunk_bytes: 3,
        }
    }

    #[tokio::test]
    async fn sends_bounded_chunks_and_collects_file_outputs() {
        let request = ExecRequest::new("print('ok')").with_files(vec![ExecFile {
            path: "input/data.bin".to_string(),
            bytes: b"abcdefg".to_vec(),
        }]);
        let (mut host_input, mut wrapper_input) = duplex(64);
        let (mut wrapper_output, mut host_output) = duplex(64);
        let wrapper = tokio::spawn(async move {
            let mut magic = vec![0; STREAM_MAGIC.len()];
            wrapper_input.read_exact(&mut magic).await.unwrap();
            let manifest_size = wrapper_input.read_u32().await.unwrap() as usize;
            let mut manifest = vec![0; manifest_size];
            wrapper_input.read_exact(&mut manifest).await.unwrap();
            let manifest: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
            let received = read_test_file(&mut wrapper_input).await;
            let mut response = json!({
                "stdout_bytes": 3,
                "stderr_bytes": 0,
                "exit_code": 0,
                "files": [{"path": "output/result.bin", "bytes": 5}],
            });
            write_test_response(&mut wrapper_output, &mut response, b"ok\n", b"", b"12345").await;
            (magic, manifest, received)
        });

        let result = transfer(&mut host_input, &mut host_output, &request, limits(), 64)
            .await
            .unwrap();
        assert_eq!(
            (wrapper.await.unwrap(), result),
            (
                (
                    STREAM_MAGIC.to_vec(),
                    json!({
                        "version": 1,
                        "code": "print('ok')",
                        "stdin": "",
                        "files": [{"path": "input/data.bin", "bytes": 7}],
                        "limits": {
                            "max_files": 2,
                            "max_file_bytes": 8,
                            "max_total_bytes": 12,
                            "chunk_bytes": 3,
                            "max_output_bytes": 64,
                        },
                    }),
                    (b"abcdefg".to_vec(), vec![3, 3, 1]),
                ),
                ExecResult {
                    stdout: "ok\n".to_string(),
                    stderr: String::new(),
                    exit_code: 0,
                    files: vec![ExecFile {
                        path: "output/result.bin".to_string(),
                        bytes: b"12345".to_vec(),
                    }],
                },
            ),
        );
    }

    #[rstest::rstest]
    #[case::traversal("../outside")]
    #[case::absolute("/outside")]
    #[case::empty_component("a//b")]
    #[case::backslash("a\\b")]
    fn rejects_unsafe_paths(#[case] path: &str) {
        assert_eq!(
            validate_relative_path(path),
            Err(KataExecError::FileTransfer(
                "file path must be a normalized relative path".to_string(),
            )),
        );
    }

    #[rstest::rstest]
    #[case::file_count(&[("a", 1), ("b", 1), ("c", 1)], "file count exceeds 2")]
    #[case::total_bytes(&[("a", 7), ("b", 6)], "total file bytes exceed 12")]
    #[case::single_file_bytes(&[("a", 9)], "file exceeds 8 bytes")]
    #[case::path_conflict(&[("item", 1), ("item/child", 1)], "conflicting file paths")]
    fn rejects_file_transfer_limits(#[case] input: &[(&str, usize)], #[case] expected: &str) {
        let files = input
            .iter()
            .map(|(path, size)| ExecFile {
                path: (*path).to_string(),
                bytes: vec![1; *size],
            })
            .collect::<Vec<_>>();
        assert_eq!(
            validate_input_files(&files, limits()),
            Err(KataExecError::FileTransfer(expected.to_string())),
        );
    }

    #[tokio::test]
    async fn reports_a_disconnected_attach_stream() {
        let request = ExecRequest::new("pass").with_files(Vec::new());
        let (mut host_input, wrapper_input) = duplex(64);
        let (wrapper_output, mut host_output) = duplex(64);
        drop(wrapper_input);
        drop(wrapper_output);

        let result = transfer(&mut host_input, &mut host_output, &request, limits(), 64)
            .await
            .map_err(|error| match error {
                KataExecError::FileTransfer(_) => "file transfer",
                _ => "other",
            });
        assert_eq!(result, Err("file transfer"));
    }

    #[tokio::test]
    async fn rejects_text_output_above_the_limit() {
        let request = ExecRequest::new("pass").with_files(Vec::new());
        let (mut host_input, mut wrapper_input) = duplex(1024);
        let (mut wrapper_output, mut host_output) = duplex(1024);
        let wrapper = tokio::spawn(async move {
            let mut input = Vec::new();
            wrapper_input.read_to_end(&mut input).await.unwrap();
            let mut response = json!({
                "stdout_bytes": 5,
                "stderr_bytes": 0,
                "exit_code": 0,
                "files": [],
            });
            write_test_response(&mut wrapper_output, &mut response, b"large", b"", b"").await;
        });

        assert_eq!(
            transfer(&mut host_input, &mut host_output, &request, limits(), 4).await,
            Err(KataExecError::OutputTooLarge { limit: 4 }),
        );
        wrapper.await.unwrap();
    }

    #[tokio::test]
    async fn returns_timeout_when_the_wrapper_keeps_output_open() {
        let request = ExecRequest::new("pass").with_files(Vec::new());
        let (mut host_input, mut wrapper_input) = duplex(1024);
        let (wrapper_output, mut host_output) = duplex(1024);
        let wrapper = tokio::spawn(async move {
            let mut input = Vec::new();
            wrapper_input.read_to_end(&mut input).await.unwrap();
            let _wrapper_output = wrapper_output;
            std::future::pending::<()>().await;
        });

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(20),
            transfer(&mut host_input, &mut host_output, &request, limits(), 64),
        )
        .await;
        let outcome = match result {
            Ok(result) => result.map_err(|_| "transfer failed"),
            Err(_) => Err("timeout"),
        };
        assert_eq!(outcome, Err("timeout"));
        wrapper.abort();
    }

    async fn read_test_file<R: AsyncRead + Unpin>(reader: &mut R) -> (Vec<u8>, Vec<usize>) {
        let mut output = Vec::new();
        let mut chunk_sizes = Vec::new();
        loop {
            let size = reader.read_u32().await.unwrap() as usize;
            if size == 0 {
                return (output, chunk_sizes);
            }
            chunk_sizes.push(size);
            let start = output.len();
            output.resize(start + size, 0);
            reader.read_exact(&mut output[start..]).await.unwrap();
        }
    }

    async fn write_test_response<W: AsyncWrite + Unpin>(
        writer: &mut W,
        response: &mut serde_json::Value,
        stdout: &[u8],
        stderr: &[u8],
        file_bytes: &[u8],
    ) {
        let encoded = serde_json::to_vec(response).unwrap();
        writer.write_all(STREAM_MAGIC).await.unwrap();
        writer.write_u32(encoded.len() as u32).await.unwrap();
        writer.write_all(&encoded).await.unwrap();
        for text in [stdout, stderr] {
            for chunk in text.chunks(3) {
                writer.write_u32(chunk.len() as u32).await.unwrap();
                writer.write_all(chunk).await.unwrap();
            }
            writer.write_u32(0).await.unwrap();
        }
        for chunk in file_bytes.chunks(3) {
            writer.write_u32(chunk.len() as u32).await.unwrap();
            writer.write_all(chunk).await.unwrap();
        }
        writer.write_u32(0).await.unwrap();
    }
}
