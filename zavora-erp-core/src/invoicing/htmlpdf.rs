//! HTML → PDF conversion.
//!
//! To guarantee the downloaded PDF and the emailed PDF look exactly like the
//! on-screen document, we render the **same HTML** to PDF with headless Chrome
//! (`--headless --print-to-pdf`). Chrome is the same engine the browser uses for
//! the on-screen preview and the print dialog, so the output matches.
//!
//! If no Chrome/Chromium binary is found, callers fall back to the lightweight
//! hand-built PDF (`invoicing::pdf`) so the feature still works headless.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Hard cap on a single Chrome conversion. If exceeded, the child is killed and
/// the caller falls back to the built-in PDF — so a stuck Chrome can never hang
/// the request.
const CONVERT_TIMEOUT: Duration = Duration::from_secs(15);

/// Locate a usable Chrome/Chromium executable, honouring `CHROME_PATH` first.
pub fn find_chrome() -> Option<String> {
    if let Ok(p) = std::env::var("CHROME_PATH") {
        if !p.is_empty() && std::path::Path::new(&p).exists() {
            return Some(p);
        }
    }
    let candidates = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/usr/bin/google-chrome",
        "/usr/bin/google-chrome-stable",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
        "/snap/bin/chromium",
    ];
    candidates
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .map(|p| p.to_string())
}

/// True once `path` holds a fully-written PDF (correct header and `%%EOF`
/// trailer), so we never read a file Chrome is still flushing.
fn pdf_ready(path: &std::path::Path) -> bool {
    match std::fs::read(path) {
        Ok(b) if b.len() > 1024 && b.starts_with(b"%PDF") => {
            let tail = &b[b.len().saturating_sub(1024)..];
            tail.windows(5).any(|w| w == b"%%EOF")
        }
        _ => false,
    }
}

/// Render an HTML string to PDF bytes using headless Chrome. Returns `None` if
/// Chrome isn't available or the conversion fails (caller should fall back).
pub fn html_to_pdf(html: &str) -> Option<Vec<u8>> {
    let chrome = find_chrome()?;

    // Unique temp paths in the OS temp dir.
    let dir = std::env::temp_dir();
    let stamp = format!(
        "zavora-inv-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let html_path = dir.join(format!("{stamp}.html"));
    let pdf_path = dir.join(format!("{stamp}.pdf"));

    // Write the HTML.
    if let Ok(mut f) = std::fs::File::create(&html_path) {
        if f.write_all(html.as_bytes()).is_err() {
            let _ = std::fs::remove_file(&html_path);
            return None;
        }
    } else {
        return None;
    }

    // A throwaway user-data-dir avoids clobbering a real Chrome profile and
    // lets headless run as a service user.
    let profile = dir.join(format!("{stamp}-profile"));

    // Spawn Chrome with flags that keep a *background* (non-GUI) process from
    // blocking — the big one is `--use-mock-keychain`, which avoids the macOS
    // keychain access prompt that otherwise hangs a headless print forever.
    let child = Command::new(&chrome)
        .args([
            "--headless=new",
            "--disable-gpu",
            "--no-sandbox",
            "--no-first-run",
            "--no-default-browser-check",
            "--no-pdf-header-footer",
            "--use-mock-keychain",
            "--password-store=basic",
            "--disable-background-networking",
            "--disable-component-update",
            "--disable-default-apps",
            "--disable-extensions",
            "--disable-sync",
            "--disable-dev-shm-usage",
            "--mute-audio",
        ])
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg(format!("--print-to-pdf={}", pdf_path.display()))
        .arg(format!("file://{}", html_path.display()))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    let result = match child {
        Ok(mut c) => {
            // Chrome usually prints in a couple of seconds. Some builds write the
            // PDF and then linger instead of exiting, so we don't require a clean
            // exit: as soon as the output file is a complete PDF we take it and
            // kill Chrome. A hard deadline guards against a true stall.
            let deadline = Instant::now() + CONVERT_TIMEOUT;
            loop {
                if pdf_ready(&pdf_path) {
                    let _ = c.kill();
                    let _ = c.wait();
                    break;
                }
                match c.try_wait() {
                    Ok(Some(_)) => break, // exited — check the file below
                    Ok(None) => {
                        if Instant::now() >= deadline {
                            tracing::warn!("Chrome PDF conversion timed out; falling back");
                            let _ = c.kill();
                            let _ = c.wait();
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    Err(e) => {
                        tracing::warn!("Error waiting on Chrome PDF conversion: {e}");
                        break;
                    }
                }
            }
            std::fs::read(&pdf_path).ok()
        }
        Err(e) => {
            tracing::warn!("Failed to invoke Chrome for PDF conversion: {e}");
            None
        }
    };

    // Cleanup (best-effort).
    let _ = std::fs::remove_file(&html_path);
    let _ = std::fs::remove_file(&pdf_path);
    let _ = std::fs::remove_dir_all(&profile);

    result.filter(|b| b.starts_with(b"%PDF"))
}
