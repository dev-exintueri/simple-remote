//! 콘솔 host: signaling 에 등록해 ID 와 코드를 보여 주고, 들어오는 viewer 를 하나씩 처리한다.
//! 표준 출력은 한 줄씩 `ID <id>`, `CODE <code>`, `APPROVE? ...`, `ATTEMPT <실패>` 등이다.

use std::io::{BufRead, BufReader, Write};
use std::net::{IpAddr, Ipv4Addr};
use std::process::ExitCode;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};

use auth::{DeviceKeys, key_fingerprint};
use host_agent::{ApprovalRequest, HostConfig, HostState, ServeOutcome, register_host, serve_next, set_mode};
use protocol::signaling::HostMode;
use protocol::{PROTOCOL_VERSION, Timeouts};

const USAGE: &str = "usage: host-agent --server <url> [--name <이름>] [--once]";
/// 허락한 세션이 Ping 에 답하는 최대 시간.
const SESSION_LIMIT: Duration = Duration::from_secs(60 * 60);

struct Args {
    server: String,
    name: String,
    once: bool,
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Option<Args> {
    let mut server = None;
    let mut name = "host".to_string();
    let mut once = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--server" => server = Some(args.next()?),
            "--name" => name = args.next()?,
            "--once" => once = true,
            _ => return None,
        }
    }
    Some(Args { server: server?, name, once })
}

