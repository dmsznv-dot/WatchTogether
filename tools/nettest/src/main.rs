//! Watch Together — проверка соединения без настройки роутера.
//!
//! Ведущий: `wt-nettest host`  → печатает код приглашения и ждёт.
//! Зритель: `wt-nettest join <код>` → подключается, 60 секунд меряет задержку
//! и показывает, идёт ли связь напрямую или через relay.
//! Без аргументов программа спрашивает режим сама (удобно для двойного клика).

use std::{
    io::{BufRead, Write},
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Result, anyhow, bail};
use iroh::{
    Endpoint, EndpointAddr, EndpointId, RelayUrl, TransportAddr, Watcher,
    endpoint::{Connection, presets},
};

const ALPN: &[u8] = b"watchtogether/nettest/1";
const TICKET_PREFIX: &str = "WT1-";
const TEST_SECS: u64 = 60;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const ONLINE_TIMEOUT: Duration = Duration::from_secs(15);

// ---------- вывод в консоль + в файл ----------

static LOG: OnceLock<Mutex<std::fs::File>> = OnceLock::new();
static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

macro_rules! say {
    ($($t:tt)*) => {{
        let line = format!($($t)*);
        println!("{line}");
        if let Some(file) = LOG.get() {
            if let Ok(mut f) = file.lock() {
                let _ = writeln!(f, "{line}");
            }
        }
    }};
}

fn init_log() {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = home.join(format!("wt-nettest-{ts}.txt"));
    if let Ok(file) = std::fs::File::create(&path) {
        let _ = LOG.set(Mutex::new(file));
        let _ = LOG_PATH.set(path);
    }
}

fn log_path() -> String {
    LOG_PATH
        .get()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(не удалось создать файл)".into())
}

fn err<E: std::fmt::Display>(e: E) -> anyhow::Error {
    anyhow!("{e:#}")
}

fn read_line(prompt: &str) -> String {
    print!("{prompt}");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    let _ = std::io::stdin().lock().read_line(&mut line);
    line.trim().to_string()
}

// ---------- точка входа ----------

enum Mode {
    Host,
    Join(EndpointAddr),
}

#[tokio::main]
async fn main() {
    init_log();
    say!("Watch Together — проверка соединения (v{})", env!("CARGO_PKG_VERSION"));
    say!("ОС: {} {}", std::env::consts::OS, std::env::consts::ARCH);
    say!("");

    let code = match run().await {
        Ok(()) => 0,
        Err(e) => {
            say!("");
            say!("[ОШИБКА] {e:#}");
            1
        }
    };
    say!("");
    say!("Лог сохранён: {}", log_path());
    read_line("Нажмите Enter, чтобы закрыть окно…");
    std::process::exit(code);
}

async fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Код проверяем до диагностики, чтобы ошибка ввода была видна сразу.
    let ask_ticket = || read_line("Вставьте код приглашения и нажмите Enter:\n> ");
    let mode = match args.first().map(String::as_str) {
        Some("host") => Mode::Host,
        Some("join") => match args.get(1) {
            Some(t) => Mode::Join(parse_ticket(t)?),
            None => Mode::Join(parse_ticket(&ask_ticket())?),
        },
        _ => {
            say!("Выберите режим:");
            say!("  1 — я создаю комнату (ведущий)");
            say!("  2 — я подключаюсь по коду (зритель)");
            match read_line("> ").as_str() {
                "1" => Mode::Host,
                "2" => Mode::Join(parse_ticket(&ask_ticket())?),
                other => bail!("неизвестный выбор: {other:?} (нужно 1 или 2)"),
            }
        }
    };

    let mut builder = Endpoint::builder(presets::N0);
    if matches!(mode, Mode::Host) {
        builder = builder.alpns(vec![ALPN.to_vec()]);
    }
    let ep = builder.bind().await.map_err(err)?;

    diagnostics(&ep).await;

    match mode {
        Mode::Host => host(ep).await,
        Mode::Join(addr) => join(ep, addr).await,
    }
}

// ---------- диагностика сети ----------

