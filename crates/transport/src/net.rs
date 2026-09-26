use std::net::{IpAddr, UdpSocket};

/// 밖으로 나가는 UDP 패킷의 local 주소로 쓰일 수 있는 IP 목록.
/// `1.1.1.1:53`, `[2606:4700:4700::1111]:53` 에 각각 UDP socket 을 `connect` 해서
/// (실제로 보내지는 않음) OS 가 고르는 local 주소를 읽는다. 실패한 계열은 뺀다.
/// loopback, unspecified 주소는 포함하지 않는다.
pub fn local_ips() -> Vec<IpAddr> {
    let targets: [&str; 2] = ["1.1.1.1:53", "[2606:4700:4700::1111]:53"];
    let mut out = Vec::new();
    for target in targets {
        if let Some(ip) = local_ip_for(target) {
            if !ip.is_loopback() && !ip.is_unspecified() {
                out.push(ip);
            }
        }
    }
    out
}

fn local_ip_for(target: &str) -> Option<IpAddr> {
    let bind_addr = if target.starts_with('[') { "[::]:0" } else { "0.0.0.0:0" };
    let sock = UdpSocket::bind(bind_addr).ok()?;
    sock.connect(target).ok()?;
    sock.local_addr().ok().map(|a| a.ip())
}
