//! http protocol - implement read and write request/response

use std::str::FromStr;

use http::{Method, Request, Response, StatusCode, Uri, Version};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

use super::HttpError;

pub async fn read_request<S>(
    stream: &mut S,
    mut max_headers: usize,
    mut max_headers_size: usize,
) -> Result<Request<()>, HttpError>
where
    S: AsyncReadExt + AsyncBufReadExt + Unpin,
{
    let mut reader = stream.lines();

    let header_str = reader.next_line().await?.ok_or(HttpError::InvalidRequest)?;
    let method_uri_version: Vec<&str> = header_str.split(|c| c == ' ').collect();

    if method_uri_version.len() != 3 {
        return Err(HttpError::InvalidLine(header_str));
    }

    let method = method_uri_version[0].parse::<Method>()?;

    let uri = if method == Method::CONNECT {
        Uri::builder().authority(method_uri_version[1]).build()?
    } else {
        method_uri_version[1].parse::<Uri>()?
    };

    let version = parse_version(method_uri_version[2])?;

    let mut builder = Request::builder().method(method).uri(uri).version(version);

    loop {
        let line = reader.next_line().await?.ok_or(HttpError::InvalidRequest)?;
        if line.is_empty() {
            break;
        }

        if max_headers == 0 {
            return Err(HttpError::HeaderTooLarge);
        }
        max_headers -= 1;

        let (key, value) = if let Some((k, v)) = line.split_once(':') {
            (k, v)
        } else {
            return Err(HttpError::InvalidLine(line));
        };
        let (key, value) = (key.trim_start(), value.trim_start());
        if key.is_empty() || value.is_empty() {
            continue;
        }

        let hdr_size = key.len() + value.len();
        if max_headers_size == 0 || max_headers_size < hdr_size {
            return Err(HttpError::HeaderTooLarge);
        }
        max_headers_size -= hdr_size;

        builder = builder.header(key.trim(), value.trim());
    }

    let request = builder.body(())?;

    Ok(request)
}

pub async fn read_response<S>(
    stream: &mut S,
    mut max_headers: usize,
    mut max_headers_size: usize,
) -> Result<Response<()>, HttpError>
where
    S: AsyncReadExt + AsyncBufReadExt + Unpin,
{
    let mut reader = stream.lines();

    let header_str = reader
        .next_line()
        .await?
        .ok_or(HttpError::InvalidResponse)?;
    let version_status: Vec<&str> = header_str.split(|c| c == ' ').collect();

    if version_status.len() < 3 {
        return Err(HttpError::InvalidLine(header_str));
    }

    let version = parse_version(version_status[0])?;
    let status = StatusCode::from_str(version_status[1])?;

    let mut builder = Response::builder().version(version).status(status);

    loop {
        let line = reader
            .next_line()
            .await?
            .ok_or(HttpError::InvalidResponse)?;
        if line.is_empty() {
            break;
        }

        if max_headers == 0 {
            return Err(HttpError::HeaderTooLarge);
        }
        max_headers -= 1;

        let (key, value) = if let Some((k, v)) = line.split_once(':') {
            (k, v)
        } else {
            return Err(HttpError::InvalidLine(line));
        };
        let (key, value) = (key.trim_start(), value.trim_start());
        if key.is_empty() || value.is_empty() {
            continue;
        }

        let hdr_size = key.len() + value.len();
        if max_headers_size == 0 || max_headers_size < hdr_size {
            return Err(HttpError::HeaderTooLarge);
        }
        max_headers_size -= hdr_size;

        builder = builder.header(key.trim(), value.trim());
    }

    let response = builder.body(())?;

    Ok(response)
}

pub async fn write_request<S>(req: &Request<()>, stream: &mut S) -> Result<(), HttpError>
where
    S: AsyncWriteExt + Unpin,
{
    let buf = format_request(req)?;
    stream.write_all(&buf).await?;

    Ok(())
}

pub async fn write_response<S>(
    resp: &Response<()>,
    stream: &mut S,
    reason: Option<&str>,
) -> Result<(), HttpError>
where
    S: AsyncWriteExt + Unpin,
{
    let buf = format_response(resp, reason)?;
    stream.write_all(&buf).await?;

    Ok(())
}

