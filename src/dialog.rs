//! A small native "enter the stream URL" prompt, one implementation per
//! platform: a GTK dialog on Linux (GTK is already linked for the tray), an
//! AppleScript dialog on macOS and a PowerShell input box on Windows — so no
//! extra GUI toolkit is pulled in.
//!
//! All implementations are asynchronous: they return immediately and invoke
//! `on_result` later with `Some(text)` when the user confirms or `None` on
//! cancel. The callback may run on any thread; callers forward the result into
//! the event loop via an [`tao::event_loop::EventLoopProxy`].

const TITLE: &str = "Whirr";

/// Why the prompt is being shown; selects the explanatory text above the
/// input field.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// A plain configuration request (menu item or first launch).
    Configure,
    /// The previous input was rejected as invalid.
    InvalidInput,
    /// Playback was requested while no stream URL is configured.
    PlayRequested,
}

fn prompt_text(reason: Reason) -> &'static str {
    match reason {
        Reason::Configure => "Enter the URL of the MP3 stream to play:",
        Reason::InvalidInput => {
            "That is not a valid http(s) URL.\nEnter the URL of the MP3 stream to play:"
        }
        Reason::PlayRequested => {
            "No stream URL is configured, so there is nothing to play yet.\n\
             Enter the URL of the MP3 stream to play:"
        }
    }
}

/// Show the stream-URL prompt, pre-filled with `current`. `reason` selects an
/// explanatory message (e.g. that the previous input was rejected).
#[cfg(all(unix, not(target_os = "macos")))]
pub fn prompt_stream_url<F>(current: Option<String>, reason: Reason, on_result: F)
where
    F: FnOnce(Option<String>) + Send + 'static,
{
    use gtk::prelude::*;

    let dialog = gtk::Dialog::with_buttons(
        Some(TITLE),
        None::<&gtk::Window>,
        gtk::DialogFlags::MODAL,
        &[
            ("Cancel", gtk::ResponseType::Cancel),
            ("Save", gtk::ResponseType::Ok),
        ],
    );
    dialog.set_default_response(gtk::ResponseType::Ok);
    dialog.set_keep_above(true);

    let label = gtk::Label::new(Some(prompt_text(reason)));
    let entry = gtk::Entry::new();
    entry.set_text(current.as_deref().unwrap_or(""));
    entry.set_width_chars(48);
    entry.set_activates_default(true);

    let content = dialog.content_area();
    content.set_border_width(12);
    content.set_spacing(8);
    content.add(&label);
    content.add(&entry);

    // `connect_response` needs `FnMut`; the one-shot callback lives in a Cell.
    let on_result = std::cell::Cell::new(Some(on_result));
    dialog.connect_response(move |dialog, response| {
        if let Some(cb) = on_result.take() {
            cb(if response == gtk::ResponseType::Ok {
                Some(entry.text().to_string())
            } else {
                None
            });
        }
        dialog.close();
    });
    dialog.show_all();
}

#[cfg(target_os = "macos")]
pub fn prompt_stream_url<F>(current: Option<String>, reason: Reason, on_result: F)
where
    F: FnOnce(Option<String>) + Send + 'static,
{
    let script = format!(
        "text returned of (display dialog {} default answer {} with title {} \
         buttons {{\"Cancel\", \"Save\"}} default button \"Save\")",
        applescript_str(prompt_text(reason)),
        applescript_str(current.as_deref().unwrap_or("")),
        applescript_str(TITLE),
    );
    // The dialog belongs to the osascript subprocess, so it can run (and
    // block) off the UI thread.
    std::thread::spawn(move || {
        let output = std::process::Command::new("osascript")
            .args(["-e", &script])
            .output();
        on_result(match output {
            // Non-success means the user cancelled (AppleScript error -128).
            Ok(out) if out.status.success() => Some(
                String::from_utf8_lossy(&out.stdout)
                    .trim_end_matches('\n')
                    .to_string(),
            ),
            Ok(_) => None,
            Err(err) => {
                log::error!("could not run osascript: {err}");
                None
            }
        });
    });
}

#[cfg(target_os = "macos")]
fn applescript_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(target_os = "windows")]
pub fn prompt_stream_url<F>(current: Option<String>, reason: Reason, on_result: F)
where
    F: FnOnce(Option<String>) + Send + 'static,
{
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let command = format!(
        "Add-Type -AssemblyName Microsoft.VisualBasic; \
         [Microsoft.VisualBasic.Interaction]::InputBox({}, {}, {})",
        powershell_str(prompt_text(reason)),
        powershell_str(TITLE),
        powershell_str(current.as_deref().unwrap_or("")),
    );
    std::thread::spawn(move || {
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &command])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        on_result(match output {
            Ok(out) if out.status.success() => {
                let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
                // InputBox returns an empty string on cancel; empty input is
                // treated as cancel either way.
                if text.is_empty() {
                    None
                } else {
                    Some(text)
                }
            }
            Ok(_) => None,
            Err(err) => {
                log::error!("could not run powershell: {err}");
                None
            }
        });
    });
}

#[cfg(target_os = "windows")]
fn powershell_str(s: &str) -> String {
    // Single-quoted PowerShell string: only embedded quotes need doubling;
    // real newlines are fine inside it.
    format!("'{}'", s.replace('\'', "''"))
}