/// 한 줄을 출력하고 바로 내보낸다. 표준 출력이 pipe 여도 읽는 쪽이 곧바로 받게 한다.
fn say(line: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

fn main() -> ExitCode {
    let Some(args) = parse_args(std::env::args().skip(1)) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let Ok(keys) = DeviceKeys::generate(&args.name) else {
        eprintln!("이름은 1~64 byte 여야 합니다.");
        return ExitCode::from(2);
    };
    let (host_id, mut link) = match register_host(&args.server, &keys, None) {
        Ok(registered) => registered,
        Err(e) => {
            eprintln!("signaling 서버에 등록하지 못했습니다: {e}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = set_mode(&mut link, HostMode::New) {
        eprintln!("signaling 서버에 모드를 알리지 못했습니다: {e}");
        return ExitCode::FAILURE;
    }
    let mut bind_ips = transport::local_ips();
    if bind_ips.is_empty() {
        bind_ips.push(IpAddr::V4(Ipv4Addr::LOCALHOST));
    }
    let cfg = HostConfig {
        host_id,
        keys,
        bind_ips,
        timeouts: Timeouts::default(),
        protocol_version: PROTOCOL_VERSION,
    };
    let mut state = HostState::new();
    let mut shown_code = state.code.clone();
    say(&format!("ID {}", cfg.host_id));
    say(&format!("CODE {}", shown_code.as_str()));

    let mut approver = Approver::spawn(BufReader::new(std::io::stdin()));
    loop {
        let outcome = serve_next(&mut link, &cfg, &mut state, &mut |req| approver.answer(req));
        approver.end_question();
        // 실패가 쌓여 코드가 바뀌었거나, 허락해서 코드를 새로 만든 경우.
        if state.code != shown_code {
            shown_code = state.code.clone();
            say(&format!("CODE {}", shown_code.as_str()));
        }
        match outcome {
            Ok(ServeOutcome::Session(mut session)) => {
                say(&format!(
                    "CONNECTED {} {}",
                    session.viewer.name,
                    key_fingerprint(&session.viewer.device_key)
                ));
                let served = session.serve_pings(SESSION_LIMIT);
                say("DISCONNECTED");
                if let Err(e) = &served {
                    eprintln!("세션 연결이 끊겼습니다: {e}");
                }
                if args.once {
                    return if served.is_ok() { ExitCode::SUCCESS } else { ExitCode::FAILURE };
                }
            }
            Ok(ServeOutcome::Failed(failure)) => say(&format!("ATTEMPT {failure:?}")),
            Ok(ServeOutcome::ViewerLeft) => {}
            Err(e) => {
                eprintln!("signaling 서버와 연결이 끊겼습니다: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
}

/// 허락 질문에 대한 답을 표준 입력에서 읽는다. 읽기는 막히므로 별도 thread 가 요청받을 때마다
/// 한 줄씩 읽고, [`Approver::answer`] 는 그 결과를 기다리지 않고 확인만 한다.
struct Approver {
    ask: Sender<()>,
    answers: Receiver<(Instant, Option<String>)>,
    /// thread 에 한 줄을 요청했고 아직 답을 받지 못함.
    reading: bool,
    /// 지금 묻고 있는 질문을 처음 출력한 시각.
    asked_at: Option<Instant>,
}

impl Approver {
    fn spawn(mut input: impl BufRead + Send + 'static) -> Approver {
        let (ask, asks) = mpsc::channel::<()>();
        let (answer, answers) = mpsc::channel();
        std::thread::spawn(move || {
            for () in asks {
                let mut line = String::new();
                // EOF 와 읽기 오류는 `None` (거절).
                let got = match input.read_line(&mut line) {
                    Ok(0) | Err(_) => None,
                    Ok(_) => Some(line),
                };
                if answer.send((Instant::now(), got)).is_err() {
                    return;
                }
            }
        });
        Approver { ask, answers, reading: false, asked_at: None }
    }

    /// `serve_next` 에 넘기는 막히지 않는 질문 함수. `y` 한 줄만 허락이다.
    fn answer(&mut self, req: &ApprovalRequest) -> Option<bool> {
        let asked_at = *self.asked_at.get_or_insert_with(|| {
            say(&format!("APPROVE? {} {} [y/N]", req.viewer_name, req.viewer_key_fingerprint));
            Instant::now()
        });
        loop {
            if !self.reading {
                if self.ask.send(()).is_err() {
                    return Some(false);
                }
                self.reading = true;
            }
            match self.answers.try_recv() {
                Ok((at, line)) => {
                    self.reading = false;
                    // 앞 질문이 끝난 뒤, 이 질문을 보여 주기 전에 들어온 줄은 이 질문의 답이 아니다.
                    if at < asked_at {
                        continue;
                    }
                    return Some(line.is_some_and(|l| l.trim() == "y"));
                }
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => return Some(false),
            }
        }
    }

    /// `serve_next` 가 돌아오면 부른다. 답하지 못한 질문의 늦은 답은 다음 질문에 쓰지 않는다.
    fn end_question(&mut self) {
        self.asked_at = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::PipeWriter;

    fn req() -> ApprovalRequest {
        ApprovalRequest { viewer_name: "v".into(), viewer_key_fingerprint: "0000 0000 0000 0000".into() }
    }

    fn approver() -> (Approver, PipeWriter) {
        let (reader, writer) = std::io::pipe().unwrap();
        (Approver::spawn(BufReader::new(reader)), writer)
    }

    /// 답이 나올 때까지 `answer` 를 부른다. 최대 5초.
    fn wait_answer(a: &mut Approver) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(ans) = a.answer(&req()) {
                return ans;
            }
            assert!(Instant::now() < deadline, "no answer");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn only_y_approves() {
        let (mut a, mut w) = approver();
        assert_eq!(a.answer(&req()), None);
        writeln!(w, " y ").unwrap();
        assert!(wait_answer(&mut a));
        a.end_question();
        writeln!(w, "yes").unwrap();
        assert!(!wait_answer(&mut a));
    }

    #[test]
    fn eof_denies() {
        let (mut a, w) = approver();
        assert_eq!(a.answer(&req()), None);
        drop(w);
        assert!(!wait_answer(&mut a));
    }

    #[test]
    fn late_answer_to_a_finished_question_is_not_reused() {
        let (mut a, mut w) = approver();
        assert_eq!(a.answer(&req()), None);
        a.end_question();
        // 앞 질문이 끝난 뒤에 들어온 `y` 가 다음 viewer 를 허락하면 안 된다.
        writeln!(w, "y").unwrap();
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(a.answer(&req()), None);
        writeln!(w, "n").unwrap();
        assert!(!wait_answer(&mut a));
    }
}
