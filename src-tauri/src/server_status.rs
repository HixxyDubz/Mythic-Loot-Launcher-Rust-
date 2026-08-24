use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{SocketAddr, TcpStream, ToSocketAddrs, UdpSocket},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::models::GameProfile;

const CACHE_TTL: Duration = Duration::from_secs(45);
const IO_TIMEOUT: Duration = Duration::from_millis(2_500);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub profile_id: String,
    pub configured: bool,
    pub checked: bool,
    pub online: Option<bool>,
    pub players: Option<u32>,
    pub max_players: Option<u32>,
    pub latency_ms: Option<u128>,
    pub version: String,
    pub motd: String,
    pub map: String,
    pub message: String,
    pub cached: bool,
    pub checked_at_epoch: Option<u64>,
}

impl ServerStatus {
    pub fn not_checked(profile: &GameProfile) -> Self {
        let configured = !profile.server_ip.trim().is_empty();
        Self {
            profile_id: profile.id.clone(),
            configured,
            checked: false,
            online: None,
            players: None,
            max_players: None,
            latency_ms: None,
            version: String::new(),
            motd: String::new(),
            map: String::new(),
            message: if configured {
                "Select Refresh status to check the server".into()
            } else {
                "Add the private server address in settings".into()
            },
            cached: false,
            checked_at_epoch: None,
        }
    }
}

static CACHE: OnceLock<Mutex<HashMap<String, (Instant, ServerStatus)>>> = OnceLock::new();

pub fn query(profile: &GameProfile, use_cache: bool) -> ServerStatus {
    if profile.server_ip.trim().is_empty() {
        return ServerStatus::not_checked(profile);
    }
    let key = format!(
        "{}:{}:{}",
        profile.id, profile.server_ip, profile.server_port
    );
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if use_cache
        && let Ok(guard) = cache.lock()
        && let Some((created, status)) = guard.get(&key)
        && created.elapsed() < CACHE_TTL
    {
        let mut status = status.clone();
        status.cached = true;
        return status;
    }

    let started = Instant::now();
    let mut status = match profile.game.as_str() {
        "minecraft" => query_minecraft(profile),
        "seven_days" => query_a2s(profile),
        _ => {
            let mut status = ServerStatus::not_checked(profile);
            status.message = "Live status is not implemented for this game yet".into();
            status
        }
    };
    if status.checked {
        status
            .latency_ms
            .get_or_insert(started.elapsed().as_millis());
        status.checked_at_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|duration| duration.as_secs());
        if let Ok(mut guard) = cache.lock() {
            guard.insert(key, (Instant::now(), status.clone()));
        }
    }
    status
}

