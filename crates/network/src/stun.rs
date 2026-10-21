use crate::error::{NetworkError, NetworkResult};
use crate::p2p::{IceCandidate, IceCandidateType};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::timeout;

pub const DEFAULT_STUN_SERVER: &str = "stun:stun.l.google.com:19302";
pub const DEFAULT_STUN_TIMEOUT: Duration = Duration::from_millis(1500);

pub const STUN_MAGIC_COOKIE: u32 = 0x2112A442;
pub const BINDING_REQUEST: u16 = 0x0001;
pub const BINDING_RESPONSE_SUCCESS: u16 = 0x0101;
pub const ATTR_MAPPED_ADDRESS: u16 = 0x0001;
pub const ATTR_XOR_MAPPED_ADDRESS: u16 = 0x0020;

#[derive(Debug, Clone)]
pub struct StunConfig {
    pub server_url: String,
    pub timeout: Duration,
}

impl Default for StunConfig {
    fn default() -> Self {
        Self {
            server_url: DEFAULT_STUN_SERVER.into(),
            timeout: DEFAULT_STUN_TIMEOUT,
        }
    }
}

pub struct StunMessage {
    pub msg_type: u16,
    pub length: u16,
    pub magic_cookie: u32,
    pub transaction_id: [u8; 12],
}

impl StunMessage {
    pub fn new_binding_request(transaction_id: [u8; 12]) -> Self {
        Self {
            msg_type: BINDING_REQUEST,
            length: 0,
            magic_cookie: STUN_MAGIC_COOKIE,
            transaction_id,
        }
    }

    pub fn serialize(&self) -> [u8; 20] {
        let mut buf = [0u8; 20];
        buf[0..2].copy_from_slice(&self.msg_type.to_be_bytes());
        buf[2..4].copy_from_slice(&self.length.to_be_bytes());
        buf[4..8].copy_from_slice(&self.magic_cookie.to_be_bytes());
        buf[8..20].copy_from_slice(&self.transaction_id);
        buf
    }

    pub fn parse_header(buf: &[u8]) -> NetworkResult<Self> {
        if buf.len() < 20 {
            return Err(NetworkError::Stun(
                "Packet smaller than 20-byte STUN header".into(),
            ));
        }
        let msg_type = u16::from_be_bytes([buf[0], buf[1]]);
        let length = u16::from_be_bytes([buf[2], buf[3]]);
        let magic_cookie = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let mut transaction_id = [0u8; 12];
        transaction_id.copy_from_slice(&buf[8..20]);

        Ok(Self {
            msg_type,
            length,
            magic_cookie,
            transaction_id,
        })
    }
}

pub fn parse_xor_mapped_address(attr_payload: &[u8]) -> NetworkResult<SocketAddr> {
    if attr_payload.len() < 8 {
        return Err(NetworkError::Stun(
            "XOR-MAPPED-ADDRESS payload too short".into(),
        ));
    }
    let family = attr_payload[1];
    let xor_port = u16::from_be_bytes([attr_payload[2], attr_payload[3]]);
    let port = xor_port ^ ((STUN_MAGIC_COOKIE >> 16) as u16);

    if family == 0x01 {
        // IPv4
        let xor_ip = u32::from_be_bytes([
            attr_payload[4],
            attr_payload[5],
            attr_payload[6],
            attr_payload[7],
        ]);
        let ip_u32 = xor_ip ^ STUN_MAGIC_COOKIE;
        let ip = Ipv4Addr::from(ip_u32);
        Ok(SocketAddr::new(IpAddr::V4(ip), port))
    } else {
        Err(NetworkError::Stun(format!(
            "Unsupported address family: {:#x}",
            family
        )))
    }
}

