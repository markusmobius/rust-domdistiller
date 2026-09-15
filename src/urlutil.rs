use std::{borrow::Cow, fmt};

#[derive(Clone, Debug, Default)]
pub(crate) struct Url {
    pub scheme: String,
    pub opaque: String,
    pub user: Option<(Vec<u8>, Option<Vec<u8>>)>,
    pub host: Vec<u8>,
    pub path: Vec<u8>,
    pub raw_path: String,
    pub query: String,
    pub force_query: bool,
    pub fragment: Vec<u8>,
    pub raw_fragment: String,
    pub omit_host: bool,
}

#[derive(Clone, Copy)]
enum Encoding {
    Path,
    Fragment,
    Host,
    User,
}

fn allowed(byte: u8, encoding: Encoding) -> bool {
    if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
        return true;
    }
    match encoding {
        Encoding::Path => b"$&+,/:;=@".contains(&byte),
        Encoding::Fragment => b"$&+,/:;=?@!()*".contains(&byte),
        Encoding::Host => b"!$&'()*+,;=:[]<>\"".contains(&byte),
        Encoding::User => b"$&+,;=".contains(&byte),
    }
}

fn escape(value: &[u8], encoding: Encoding) -> String {
    let mut output = String::new();
    for &byte in value {
        if allowed(byte, encoding) {
            output.push(byte as char);
        } else {
            const HEX: &[u8] = b"0123456789ABCDEF";
            output.push('%');
            output.push(HEX[(byte >> 4) as usize] as char);
            output.push(HEX[(byte & 15) as usize] as char);
        }
    }
    output
}

pub(crate) fn unescape(value: &str) -> Option<Vec<u8>> {
    let bytes = value.as_bytes();
    if !bytes.contains(&b'%') {
        return Some(bytes.to_vec());
    }
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len()
                || !bytes[index + 1].is_ascii_hexdigit()
                || !bytes[index + 2].is_ascii_hexdigit()
            {
                return None;
            }
            index += 3;
        } else {
            index += 1;
        }
    }
    Some(percent_encoding::percent_decode_str(value).collect())
}

fn encoded(raw: &str, decoded: &[u8], encoding: Encoding) -> String {
    if !raw.is_empty()
        && raw
            .bytes()
            .all(|byte| b"!$&'()*+,;=:@[]%".contains(&byte) || allowed(byte, encoding))
        && unescape(raw).as_deref() == Some(decoded)
    {
        return raw.into();
    }
    escape(decoded, encoding)
}

fn port_valid(value: &str) -> bool {
    value.is_empty()
        || value
            .strip_prefix(':')
            .is_some_and(|port| port.bytes().all(|byte| byte.is_ascii_digit()))
}

fn parse_host(scheme: &str, value: &str) -> Option<Vec<u8>> {
    let mut zone_start = None;
    if let Some(bracket) = value.rfind('[') {
        if bracket != 0 {
            return None;
        }
        let end = value.rfind(']')?;
        if !port_valid(&value[end + 1..]) {
            return None;
        }
        let hostname = &value[1..end];
        let address = if let Some(zone) = hostname.find("%25") {
            zone_start = Some(zone + 1);
            let decoded_zone = unescape(&hostname[zone + 3..])?;
            if decoded_zone.is_empty() {
                return None;
            }
            &hostname[..zone]
        } else {
            hostname
        };
        String::from_utf8(unescape(address)?)
            .ok()?
            .parse::<std::net::Ipv6Addr>()
            .ok()?;
    } else if let Some(first) = value.find(':') {
        let index = if matches!(scheme, "http" | "https") {
            first
        } else {
            value.rfind(':').unwrap()
        };
        if !port_valid(&value[index..]) {
            return None;
        }
    }
    let mut index = 0;
    let bytes = value.as_bytes();
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'%' {
            let end = index.checked_add(3)?;
            let part = value.get(index..end)?;
            let decoded = unescape(part)?[0];
            if zone_start.is_some_and(|start| index >= start) {
                if decoded != b'%' && decoded != b' ' && !allowed(decoded, Encoding::Host) {
                    return None;
                }
            } else if decoded < 128 && part != "%25" {
                return None;
            }
            index = end;
        } else {
            if byte < 128 && !allowed(byte, Encoding::Host) {
                return None;
            }
            index += 1;
        }
    }
    unescape(value)
}