fn query_minecraft(profile: &GameProfile) -> ServerStatus {
    let result = (|| -> Result<ServerStatus, String> {
        let address = resolve(&profile.server_ip, profile.server_port)?;
        let mut stream = TcpStream::connect_timeout(&address, IO_TIMEOUT)
            .map_err(|error| format!("connection failed: {error}"))?;
        stream.set_read_timeout(Some(IO_TIMEOUT)).ok();
        stream.set_write_timeout(Some(IO_TIMEOUT)).ok();

        let mut handshake = Vec::new();
        write_varint(&mut handshake, 0);
        write_varint(&mut handshake, 760);
        write_varint(&mut handshake, profile.server_ip.len() as i32);
        handshake.extend_from_slice(profile.server_ip.as_bytes());
        handshake.extend_from_slice(&profile.server_port.to_be_bytes());
        write_varint(&mut handshake, 1);
        write_packet(&mut stream, &handshake)?;
        write_packet(&mut stream, &[0])?;

        let packet_length = read_varint(&mut stream)?;
        if packet_length <= 0 || packet_length > 2_000_000 {
            return Err("server returned an invalid status packet length".into());
        }
        if read_varint(&mut stream)? != 0 {
            return Err("server returned an unexpected status packet".into());
        }
        let json_length = read_varint(&mut stream)?;
        if json_length <= 0 || json_length > packet_length {
            return Err("server returned an invalid JSON length".into());
        }
        let mut json = vec![0_u8; json_length as usize];
        stream
            .read_exact(&mut json)
            .map_err(|error| format!("status response ended early: {error}"))?;
        let response: serde_json::Value = serde_json::from_slice(&json)
            .map_err(|error| format!("status response was not valid JSON: {error}"))?;

        let mut ping = vec![1];
        ping.extend_from_slice(&0_i64.to_be_bytes());
        write_packet(&mut stream, &ping).ok();
        let players = response["players"]["online"]
            .as_u64()
            .map(|value| value as u32);
        let max_players = response["players"]["max"]
            .as_u64()
            .map(|value| value as u32);
        let version = response["version"]["name"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let motd = flatten_text(&response["description"]);
        Ok(online_status(
            profile,
            players,
            max_players,
            None,
            version,
            motd,
            String::new(),
        ))
    })();
    result.unwrap_or_else(|error| offline_status(profile, error))
}

fn query_a2s(profile: &GameProfile) -> ServerStatus {
    let result = (|| -> Result<ServerStatus, String> {
        let address = resolve(&profile.server_ip, profile.server_port)?;
        let socket = UdpSocket::bind("0.0.0.0:0")
            .map_err(|error| format!("could not create a UDP query socket: {error}"))?;
        socket.set_read_timeout(Some(IO_TIMEOUT)).ok();
        socket.set_write_timeout(Some(IO_TIMEOUT)).ok();
        socket
            .connect(address)
            .map_err(|error| format!("connection failed: {error}"))?;
        let mut query = b"\xFF\xFF\xFF\xFFTSource Engine Query\x00".to_vec();
        socket
            .send(&query)
            .map_err(|error| format!("A2S query failed: {error}"))?;
        let mut buffer = [0_u8; 65_507];
        let mut count = socket
            .recv(&mut buffer)
            .map_err(|error| format!("A2S response failed: {error}"))?;
        if count >= 9 && buffer[..5] == [0xff, 0xff, 0xff, 0xff, b'A'] {
            query.extend_from_slice(&buffer[5..9]);
            socket
                .send(&query)
                .map_err(|error| format!("A2S challenge reply failed: {error}"))?;
            count = socket
                .recv(&mut buffer)
                .map_err(|error| format!("A2S info response failed: {error}"))?;
        }
        parse_a2s_response(profile, &buffer[..count])
    })();
    result.unwrap_or_else(|error| offline_status(profile, error))
}

fn parse_a2s_response(profile: &GameProfile, data: &[u8]) -> Result<ServerStatus, String> {
    if data.len() < 7 || data[..5] != [0xff, 0xff, 0xff, 0xff, b'I'] {
        return Err("server returned an unsupported A2S response".into());
    }
    let mut cursor = 6; // header, response type, protocol byte
    let name = read_c_string(data, &mut cursor)?;
    let map = read_c_string(data, &mut cursor)?;
    let _folder = read_c_string(data, &mut cursor)?;
    let _game = read_c_string(data, &mut cursor)?;
    if cursor + 9 > data.len() {
        return Err("A2S response ended before player counts".into());
    }
    cursor += 2; // Steam app id
    let players = data[cursor] as u32;
    let max_players = data[cursor + 1] as u32;
    cursor += 7; // players, max, bots, type, environment, visibility, VAC
    let version = read_c_string(data, &mut cursor).unwrap_or_default();
    Ok(online_status(
        profile,
        Some(players),
        Some(max_players),
        None,
        version,
        name,
        map,
    ))
}

fn online_status(
    profile: &GameProfile,
    players: Option<u32>,
    max_players: Option<u32>,
    latency_ms: Option<u128>,
    version: String,
    motd: String,
    map: String,
) -> ServerStatus {
    ServerStatus {
        profile_id: profile.id.clone(),
        configured: true,
        checked: true,
        online: Some(true),
        players,
        max_players,
        latency_ms,
        version,
        motd,
        map,
        message: "Server is online".into(),
        cached: false,
        checked_at_epoch: None,
    }
}

fn offline_status(profile: &GameProfile, error: String) -> ServerStatus {
    ServerStatus {
        profile_id: profile.id.clone(),
        configured: true,
        checked: true,
        online: Some(false),
        players: None,
        max_players: None,
        latency_ms: None,
        version: String::new(),
        motd: String::new(),
        map: String::new(),
        message: error,
        cached: false,
        checked_at_epoch: None,
    }
}

fn resolve(host: &str, port: u16) -> Result<SocketAddr, String> {
    (host, port)
        .to_socket_addrs()
        .map_err(|error| format!("could not resolve server address: {error}"))?
        .next()
        .ok_or_else(|| "server address did not resolve".into())
}

fn write_packet(stream: &mut TcpStream, payload: &[u8]) -> Result<(), String> {
    let mut packet = Vec::new();
    write_varint(&mut packet, payload.len() as i32);
    packet.extend_from_slice(payload);
    stream
        .write_all(&packet)
        .map_err(|error| format!("status request failed: {error}"))
}

fn write_varint(output: &mut Vec<u8>, mut value: i32) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value = ((value as u32) >> 7) as i32;
        if value != 0 {
            byte |= 0x80;
        }
        output.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn read_varint(reader: &mut impl Read) -> Result<i32, String> {
    let mut result = 0_i32;
    for position in 0..5 {
        let mut byte = [0_u8; 1];
        reader
            .read_exact(&mut byte)
            .map_err(|error| format!("status packet ended early: {error}"))?;
        result |= ((byte[0] & 0x7f) as i32) << (position * 7);
        if byte[0] & 0x80 == 0 {
            return Ok(result);
        }
    }
    Err("status packet contained an oversized VarInt".into())
}

fn flatten_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Array(items) => items.iter().map(flatten_text).collect(),
        serde_json::Value::Object(object) => {
            let mut text = object
                .get("text")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string();
            if let Some(extra) = object.get("extra").and_then(|value| value.as_array()) {
                text.extend(extra.iter().map(flatten_text));
            }
            text
        }
        _ => String::new(),
    }
}

