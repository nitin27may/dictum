#![cfg(target_os = "windows")]

use anyhow::{anyhow, Result};

/// Injects text at the current cursor position on Windows.
/// Uses OpenClipboard → SetClipboardData → SendInput(Ctrl+V).
pub async fn inject_text(text: &str) -> Result<()> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL, VK_V,
    };

    let text_bytes = text.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let byte_len = text_bytes.len() * 2;

    unsafe {
        // 1. Open clipboard and set text
        OpenClipboard(None).map_err(|e| anyhow!("OpenClipboard failed: {}", e))?;
        EmptyClipboard().map_err(|e| {
            let _ = CloseClipboard();
            anyhow!("EmptyClipboard failed: {}", e)
        })?;

        let hmem = GlobalAlloc(GMEM_MOVEABLE, byte_len)
            .map_err(|e| anyhow!("GlobalAlloc failed: {}", e))?;

        let ptr = GlobalLock(hmem) as *mut u16;
        if ptr.is_null() {
            let _ = CloseClipboard();
            return Err(anyhow!("GlobalLock returned null"));
        }

        std::ptr::copy_nonoverlapping(text_bytes.as_ptr(), ptr, text_bytes.len());
        GlobalUnlock(hmem);

        // CF_UNICODETEXT = 13
        SetClipboardData(13, HANDLE(hmem.0)).map_err(|e| {
            let _ = CloseClipboard();
            anyhow!("SetClipboardData failed: {}", e)
        })?;

        CloseClipboard().map_err(|e| anyhow!("CloseClipboard failed: {}", e))?;

        // 2. Send Ctrl+V
        let inputs = [
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_CONTROL,
                        wScan: 0,
                        dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0),
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_V,
                        wScan: 0,
                        dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0),
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_V,
                        wScan: 0,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_CONTROL,
                        wScan: 0,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
        ];

        let sent = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        if sent != inputs.len() as u32 {
            return Err(anyhow!("SendInput: only {} of {} inputs sent", sent, inputs.len()));
        }
    }

    Ok(())
}

pub fn check_accessibility_permission() -> bool {
    // Windows doesn't have the same permission model; SendInput works without special permissions
    true
}
