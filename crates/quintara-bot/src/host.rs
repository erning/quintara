//! host 端：拉起并驱动外部 `pbrain-<name>` bot（子进程 + 管道 + Gomocup 协议）。
//!
//! `spawn(cmd)` 返回 [`ExternalBot`] 句柄。管道异步性用一个 reader 线程 + `mpsc` 吸收，对
//! 上暴露同步带超时接口。`arbiter`（P1d）会把它包成统一 `Player` 端口。协议字节不出本模块。

use std::io::{self, BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command as OsCommand, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use quintara_model::{PlayerLostKind, Position};
use quintara_protocol::{command, reply, Command, Reply};

/// 非阻塞读回复的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplyPoll {
    /// 收到一条可解析的回复。
    Reply(Reply),
    /// 当前无数据。
    Empty,
    /// 子进程已退出（管道关闭）。
    Closed,
}

/// 一个正在运行的外部 bot 子进程。
pub struct ExternalBot {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    reader: Option<JoinHandle<()>>,
}

/// 拉起一个外部 bot。`command_line` 首 token 为可执行文件，其余为参数（空白分隔）。
///
/// # Errors
/// 命令行为空或进程启动失败时返回 `io::Error`。
pub fn spawn(command_line: &str) -> io::Result<ExternalBot> {
    let mut parts = command_line.split_whitespace();
    let program = parts
        .next()
        .ok_or_else(|| io::Error::other("empty command line"))?;
    let args: Vec<&str> = parts.collect();

    let mut child = OsCommand::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("no child stdin"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("no child stdout"))?;

    let (tx, rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(line) => {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        // EOF / 读错误：丢弃 tx → 接收端 Disconnected
    });

    Ok(ExternalBot {
        child,
        stdin,
        lines: rx,
        reader: Some(reader),
    })
}

impl ExternalBot {
    /// 发一条命令（多行命令整块写出，行尾补换行）。
    ///
    /// # Errors
    /// 写子进程 stdin 失败时返回 `io::Error`。
    pub fn send(&mut self, command: &Command) -> io::Result<()> {
        let text = command::encode(command);
        writeln!(self.stdin, "{text}")?;
        self.stdin.flush()
    }

    /// 阻塞读下一条回复或超时。
    ///
    /// # Errors
    /// 超时 / 子进程退出返回 [`RecvTimeoutError`]；该行无法解析为回复时返回
    /// [`quintara_protocol::ParseError`] 包装的 `RecvTimeoutError::Disconnected` 之外——
    /// 这里简单丢弃无法解析的行，故只返回 `RecvTimeoutError`。
    pub fn recv_reply(&self, timeout: Duration) -> Result<Reply, RecvTimeoutError> {
        loop {
            let line = self.lines.recv_timeout(timeout)?;
            if let Ok(reply) = reply::decode(&line) {
                return Ok(reply);
            }
            // 无法解析的行：忽略，继续等。
        }
    }

    /// 发一个会触发落子的命令（`TURN`/`BEGIN`/`BOARD`），在 `timeout` 内取回落子坐标。
    /// 期间忽略 `MESSAGE`/`DEBUG` 等噪声；`SUGGEST` 视作落子。
    ///
    /// # Errors
    /// 超时 / 失联 / bot 报错时返回对应 [`PlayerLostKind`]。
    pub fn request_move(
        &mut self,
        command: &Command,
        timeout: Duration,
    ) -> Result<Position, PlayerLostKind> {
        self.send(command).map_err(|_| PlayerLostKind::Disconnect)?;
        let end = Instant::now() + timeout;
        loop {
            let remaining = end.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(remaining) {
                Ok(line) => match reply::decode(&line) {
                    Ok(Reply::Coord(pos) | Reply::Suggest(pos)) => return Ok(pos),
                    Ok(Reply::Error(_)) => return Err(PlayerLostKind::Malformed),
                    _ => {} // 噪声 / 不相关 / 不可解析：继续等
                },
                Err(RecvTimeoutError::Timeout) => return Err(PlayerLostKind::Timeout),
                Err(RecvTimeoutError::Disconnected) => return Err(PlayerLostKind::Crash),
            }
        }
    }

    /// 非阻塞取下一条**可解析**的回复（跳过无法解析的噪声行）。供 poll 式驱动用。
    #[must_use]
    pub fn try_recv_reply(&self) -> ReplyPoll {
        loop {
            match self.lines.try_recv() {
                Ok(line) => {
                    if let Ok(reply) = reply::decode(&line) {
                        return ReplyPoll::Reply(reply);
                    }
                    // 无法解析的行：丢弃，继续取下一行。
                }
                Err(TryRecvError::Empty) => return ReplyPoll::Empty,
                Err(TryRecvError::Disconnected) => return ReplyPoll::Closed,
            }
        }
    }

    /// 发 `ABOUT` 并取回 bot 自报信息行。
    ///
    /// # Errors
    /// 写失败或超时无应答时返回 `None`。
    #[must_use]
    pub fn about(&mut self, timeout: Duration) -> Option<String> {
        self.send(&Command::About).ok()?;
        // 引擎在 ABOUT 行前可能先吐若干 MESSAGE / DEBUG 噪声（如 Rapfi 加载权重）；跳过它们，
        // 在总时限内一直等到真正的 ABOUT 应答。
        let end = Instant::now() + timeout;
        loop {
            let remaining = end.checked_duration_since(Instant::now())?;
            match self.recv_reply(remaining) {
                Ok(Reply::About(info)) => return Some(info),
                Ok(_) => {}            // MESSAGE / 其它噪声：继续等。
                Err(_) => return None, // 超时 / 断开。
            }
        }
    }
}

impl Drop for ExternalBot {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "END");
        let _ = self.stdin.flush();
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}