async fn diagnostics(ep: &Endpoint) {
    say!("== Диагностика сети ==");
    say!("ID этого компьютера: {}", ep.id().fmt_short());
    for s in ep.bound_sockets() {
        say!("Локальный сокет: {s}");
    }

    let started = Instant::now();
    match tokio::time::timeout(ONLINE_TIMEOUT, ep.online()).await {
        Ok(()) => {
            let relay = ep
                .addr()
                .relay_urls()
                .next()
                .map(|u| u.to_string())
                .unwrap_or_else(|| "?".into());
            say!(
                "[OK] Relay доступен за {} мс: {relay}",
                started.elapsed().as_millis()
            );
        }
        Err(_) => {
            say!("[!!] Relay НЕ подключился за {} с.", ONLINE_TIMEOUT.as_secs());
            for st in ep.home_relay_status().get() {
                let reason = st
                    .last_error()
                    .map(|e| format!("{e:#}"))
                    .unwrap_or_else(|| "нет ответа".into());
                say!("     {} — {reason}", st.url());
            }
            say!("     Без relay возможно только прямое соединение (IPv6 / UPnP).");
        }
    }

    // Даём сетевому отчёту время собраться.
    let mut report = None;
    for _ in 0..16 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if let Some(r) = ep.net_report().get() {
            let complete = r.global_v4.is_some() || r.global_v6.is_some() || r.preferred_relay.is_some();
            report = Some(r);
            if complete {
                // ещё секунда, чтобы успели прийти остальные пробы
                tokio::time::sleep(Duration::from_secs(1)).await;
                report = ep.net_report().get().or(report);
                break;
            }
        }
    }

    match report {
        None => say!("[!!] Сетевой отчёт не получен."),
        Some(r) => {
            say!("UDP IPv4: {}   UDP IPv6: {}", yes(r.udp_v4), yes(r.udp_v6));
            say!(
                "Внешний IPv4: {}",
                r.global_v4.map(|a| a.to_string()).unwrap_or_else(|| "—".into())
            );
            say!(
                "Внешний IPv6: {}",
                r.global_v6.map(|a| a.to_string()).unwrap_or_else(|| "—".into())
            );
            match r.mapping_varies_by_dest() {
                Some(true) => say!(
                    "Тип NAT: ЖЁСТКИЙ (адрес меняется для разных серверов) — прямое соединение маловероятно, ожидаем relay"
                ),
                Some(false) => say!("Тип NAT: обычный — пробивка NAT должна работать"),
                None => say!("Тип NAT: не определён"),
            }
            if let Some(p) = &r.preferred_relay {
                say!("Ближайший relay: {p}");
            }
            say!("Задержки до relay: {:?}", r.relay_latency);
            if !r.has_udp() {
                say!("[!!] UDP не проходит: прямое соединение невозможно, только relay.");
            }
            if r.global_v6.is_some() {
                say!("[OK] Есть публичный IPv6 — хороший шанс прямого соединения.");
            }
        }
    }
    say!("");
}

fn yes(b: bool) -> &'static str {
    if b { "да" } else { "нет" }
}

// ---------- код приглашения ----------

fn make_ticket(addr: &EndpointAddr) -> String {
    let relays: Vec<String> = addr.relay_urls().map(|u| u.to_string()).collect();
    let ips: Vec<String> = addr.ip_addrs().take(8).map(|a| a.to_string()).collect();
    let json = serde_json::json!({ "i": addr.id.to_string(), "r": relays, "a": ips });
    format!(
        "{TICKET_PREFIX}{}",
        data_encoding::BASE64URL_NOPAD.encode(json.to_string().as_bytes())
    )
}