pub fn parse_mapped_address(attr_payload: &[u8]) -> NetworkResult<SocketAddr> {
    if attr_payload.len() < 8 {
        return Err(NetworkError::Stun(
            "MAPPED-ADDRESS payload too short".into(),
        ));
    }
    let family = attr_payload[1];
    let port = u16::from_be_bytes([attr_payload[2], attr_payload[3]]);

    if family == 0x01 {
        // IPv4
        let ip_u32 = u32::from_be_bytes([
            attr_payload[4],
            attr_payload[5],
            attr_payload[6],
            attr_payload[7],
        ]);
        let ip = Ipv4Addr::from(ip_u32);
        Ok(SocketAddr::new(IpAddr::V4(ip), port))
    } else {
        Err(NetworkError::Stun(format!(
            "Unsupported address family: {:#x}",
            family
        )))
    }
}

pub fn compute_ice_candidate_priority(candidate_type: &IceCandidateType) -> u32 {
    match candidate_type {
        IceCandidateType::Host => 2130706431,
        IceCandidateType::ServerReflexive => 1694498815,
        IceCandidateType::Relay => 16777215,
    }
}

pub async fn resolve_server_reflexive_candidate(
    socket: &UdpSocket,
    config: &StunConfig,
) -> NetworkResult<IceCandidate> {
    let server_addr_str = config
        .server_url
        .strip_prefix("stun:")
        .unwrap_or(&config.server_url);
    let mut addrs = tokio::net::lookup_host(server_addr_str)
        .await
        .map_err(|e| {
            NetworkError::Stun(format!("DNS lookup failed for {}: {}", server_addr_str, e))
        })?;
    let stun_addr = addrs
        .next()
        .ok_or_else(|| NetworkError::Stun(format!("No IP address found for {}", server_addr_str)))?;

    let mut tx_id = [0u8; 12];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut tx_id);

    let req = StunMessage::new_binding_request(tx_id);
    let req_bytes = req.serialize();

    socket.send_to(&req_bytes, stun_addr).await?;

    let mut resp_buf = [0u8; 512];
    let (len, _src) = timeout(config.timeout, socket.recv_from(&mut resp_buf))
        .await
        .map_err(|_| NetworkError::StunTimeout(config.timeout))??;

    let header = StunMessage::parse_header(&resp_buf[..len])?;
    if header.msg_type != BINDING_RESPONSE_SUCCESS {
        return Err(NetworkError::Stun(format!(
            "Unexpected STUN response type: {:#06x}",
            header.msg_type
        )));
    }
    if header.transaction_id != tx_id {
        return Err(NetworkError::Stun("STUN transaction ID mismatch".into()));
    }

    // Parse attributes
    let mut offset = 20;
    while offset + 4 <= len {
        let attr_type = u16::from_be_bytes([resp_buf[offset], resp_buf[offset + 1]]);
        let attr_len = u16::from_be_bytes([resp_buf[offset + 2], resp_buf[offset + 3]]) as usize;
        offset += 4;
        if offset + attr_len > len {
            break;
        }

        if attr_type == ATTR_XOR_MAPPED_ADDRESS {
            let addr = parse_xor_mapped_address(&resp_buf[offset..offset + attr_len])?;
            return Ok(IceCandidate {
                candidate_type: IceCandidateType::ServerReflexive,
                ip: addr.ip().to_string(),
                port: addr.port(),
                protocol: "udp".into(),
                priority: compute_ice_candidate_priority(&IceCandidateType::ServerReflexive),
            });
        } else if attr_type == ATTR_MAPPED_ADDRESS {
            let addr = parse_mapped_address(&resp_buf[offset..offset + attr_len])?;
            return Ok(IceCandidate {
                candidate_type: IceCandidateType::ServerReflexive,
                ip: addr.ip().to_string(),
                port: addr.port(),
                protocol: "udp".into(),
                priority: compute_ice_candidate_priority(&IceCandidateType::ServerReflexive),
            });
        }
        offset += (attr_len + 3) & !3; // Align to 4 bytes boundary
    }

    Err(NetworkError::Stun(
        "XOR-MAPPED-ADDRESS attribute not found in STUN response".into(),
    ))
}
