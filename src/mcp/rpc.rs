//! The game's loopback control service.
//!
//! `fableflight-mcp` speaks MCP to the AI client and forwards tool calls here
//! as one JSON-RPC request per line.  The game drains the calls on its main
//! thread, where it can read and change the live simulation safely.

use super::tools;
use crossbeam_channel::{Receiver, Sender, bounded};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

/// Where the bridge looks unless `FABLEFLIGHT_MCP_PORT` or `--port` says otherwise.
pub const DEFAULT_PORT: u16 = 42761;

/// How long a tool may take before the bridge gives up.
const CALL_TIMEOUT: Duration = Duration::from_secs(30);

/// One tool call for the game thread.
pub struct Call {
    pub name: String,
    pub args: Value,
    /// The complete MCP `CallToolResult` object.
    pub reply: Sender<Value>,
}

/// Start listening on `127.0.0.1:port` and return the queue the game drains.
pub fn serve(port: u16) -> std::io::Result<Receiver<Call>> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    serve_listener(listener)
}

fn serve_listener(listener: TcpListener) -> std::io::Result<Receiver<Call>> {
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    let (tx, rx) = bounded(256);
    std::thread::Builder::new().name("mcp-rpc".into()).spawn(move || {
        log::info!("MCP copilot listening on 127.0.0.1:{port}");
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let tx = tx.clone();
                    let _ = std::thread::Builder::new().name("mcp-call".into()).spawn(move || handle(stream, tx));
                }
                Err(e) => log::warn!("MCP connection: {e}"),
            }
        }
    })?;
    Ok(rx)
}

fn handle(stream: TcpStream, tx: Sender<Call>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(60)));
    let Ok(reader_stream) = stream.try_clone() else { return };
    let mut reader = BufReader::new(reader_stream);
    let mut writer = stream;
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some(response) = response(line, &tx) else { continue };
        let mut text = match serde_json::to_string(&response) {
            Ok(text) => text,
            Err(e) => {
                log::warn!("MCP response: {e}");
                continue;
            }
        };
        text.push('\n');
        if writer.write_all(text.as_bytes()).is_err() || writer.flush().is_err() {
            break;
        }
    }
}

fn response(line: &str, tx: &Sender<Call>) -> Option<Value> {
    let request: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return Some(error(Value::Null, -32700, format!("parse error: {e}"))),
    };
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    if method.is_empty() || method.starts_with("notifications/") {
        return None;
    }
    let params = request.get("params").cloned().unwrap_or(Value::Null);
    match method {
        "ping" => Some(ok(id, json!({}))),
        "tools/list" => Some(ok(id, tools::catalog_json())),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("").to_string();
            if name.is_empty() {
                return Some(error(id, -32602, "tools/call needs a tool name".into()));
            }
            let mut args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            if args.is_null() {
                args = json!({});
            }
            let (reply, answer) = bounded(1);
            if tx.send_timeout(Call { name, args, reply }, Duration::from_secs(2)).is_err() {
                return Some(ok(id, tools::error_result("The simulator is busy; try again in a moment.")));
            }
            match answer.recv_timeout(CALL_TIMEOUT) {
                Ok(result) => Some(ok(id, result)),
                Err(_) => Some(error(id, -32603, "the simulator did not answer; is it still running?".into())),
            }
        }
        other => Some(error(id, -32601, format!("unknown method {other}"))),
    }
}

fn ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error(id: Value, code: i64, message: String) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bridge's side of the loopback protocol: a call reaches the game
    /// queue and the game's result reaches the socket.
    #[test]
    fn tool_call_round_trip() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let calls = serve_listener(listener).unwrap();
        let game = std::thread::spawn(move || {
            let call = calls.recv_timeout(Duration::from_secs(5)).unwrap();
            assert_eq!(call.name, "get_aircraft_state");
            assert_eq!(call.args, json!({ "limit": 1 }));
            call.reply.send(tools::json_result(json!({ "ok": true }))).unwrap();
        });
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        stream.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"tools/call\",\"params\":{\"name\":\"get_aircraft_state\",\"arguments\":{\"limit\":1}}}\n").unwrap();
        stream.flush().unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let answer: Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(answer["id"], 7);
        assert_eq!(answer["result"]["isError"], false);
        assert!(answer["result"]["content"][0]["text"].as_str().unwrap().contains("\"ok\""));
        game.join().unwrap();
    }
}