fn parse_ticket(raw: &str) -> Result<EndpointAddr> {
    let cleaned: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.is_empty() {
        bail!("код приглашения не введён — скопируйте у ведущего строку, начинающуюся с {TICKET_PREFIX}");
    }
    let body = cleaned
        .strip_prefix(TICKET_PREFIX)
        .ok_or_else(|| anyhow!("код должен начинаться с {TICKET_PREFIX}"))?;
    let bytes = data_encoding::BASE64URL_NOPAD
        .decode(body.as_bytes())
        .map_err(|e| anyhow!("код повреждён (обрезан при копировании?): {e}"))?;
    let v: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow!("код повреждён (обрезан при копировании?) — скопируйте его целиком"))?;
    let id: EndpointId = v["i"]
        .as_str()
        .ok_or_else(|| anyhow!("в коде нет ID ведущего"))?
        .parse()
        .map_err(|e| anyhow!("в коде неверный ID ведущего: {e}"))?;
    let mut addr = EndpointAddr::new(id);
    for r in v["r"].as_array().into_iter().flatten() {
        if let Some(Ok(url)) = r.as_str().map(|s| s.parse::<RelayUrl>()) {
            addr = addr.with_relay_url(url);
        }
    }
    for a in v["a"].as_array().into_iter().flatten() {
        if let Some(Ok(sa)) = a.as_str().map(|s| s.parse::<SocketAddr>()) {
            addr = addr.with_ip_addr(sa);
        }
    }
    Ok(addr)
}

// ---------- ведущий ----------

async fn host(ep: Endpoint) -> Result<()> {
    let ticket = make_ticket(&ep.addr());
    say!("== Комната открыта ==");
    say!("Отправьте другу этот код целиком (одной строкой):");
    say!("");
    say!("{ticket}");
    say!("");
    say!("Ждём подключения. Окно не закрывайте, пока друг не закончит тест.");
    say!("Лог пишется в: {}", log_path());
    say!("");

    let accept_ep = ep.clone();
    tokio::spawn(async move {
        while let Some(incoming) = accept_ep.accept().await {
            tokio::spawn(async move {
                let conn = match incoming.await {
                    Ok(c) => c,
                    Err(e) => {
                        say!("[!!] Входящее соединение сорвалось: {e:#}");
                        return;
                    }
                };
                let who = conn.remote_id().fmt_short().to_string();
                say!("[OK] Подключился зритель {who}: {}", path_info(&conn).1);
                let watcher = {
                    let conn = conn.clone();
                    let who = who.clone();
                    tokio::spawn(async move {
                        let mut last = String::new();
                        loop {
                            tokio::time::sleep(Duration::from_secs(2)).await;
                            let (_, desc) = path_info(&conn);
                            if desc != last {
                                say!("     [{who}] путь: {desc}");
                                last = desc;
                            }
                        }
                    })
                };
                if let Err(e) = echo(&conn).await {
                    say!("     [{who}] соединение закрыто: {e:#}");
                } else {
                    say!("     [{who}] тест завершён");
                }
                watcher.abort();
            });
        }
    });

    tokio::task::spawn_blocking(|| read_line("(Enter — закрыть комнату)\n")).await?;
    ep.close().await;
    Ok(())
}

async fn echo(conn: &Connection) -> Result<()> {
    let (mut send, mut recv) = conn.accept_bi().await.map_err(err)?;
    let mut buf = [0u8; 8];
    loop {
        match recv.read_exact(&mut buf).await {
            Ok(()) => send.write_all(&buf).await.map_err(err)?,
            Err(_) => return Ok(()),
        }
    }
}

// ---------- зритель ----------

#[derive(Clone, Copy, PartialEq, Eq)]
enum PathKind {
    Direct,
    Relay,
    Unknown,
}

fn path_info(conn: &Connection) -> (PathKind, String) {
    let paths = conn.paths();
    let selected = paths.iter().find(|p| p.is_selected());
    match selected {
        Some(p) if p.is_relay() => (
            PathKind::Relay,
            format!("ЧЕРЕЗ RELAY {} (rtt {} мс)", p.remote_addr(), p.rtt().as_millis()),
        ),
        Some(p) => {
            let place = match p.remote_addr() {
                TransportAddr::Ip(sa) if is_lan(sa.ip()) => "локальная сеть",
                TransportAddr::Ip(sa) if sa.is_ipv6() => "интернет, IPv6",
                _ => "интернет, IPv4",
            };
            (
                PathKind::Direct,
                format!(
                    "ПРЯМОЕ, {place}: {} (rtt {} мс)",
                    p.remote_addr(),
                    p.rtt().as_millis()
                ),
            )
        }
        None => (PathKind::Unknown, "путь ещё не выбран".into()),
    }
}