pub fn format_request(req: &Request<()>) -> Result<Vec<u8>, HttpError> {
    let method = req.method().as_str();
    let uri = req.uri().to_string();
    let version = format_version(req.version())?;
    let estimated_len = method.len()
        + uri.len()
        + version.len()
        + 4
        + req
            .headers()
            .iter()
            .map(|(k, v)| k.as_str().len() + v.as_bytes().len() + 4)
            .sum::<usize>()
        + 2;

    let mut buf: Vec<u8> = Vec::with_capacity(estimated_len);

    buf.extend_from_slice(method.as_bytes());
    buf.extend_from_slice(b" ");
    buf.extend_from_slice(uri.as_bytes());
    buf.extend_from_slice(b" ");
    buf.extend_from_slice(version.as_bytes());
    buf.extend_from_slice(b"\r\n");

    for (key, value) in req.headers().iter() {
        buf.extend_from_slice(canonical_header_key(key.as_str()).as_bytes());
        buf.extend_from_slice(b": ");
        buf.extend_from_slice(value.as_bytes());
        buf.extend_from_slice(b"\r\n");
    }
    buf.extend_from_slice(b"\r\n");

    Ok(buf)
}

pub fn format_response(resp: &Response<()>, reason: Option<&str>) -> Result<Vec<u8>, HttpError> {
    let version = format_version(resp.version())?;
    let status = resp.status();
    let status_str = status.as_str();
    let reason_str = reason.unwrap_or(
        status
            .canonical_reason()
            .ok_or(HttpError::InvalidResponse)?,
    );

    let estimated_len = version.len()
        + status_str.len()
        + reason_str.len()
        + 4
        + resp
            .headers()
            .iter()
            .map(|(k, v)| k.as_str().len() + v.as_bytes().len() + 4)
            .sum::<usize>()
        + 2;

    let mut buf = Vec::with_capacity(estimated_len);

    buf.extend_from_slice(version.as_bytes());
    buf.extend_from_slice(b" ");
    buf.extend_from_slice(status_str.as_bytes());
    buf.extend_from_slice(b" ");
    buf.extend_from_slice(reason_str.as_bytes());
    buf.extend_from_slice(b"\r\n");

    for (key, value) in resp.headers().iter() {
        buf.extend_from_slice(canonical_header_key(key.as_str()).as_bytes());
        buf.extend_from_slice(b": ");
        buf.extend_from_slice(value.as_bytes());
        buf.extend_from_slice(b"\r\n");
    }
    buf.extend_from_slice(b"\r\n");

    Ok(buf)
}

fn parse_version(version: &str) -> Result<http::Version, HttpError> {
    match version {
        "HTTP/0.9" => Ok(Version::HTTP_09),
        "HTTP/1.0" => Ok(Version::HTTP_10),
        "HTTP/1.1" => Ok(Version::HTTP_11),
        "HTTP/2.0" => Ok(Version::HTTP_2),
        "HTTP/3.0" => Ok(Version::HTTP_3),
        _ => Err(HttpError::InvalidVersion),
    }
}

fn format_version(version: http::Version) -> Result<&'static str, HttpError> {
    match version {
        Version::HTTP_09 => Ok("HTTP/0.9"),
        Version::HTTP_10 => Ok("HTTP/1.0"),
        Version::HTTP_11 => Ok("HTTP/1.1"),
        Version::HTTP_2 => Ok("HTTP/2.0"),
        Version::HTTP_3 => Ok("HTTP/3.0"),
        _ => Err(HttpError::InvalidVersion),
    }
}

fn canonical_header_key(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut upper = true;

    for c in s.chars() {
        if c == '-' {
            upper = true;
            result.push(c);
        } else {
            if upper {
                result.push(c.to_ascii_uppercase());
                upper = false;
            } else {
                result.push(c.to_ascii_lowercase());
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Cursor;

    #[tokio::test]
    async fn test_request() {
        let data =
            b"CONNECT bing.com HTTP/1.1\r\nHost: bing.com\r\nContent-Type: json\r\nContent-Length: 0\r\n\r\n".to_vec();
        let mut data = Cursor::new(data);
        let req = read_request(&mut data, 64, 65535).await.unwrap();
        println!("{:?}", req);
        let mut req_data = Cursor::new(vec![]);
        write_request(&req, &mut req_data).await.unwrap();
        assert_eq!(req_data.into_inner(), data.into_inner());
    }

    #[tokio::test]
    async fn test_response() {
        let data = b"HTTP/1.1 200 Connection established\r\nServer: ExampleServer/1.0\r\nContent-Length: 0\r\nConnection: keep-alive\r\nCache-Control: no-cache\r\n\r\n".to_vec();
        let mut data = Cursor::new(data);
        let resp = read_response(&mut data, 64, 65535).await.unwrap();
        println!("{:?}", resp);
        let mut resp_data = Cursor::new(vec![]);
        write_response(&resp, &mut resp_data, Some("Connection established"))
            .await
            .unwrap();
        assert_eq!(resp_data.into_inner(), data.into_inner());
    }
}
