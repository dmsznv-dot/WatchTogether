use network::{
    compute_ice_candidate_priority, parse_xor_mapped_address, IceCandidateType, StunConfig,
    StunMessage, BINDING_REQUEST, STUN_MAGIC_COOKIE,
};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

#[test]
fn test_stun_default_config() {
    let config = StunConfig::default();
    assert_eq!(config.server_url, "stun:stun.l.google.com:19302");
    assert_eq!(config.timeout, std::time::Duration::from_millis(1500));
}

#[test]
fn test_stun_binding_request_serialization_and_parsing() {
    let tx_id = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
    let req = StunMessage::new_binding_request(tx_id);
    let bytes = req.serialize();

    assert_eq!(bytes.len(), 20);
    let parsed = StunMessage::parse_header(&bytes).expect("Parse header");
    assert_eq!(parsed.msg_type, BINDING_REQUEST);
    assert_eq!(parsed.length, 0);
    assert_eq!(parsed.magic_cookie, STUN_MAGIC_COOKIE);
    assert_eq!(parsed.transaction_id, tx_id);
}

#[test]
fn test_parse_xor_mapped_address() {
    // Port 54321 = 0xD431
    // Magic cookie high 16 bits = 0x2112
    // XOR port = 0xD431 ^ 0x2112 = 0xF523
    let xor_port: u16 = 54321 ^ ((STUN_MAGIC_COOKIE >> 16) as u16);

    // IP 203.0.113.45 = 0xCB00712D
    // Magic cookie = 0x2112A442
    // XOR IP = 0xCB00712D ^ 0x2112A442 = 0xEA12D56F
    let ip_u32 = u32::from(Ipv4Addr::new(203, 0, 113, 45));
    let xor_ip = ip_u32 ^ STUN_MAGIC_COOKIE;

    let mut payload = Vec::new();
    payload.push(0x00); // reserved
    payload.push(0x01); // IPv4 family
    payload.extend_from_slice(&xor_port.to_be_bytes());
    payload.extend_from_slice(&xor_ip.to_be_bytes());

    let addr = parse_xor_mapped_address(&payload).expect("Decode XOR mapped address");
    assert_eq!(addr, SocketAddr::new(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 45)), 54321));
}

#[test]
fn test_ice_candidate_priority_ordering() {
    let host_prio = compute_ice_candidate_priority(&IceCandidateType::Host);
    let srflx_prio = compute_ice_candidate_priority(&IceCandidateType::ServerReflexive);
    let relay_prio = compute_ice_candidate_priority(&IceCandidateType::Relay);

    assert_eq!(host_prio, 2130706431);
    assert_eq!(srflx_prio, 1694498815);
    assert_eq!(relay_prio, 16777215);

    assert!(host_prio > srflx_prio);
    assert!(srflx_prio > relay_prio);
}
