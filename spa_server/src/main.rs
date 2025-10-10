//! SPA Server for Nilo WASM
//! 
//! A simple HTTP server that serves static files and falls back to index.html
//! for all routes (to support client-side routing in Single Page Applications).
//!
//! Usage:
//!   cd spa_server
//!   cargo run
//!   Or: cargo run --release (for production)

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::thread;
use std::env;

const DEFAULT_PORT: u16 = 8000;
const DEFAULT_ROOT_DIR: &str = "../pkg";

fn main() {
    // 環境変数またはコマンドライン引数からポートとディレクトリを取得
    let port = env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .or_else(|| env::args().nth(1).and_then(|p| p.parse().ok()))
        .unwrap_or(DEFAULT_PORT);
    
    let root_dir = env::var("ROOT_DIR")
        .ok()
        .or_else(|| env::args().nth(2))
        .unwrap_or_else(|| DEFAULT_ROOT_DIR.to_string());

    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr)
        .unwrap_or_else(|e| panic!("Failed to bind to {}: {}", addr, e));
    
    println!("🚀 Nilo SPA Server running at http://localhost:{}/", port);
    println!("📁 Serving from: {}/", root_dir);
    println!("🔄 All routes will fallback to index.html for client-side routing");
    println!("\n   Press Ctrl+C to stop the server\n");

    let root_dir = root_dir.clone();
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let root_dir = root_dir.clone();
                thread::spawn(move || {
                    handle_client(stream, &root_dir);
                });
            }
            Err(e) => {
                eprintln!("❌ Error accepting connection: {}", e);
            }
        }
    }
}

fn handle_client(mut stream: TcpStream, root_dir: &str) {
    let mut buffer = [0; 2048];
    match stream.read(&mut buffer) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("❌ Error reading from stream: {}", e);
            return;
        }
    }

    let request = String::from_utf8_lossy(&buffer);
    let request_line = request.lines().next().unwrap_or("");
    
    // Parse the request: "GET /path HTTP/1.1"
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        send_error_response(&mut stream, 400, "Bad Request");
        return;
    }

    let method = parts[0];
    let path = parts[1];

    if method != "GET" {
        send_error_response(&mut stream, 405, "Method Not Allowed");
        return;
    }

    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    println!("[{}] {} {}", timestamp, method, path);

    serve_file(&mut stream, path, root_dir);
}

fn serve_file(stream: &mut TcpStream, request_path: &str, root_dir: &str) {
    // Clean the path (remove query params, normalize)
    let clean_path = request_path.split('?').next().unwrap_or(request_path);
    let clean_path = if clean_path == "/" {
        "/index.html"
    } else {
        clean_path
    };

    // Security: prevent directory traversal attacks
    if clean_path.contains("..") {
        send_error_response(stream, 403, "Forbidden");
        return;
    }

    // Build file path
    let file_path = format!("{}{}", root_dir, clean_path);
    let path = Path::new(&file_path);

    // Try to read the file
    if let Ok(mut file) = fs::File::open(path) {
        let mut contents = Vec::new();
        if file.read_to_end(&mut contents).is_ok() {
            let mime_type = get_mime_type(path);
            send_response(stream, 200, "OK", mime_type, &contents);
            return;
        }
    }

    // If file doesn't exist and it's not a file request (no extension or specific extensions),
    // fall back to index.html for SPA routing
    if should_fallback_to_index(clean_path) {
        let index_path = format!("{}/index.html", root_dir);
        if let Ok(mut file) = fs::File::open(&index_path) {
            let mut contents = Vec::new();
            if file.read_to_end(&mut contents).is_ok() {
                println!("   ↳ Fallback to index.html for SPA route: {}", clean_path);
                send_response(stream, 200, "OK", "text/html; charset=utf-8", &contents);
                return;
            }
        }
    }

    // File not found
    send_error_response(stream, 404, "Not Found");
}

fn should_fallback_to_index(path: &str) -> bool {
    // Don't fallback for requests with file extensions (except .html)
    if let Some(extension) = Path::new(path).extension() {
        let ext = extension.to_string_lossy().to_lowercase();
        // Only fallback for .html or routes without recognized file extensions
        return ext == "html" || (!matches!(ext.as_str(), 
            "js" | "wasm" | "css" | "png" | "jpg" | "jpeg" | "svg" | "ico" | 
            "json" | "txt" | "xml" | "pdf" | "zip" | "woff" | "woff2" | "ttf" | "otf"
        ));
    }
    
    // No extension = likely a route, fallback to index.html
    true
}

fn get_mime_type(path: &Path) -> &'static str {
    match path.extension().and_then(|s| s.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("txt") => "text/plain; charset=utf-8",
        Some("xml") => "application/xml; charset=utf-8",
        Some("pdf") => "application/pdf",
        Some("zip") => "application/zip",
        _ => "application/octet-stream",
    }
}

fn send_response(stream: &mut TcpStream, status_code: u16, status_text: &str, content_type: &str, body: &[u8]) {
    let response = format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-cache\r\n\
         Access-Control-Allow-Origin: *\r\n\
         \r\n",
        status_code, status_text, content_type, body.len()
    );

    if let Err(e) = stream.write_all(response.as_bytes()) {
        eprintln!("❌ Error writing response headers: {}", e);
        return;
    }
    if let Err(e) = stream.write_all(body) {
        eprintln!("❌ Error writing response body: {}", e);
    }
    let _ = stream.flush();
}

fn send_error_response(stream: &mut TcpStream, status_code: u16, status_text: &str) {
    let body = format!(
        "<!DOCTYPE html>\
         <html>\
         <head><title>{} {}</title>\
         <style>body{{font-family:sans-serif;padding:40px;text-align:center}}\
         h1{{color:#d32f2f}}</style>\
         </head>\
         <body>\
         <h1>{} {}</h1>\
         <p>Nilo SPA Server</p>\
         </body>\
         </html>",
        status_code, status_text, status_code, status_text
    );
    send_response(stream, status_code, status_text, "text/html; charset=utf-8", body.as_bytes());
}