impl Url {
    pub fn parse(value: &str) -> Option<Self> {
        Self::parse_inner(value, false)
    }

    pub fn request(value: &str) -> Option<Self> {
        Self::parse_inner(value, true)
    }

    fn parse_inner(value: &str, request: bool) -> Option<Self> {
        let mut url = Self::default();
        let value = if !request {
            if let Some((value, fragment)) = value.split_once('#') {
                url.fragment = unescape(fragment)?;
                url.raw_fragment = fragment.into();
                value
            } else {
                value
            }
        } else {
            value
        };
        if value.bytes().any(|byte| byte < 32 || byte == 127) || (request && value.is_empty()) {
            return None;
        }
        if value == "*" {
            url.path = b"*".to_vec();
            return Some(url);
        }
        let mut rest = value;
        for (index, byte) in value.bytes().enumerate() {
            if byte.is_ascii_alphabetic() {
                continue;
            }
            if byte.is_ascii_digit() || b"+-.".contains(&byte) {
                if index == 0 {
                    break;
                }
            } else if byte == b':' {
                if index == 0 {
                    return None;
                }
                url.scheme = value[..index].to_ascii_lowercase();
                rest = &value[index + 1..];
                break;
            } else {
                break;
            }
        }
        if let Some((path, query)) = rest.split_once('?') {
            url.force_query = query.is_empty();
            url.query = query.into();
            rest = path;
        }
        if !rest.starts_with('/') {
            if !url.scheme.is_empty() {
                url.opaque = rest.into();
                return Some(url);
            }
            if request
                || rest
                    .split('/')
                    .next()
                    .is_some_and(|part| part.contains(':'))
            {
                return None;
            }
        }
        if (!url.scheme.is_empty() || (!request && !rest.starts_with("///")))
            && rest.starts_with("//")
        {
            let authority = &rest[2..];
            let end = authority.find('/').unwrap_or(authority.len());
            rest = &authority[end..];
            let authority = &authority[..end];
            let host = if let Some((user, host)) = authority.rsplit_once('@') {
                if !user.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:%@".contains(&byte)
                }) {
                    return None;
                }
                url.user = Some(if let Some((name, password)) = user.split_once(':') {
                    (unescape(name)?, Some(unescape(password)?))
                } else {
                    (unescape(user)?, None)
                });
                host
            } else {
                authority
            };
            url.host = parse_host(&url.scheme, host)?;
        } else if !url.scheme.is_empty() && rest.starts_with('/') {
            url.omit_host = true;
        }
        url.path = unescape(rest)?;
        url.raw_path = rest.into();
        Some(url)
    }

    pub fn hostname(&self) -> Cow<'_, str> {
        let mut host = self.host.as_slice();
        if let Some(colon) = host.iter().rposition(|&byte| byte == b':') {
            if host[colon + 1..].iter().all(u8::is_ascii_digit) {
                host = &host[..colon];
            }
        }
        if host.starts_with(b"[") && host.ends_with(b"]") {
            host = &host[1..host.len() - 1];
        }
        String::from_utf8_lossy(host)
    }

    pub fn escaped_path(&self) -> String {
        if self.path == b"*" {
            "*".into()
        } else {
            encoded(&self.raw_path, &self.path, Encoding::Path)
        }
    }

    pub fn set_path(&mut self, path: &[u8]) {
        self.path = path.to_vec();
        self.raw_path.clear();
    }

    pub fn clear_fragment(&mut self) {
        self.fragment.clear();
        self.raw_fragment.clear();
    }

    pub fn resolve(&self, mut reference: Self) -> Self {
        let absolute =
            !reference.scheme.is_empty() || !reference.host.is_empty() || reference.user.is_some();
        if reference.scheme.is_empty() {
            reference.scheme.clone_from(&self.scheme);
        }
        if absolute {
            let path = resolve_path(&reference.escaped_path(), "");
            reference.path = unescape(&path).unwrap();
            reference.raw_path = path;
            return reference;
        }
        if !reference.opaque.is_empty() {
            reference.user = None;
            reference.host.clear();
            reference.path.clear();
            return reference;
        }
        if reference.path.is_empty() && !reference.force_query && reference.query.is_empty() {
            reference.query.clone_from(&self.query);
            if reference.fragment.is_empty() {
                reference.fragment.clone_from(&self.fragment);
                reference.raw_fragment.clone_from(&self.raw_fragment);
            }
        }
        if reference.path.is_empty() && !self.opaque.is_empty() {
            reference.opaque.clone_from(&self.opaque);
            reference.user = None;
            reference.host.clear();
            reference.path.clear();
            return reference;
        }
        reference.host.clone_from(&self.host);
        reference.user.clone_from(&self.user);
        let path = resolve_path(&self.escaped_path(), &reference.escaped_path());
        reference.path = unescape(&path).unwrap();
        reference.raw_path = path;
        reference
    }

    pub fn unescaped(&self) -> String {
        self.render(false)
    }

    fn render(&self, escaped: bool) -> String {
        let mut output = String::new();
        if !self.scheme.is_empty() {
            output.push_str(&self.scheme);
            output.push(':');
        }
        if !self.opaque.is_empty() {
            output.push_str(&self.opaque);
        } else {
            let omit_host =
                escaped && self.omit_host && self.host.is_empty() && self.user.is_none();
            if (!self.scheme.is_empty() || !self.host.is_empty() || self.user.is_some())
                && !omit_host
            {
                if !self.host.is_empty() || !self.path.is_empty() || self.user.is_some() {
                    output.push_str("//");
                }
                if let Some((user, password)) = &self.user {
                    output.push_str(&escape(user, Encoding::User));
                    if let Some(password) = password {
                        output.push(':');
                        output.push_str(&escape(password, Encoding::User));
                    }
                    output.push('@');
                }
                if escaped {
                    output.push_str(&escape(&self.host, Encoding::Host));
                } else {
                    output.push_str(&String::from_utf8_lossy(&self.host));
                }
            }
            let mut path = if escaped {
                self.escaped_path()
            } else {
                String::from_utf8_lossy(&self.path).into_owned()
            };
            if omit_host && path.starts_with("//") {
                output.push_str("%2F");
                path.remove(0);
            }
            if !path.is_empty() && !path.starts_with('/') && !self.host.is_empty() {
                output.push('/');
            }
            if output.is_empty()
                && path
                    .split('/')
                    .next()
                    .is_some_and(|part| part.contains(':'))
            {
                output.push_str("./");
            }
            output.push_str(&path);
        }
        if self.force_query || !self.query.is_empty() {
            output.push('?');
            output.push_str(&self.query);
        }
        if !self.fragment.is_empty() {
            output.push('#');
            output.push_str(&encoded(
                &self.raw_fragment,
                &self.fragment,
                Encoding::Fragment,
            ));
        }
        output
    }
}

impl fmt::Display for Url {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.render(true))
    }
}

fn resolve_path(base: &str, reference: &str) -> String {
    let full = if reference.is_empty() {
        base.into()
    } else if !reference.starts_with('/') {
        format!(
            "{}{reference}",
            &base[..base.rfind('/').map_or(0, |index| index + 1)]
        )
    } else {
        reference.to_owned()
    };
    if full.is_empty() {
        return full;
    }
    let mut output = String::from("/");
    let mut first = true;
    let mut last = "";
    for part in full.split('/') {
        last = part;
        match part {
            "." => {
                first = false;
                continue;
            }
            ".." => {
                if let Some(index) = output[1..].rfind('/') {
                    output.truncate(index + 1);
                } else {
                    output.truncate(1);
                }
                first = output.len() == 1;
            }
            _ => {
                if !first {
                    output.push('/');
                }
                output.push_str(part);
                first = false;
            }
        }
    }
    if matches!(last, "." | "..") {
        output.push('/');
    }
    if output.starts_with("//") {
        output.remove(0);
    }
    output
}