fn read_c_string(data: &[u8], cursor: &mut usize) -> Result<String, String> {
    let remaining = data
        .get(*cursor..)
        .ok_or_else(|| "A2S string cursor was out of bounds".to_string())?;
    let end = remaining
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| "A2S response contained an unterminated string".to_string())?;
    let text = String::from_utf8_lossy(&remaining[..end]).into_owned();
    *cursor += end + 1;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{net::TcpListener, thread};

    fn profile(game: &str, port: u16) -> GameProfile {
        let mut profile = crate::models::LauncherConfig::default().profiles.remove(0);
        profile.id = format!("fixture-{port}");
        profile.game = game.into();
        profile.server_ip = "127.0.0.1".into();
        profile.server_port = port;
        profile
    }

    #[test]
    fn parses_minecraft_status_from_a_local_protocol_fixture() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            for _ in 0..2 {
                let length = read_varint(&mut stream).unwrap() as usize;
                let mut request = vec![0_u8; length];
                stream.read_exact(&mut request).unwrap();
            }
            let json = br#"{"version":{"name":"1.21.1"},"players":{"max":20,"online":3},"description":{"text":"Mythic ","extra":[{"text":"Loot"}]}}"#;
            let mut body = vec![0];
            write_varint(&mut body, json.len() as i32);
            body.extend_from_slice(json);
            write_packet(&mut stream, &body).unwrap();
            let length = read_varint(&mut stream).unwrap() as usize;
            let mut ping = vec![0_u8; length];
            stream.read_exact(&mut ping).unwrap();
        });
        let fixture_profile = profile("minecraft", port);
        let status = query(&fixture_profile, true);
        server.join().unwrap();
        assert_eq!(status.online, Some(true), "{}", status.message);
        assert_eq!(status.players, Some(3));
        assert_eq!(status.max_players, Some(20));
        assert_eq!(status.version, "1.21.1");
        assert_eq!(status.motd, "Mythic Loot");
        let cached = query(&fixture_profile, true);
        assert!(cached.cached);
        assert_eq!(cached.online, Some(true));
    }

    #[test]
    fn follows_an_a2s_challenge_and_parses_info() {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = socket.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let mut request = [0_u8; 128];
            let (_, peer) = socket.recv_from(&mut request).unwrap();
            socket
                .send_to(&[0xff, 0xff, 0xff, 0xff, b'A', 1, 2, 3, 4], peer)
                .unwrap();
            let (count, peer) = socket.recv_from(&mut request).unwrap();
            assert_eq!(&request[count - 4..count], &[1, 2, 3, 4]);
            let mut info = vec![0xff, 0xff, 0xff, 0xff, b'I', 17];
            for text in ["Mythic Loot 7DTD", "Navezgane", "7dtd", "7 Days to Die"] {
                info.extend_from_slice(text.as_bytes());
                info.push(0);
            }
            info.extend_from_slice(&[0, 0, 4, 12, 0, b'd', b'w', 0, 1]);
            info.extend_from_slice(b"1.0.0\0");
            socket.send_to(&info, peer).unwrap();
        });
        let status = query_a2s(&profile("seven_days", port));
        server.join().unwrap();
        assert_eq!(status.online, Some(true));
        assert_eq!(status.players, Some(4));
        assert_eq!(status.max_players, Some(12));
        assert_eq!(status.map, "Navezgane");
        assert_eq!(status.version, "1.0.0");
    }

    #[test]
    fn unconfigured_servers_are_not_falsely_reported_offline() {
        let mut profile = profile("minecraft", 25565);
        profile.server_ip.clear();
        let status = query(&profile, false);
        assert!(!status.checked);
        assert_eq!(status.online, None);
    }
}