fn is_lan(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_loopback() || v4.is_link_local(),
        IpAddr::V6(v6) => {
            v6.is_loopback() || (v6.segments()[0] & 0xfe00) == 0xfc00 || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

async fn join(ep: Endpoint, addr: EndpointAddr) -> Result<()> {
    say!("== Подключение к {} ==", addr.id.fmt_short());
    for r in addr.relay_urls() {
        say!("relay ведущего: {r}");
    }
    for a in addr.ip_addrs() {
        say!("адрес ведущего: {a}");
    }

    let t0 = Instant::now();
    let conn = tokio::time::timeout(CONNECT_TIMEOUT, ep.connect(addr, ALPN))
        .await
        .map_err(|_| anyhow!("не удалось подключиться за {} с", CONNECT_TIMEOUT.as_secs()))?
        .map_err(err)?;
    say!(
        "[OK] Соединение установлено за {} мс: {}",
        t0.elapsed().as_millis(),
        path_info(&conn).1
    );
    say!("Тест {TEST_SECS} с: каждую секунду отправляем пинг…");
    say!("");

    let (mut send, mut recv) = conn.open_bi().await.map_err(err)?;
    let mut rtts: Vec<u128> = Vec::new();
    let (mut direct, mut relay, mut lost) = (0u32, 0u32, 0u32);
    let mut first_direct: Option<u64> = None;

    for seq in 1..=TEST_SECS {
        let tick = Instant::now();
        send.write_all(&seq.to_be_bytes()).await.map_err(err)?;
        let mut buf = [0u8; 8];
        let rtt = match tokio::time::timeout(Duration::from_secs(3), recv.read_exact(&mut buf)).await {
            Ok(Ok(())) if u64::from_be_bytes(buf) == seq => Some(tick.elapsed().as_millis()),
            Ok(Ok(())) => None,
            Ok(Err(e)) => bail!("ведущий закрыл соединение: {e:#}"),
            Err(_) => None,
        };
        let (kind, desc) = path_info(&conn);
        match kind {
            PathKind::Direct => {
                direct += 1;
                first_direct.get_or_insert(seq);
            }
            PathKind::Relay => relay += 1,
            PathKind::Unknown => {}
        }
        match rtt {
            Some(ms) => {
                rtts.push(ms);
                say!("[{seq:>2} с] пинг {ms:>4} мс · {desc}");
            }
            None => {
                lost += 1;
                say!("[{seq:>2} с] пинг ПОТЕРЯН · {desc}");
            }
        }
        if let Some(rest) = Duration::from_secs(1).checked_sub(tick.elapsed()) {
            tokio::time::sleep(rest).await;
        }
    }
    let _ = send.finish();
    conn.close(0u32.into(), b"done");

    // ---------- итог ----------
    say!("");
    say!("== Итог ==");
    let total = TEST_SECS as u32;
    say!("Прямое соединение: {direct}/{total} с   Через relay: {relay}/{total} с   Потери: {lost}");
    if let Some(s) = first_direct {
        say!("Прямой путь появился на {s}-й секунде.");
    }
    if !rtts.is_empty() {
        let mut sorted = rtts.clone();
        sorted.sort_unstable();
        say!(
            "Пинг: мин {} мс · медиана {} мс · макс {} мс",
            sorted[0],
            sorted[sorted.len() / 2],
            sorted[sorted.len() - 1]
        );
    }
    let verdict = if lost > total / 4 {
        "[!!] Соединение нестабильное — для синхронизации нужно разбираться."
    } else if direct > total / 2 {
        "[OK] Работает НАПРЯМУЮ без настройки роутера. Схема для Watch Together подходит."
    } else if relay > 0 {
        "[OK] Работает через relay. Для синхронизации этого достаточно, но лучше свой relay."
    } else {
        "[??] Связь есть, но тип пути не определён — пришлите лог."
    };
    say!("{verdict}");
    ep.close().await;
    Ok(())
}
