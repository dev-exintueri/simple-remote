//! 콘솔 viewer: host ID 와 코드로 연결해 Ping 3번의 RTT 를 보여 준다.
//! 성공하면 `CONNECTED <host 이름> <host key 지문>` 과 `PONG rtt_ms=<n>` 3줄을 출력한다.
//! 코드는 어디에도 출력하지 않는다.

use std::net::{IpAddr, Ipv4Addr};
use std::process::ExitCode;
use std::time::Duration;

use auth::{DeviceKeys, OneTimeCode, key_fingerprint};
use protocol::Timeouts;
use protocol::control::RejectReason;
use protocol::signaling::{ServerToViewer, ViewerToServer};
use transport::{LinkError, WsLink};
use viewer_core::{ConnectError, Side, connect};

const USAGE: &str = "usage: viewer --server <url> --id <9자리 ID> --code <6자리 코드> [--name <이름>]";
const PINGS: usize = 3;
const PING_TIMEOUT: Duration = Duration::from_secs(2);

struct Args {
    server: String,
    id: String,
    code: String,
    name: String,
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Option<Args> {
    let (mut server, mut id, mut code) = (None, None, None);
    let mut name = "viewer".to_string();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--server" => server = Some(args.next()?),
            "--id" => id = Some(args.next()?),
            "--code" => code = Some(args.next()?),
            "--name" => name = args.next()?,
            _ => return None,
        }
    }
    // ID 는 URL 경로에 들어가므로 숫자 9자리만 받는다.
    let id = id.filter(|id| id.len() == 9 && id.bytes().all(|b| b.is_ascii_digit()))?;
    Some(Args { server: server?, id, code: code?, name })
}

fn main() -> ExitCode {
    let Some(args) = parse_args(std::env::args().skip(1)) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let Ok(code) = OneTimeCode::parse(&args.code) else {
        eprintln!("코드는 숫자 6자리여야 합니다.");
        return ExitCode::FAILURE;
    };
    let Ok(keys) = DeviceKeys::generate(&args.name) else {
        eprintln!("이름은 1~64 byte 여야 합니다.");
        return ExitCode::from(2);
    };
    match run(&args, &code, &keys) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{}", describe(&e));
            ExitCode::FAILURE
        }
    }
}

fn run(args: &Args, code: &OneTimeCode, keys: &DeviceKeys) -> Result<(), ConnectError> {
    let link: WsLink<ServerToViewer, ViewerToServer> =
        WsLink::connect(&format!("{}/v1/viewer/{}", args.server, args.id)).map_err(ConnectError::from_join_error)?;
    let mut bind_ips = transport::local_ips();
    if bind_ips.is_empty() {
        bind_ips.push(IpAddr::V4(Ipv4Addr::LOCALHOST));
    }
    let mut session = connect(link, &args.id, code, keys, &bind_ips, &Timeouts::default())?;
    println!("CONNECTED {} {}", session.host.name, key_fingerprint(&session.host.device_key));
    for _ in 0..PINGS {
        let rtt = session.ping(PING_TIMEOUT)?;
        println!("PONG rtt_ms={}", rtt.as_millis());
    }
    Ok(())
}

/// 사용자에게 보일 한 문장.
fn describe(e: &ConnectError) -> String {
    match e {
        ConnectError::WrongCode => "코드가 틀렸습니다.".into(),
        ConnectError::RetryAfter(d) => {
            let secs = d.as_millis().div_ceil(1000).max(1);
            format!("코드를 여러 번 틀려 기다려야 합니다. {secs}초 뒤 다시 시도하세요.")
        }
        ConnectError::HostNotWaiting => "상대가 대기 중이 아닙니다. ID 를 확인하세요.".into(),
        ConnectError::Busy => "상대가 다른 사람의 연결을 처리하고 있습니다. 잠시 뒤 다시 시도하세요.".into(),
        ConnectError::RateLimited => "연결 요청이 너무 많습니다. 몇 분 뒤 다시 시도하세요.".into(),
        ConnectError::HostLeft => "상대가 연결을 끊었습니다.".into(),
        ConnectError::Rejected(RejectReason::Denied) => "상대가 연결을 거절했습니다.".into(),
        ConnectError::Rejected(RejectReason::NoResponse) => "상대가 30초 안에 허락하지 않았습니다.".into(),
        ConnectError::Rejected(RejectReason::BadIdentity) => "상대가 이 기기의 신원을 확인하지 못해 거절했습니다.".into(),
        ConnectError::VersionMismatch { older: Side::Host, .. } => {
            "상대 프로그램이 구버전입니다. 상대에게 업데이트를 부탁하세요.".into()
        }
        ConnectError::VersionMismatch { older: Side::Viewer, .. } => {
            "이 프로그램이 구버전입니다. 업데이트한 뒤 다시 시도하세요.".into()
        }
        // viewer-core 는 이 거절을 위의 `VersionMismatch` 로 바꿔 돌려주므로 보통 오지 않는다.
        ConnectError::Rejected(RejectReason::VersionMismatch { .. }) => "상대와 프로그램 버전이 다릅니다.".into(),
        ConnectError::Timeout(stage) => format!("시간 안에 연결하지 못했습니다 ({stage:?} 단계)."),
        ConnectError::Security(what) => format!("보안 확인에 실패해 연결을 끊었습니다 ({what})."),
        ConnectError::Link(LinkError::InsecureUrl) => "서버 주소는 wss:// 로 시작해야 합니다.".into(),
        ConnectError::Link(e) => format!("signaling 서버와 통신하지 못했습니다: {e}"),
        ConnectError::Transport(e) => format!("P2P 연결을 만들지 못했습니다: {e}"),
    }
}
