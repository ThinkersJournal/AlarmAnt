// SPDX-License-Identifier: MIT OR Apache-2.0
//! `alarmant-pc`: the PC alarm (ruling 389, tier 3). A program in the operator's logged-in Windows
//! session, not a service: it asks the server whether an alert has reached the PC step, and while
//! one has, it unmutes the default speakers, sets their master volume to maximum (Core Audio) and
//! sounds a siren until the operator clicks Stop, which acknowledges the alert.
//!
//! Environment: `ALARMANT_PC_SERVER` (https base URL, or http on this machine) and
//! `ALARMANT_PC_KEY`; optionally `ALARMANT_PC_SERVER_CERT_FILE`, the server's own self-signed `tls-cert.pem`,
//! to trust it (a home LAN). The key is never an argument and never printed.
//!
//! Limits, stated plainly: it works only while the PC is on, the user is logged in, the program is
//! running and a playback device exists; it cannot wake a sleeping PC; headphones or a muted
//! external amplifier still make it inaudible; if a Stop dialog is dismissed after the alert ended
//! elsewhere nothing happens; the server is never told when the PC could not sound (constraint 18).

#[cfg(not(windows))]
fn main() {
    eprintln!("alarmant-pc runs on Windows only");
    std::process::exit(1);
}

#[cfg(windows)]
mod win {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use alarmant::hostwatch::{CHECK_SECS, HealthWatcher, Notice, terminal_notice};
    use alarmant::pcclient::{PcClient, Siren, Watcher};
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        IMMDeviceEnumerator, MMDeviceEnumerator, PlaySoundW, SND_ASYNC, SND_LOOP, SND_MEMORY,
        SND_PURGE, eConsole, eRender,
    };
    use windows::Win32::System::Com::{
        CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        MB_ICONWARNING, MB_OK, MB_SETFOREGROUND, MB_TOPMOST, MessageBoxW,
    };
    use windows::core::{PCWSTR, w};

    const POLL: Duration = Duration::from_secs(5);
    const RATE: u32 = 22_050;

    /// A two-tone siren as an in-memory 16-bit mono WAV (about 1.2 s, played in a loop).
    pub(super) fn siren_wav() -> Vec<u8> {
        let mut pcm: Vec<i16> = Vec::new();
        for (hz, ms) in [(650.0_f64, 300), (950.0, 300), (650.0, 300), (950.0, 300)] {
            let n = RATE as usize * ms / 1000;
            for i in 0..n {
                let t = i as f64 / f64::from(RATE);
                let s = (t * hz * std::f64::consts::TAU).sin();
                // A little clipped, so it is harsh and carries.
                pcm.push(((s * 1.6).clamp(-1.0, 1.0) * 30_000.0) as i16);
            }
        }
        let data_len = (pcm.len() * 2) as u32;
        let mut w = Vec::with_capacity(44 + pcm.len() * 2);
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&(36 + data_len).to_le_bytes());
        w.extend_from_slice(b"WAVEfmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&RATE.to_le_bytes());
        w.extend_from_slice(&(RATE * 2).to_le_bytes());
        w.extend_from_slice(&2u16.to_le_bytes());
        w.extend_from_slice(&16u16.to_le_bytes());
        w.extend_from_slice(b"data");
        w.extend_from_slice(&data_len.to_le_bytes());
        for s in pcm {
            w.extend_from_slice(&s.to_le_bytes());
        }
        w
    }

    struct WinSiren {
        wav: Vec<u8>,
    }

    impl WinSiren {
        /// Unmute and set the default playback device to full volume. Errors are local: a PC with
        /// no playback device simply cannot sound.
        fn max_volume() -> windows::core::Result<()> {
            // SAFETY: plain COM calls on interfaces we just created, on the thread that
            // initialised COM; the null event context is allowed.
            unsafe {
                let enumerator: IMMDeviceEnumerator =
                    CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
                let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
                let volume: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;
                volume.SetMute(false, std::ptr::null())?;
                volume.SetMasterVolumeLevelScalar(1.0, std::ptr::null())?;
            }
            Ok(())
        }
    }

    impl Siren for WinSiren {
        fn start(&mut self) {
            if let Err(e) = Self::max_volume() {
                eprintln!("alarmant-pc: could not set the volume ({e})");
            }
            // SAFETY: the buffer lives in `self` for as long as the sound can play; it is purged
            // in `stop` before anything frees it.
            unsafe {
                let _ = PlaySoundW(
                    PCWSTR(self.wav.as_ptr().cast()),
                    None,
                    SND_MEMORY | SND_LOOP | SND_ASYNC,
                );
            }
        }

        fn keep(&mut self) {
            let _ = Self::max_volume();
        }

        fn stop(&mut self) {
            // SAFETY: a null name with SND_PURGE stops the current sound.
            unsafe {
                let _ = PlaySoundW(PCWSTR::null(), None, SND_PURGE);
            }
        }
    }

    impl Drop for WinSiren {
        fn drop(&mut self) {
            self.stop();
        }
    }

    fn show_stop_dialog(clicked: Arc<AtomicBool>) {
        std::thread::spawn(move || {
            // SAFETY: a blocking message box with static text, no owner window.
            unsafe {
                MessageBoxW(
                    None,
                    w!(
                        "An AlarmAnt alert reached this PC. Click OK to stop the alarm and acknowledge it."
                    ),
                    w!("AlarmAnt alert"),
                    MB_OK | MB_ICONWARNING | MB_TOPMOST | MB_SETFOREGROUND,
                );
            }
            clicked.store(true, Ordering::SeqCst);
        });
    }

    /// Observer B's dialog: the server stopped answering; Stop only silences the siren (nothing is sent).
    fn show_server_down_dialog(clicked: Arc<AtomicBool>) {
        std::thread::spawn(move || {
            // SAFETY: a blocking message box with static text, no owner window.
            unsafe {
                MessageBoxW(
                    None,
                    w!(
                        "The AlarmAnt server has not answered 3 checks in a row, so phone alerts cannot be sent. Click OK to stop this alarm."
                    ),
                    w!("AlarmAnt server down"),
                    MB_OK | MB_ICONWARNING | MB_TOPMOST | MB_SETFOREGROUND,
                );
            }
            clicked.store(true, Ordering::SeqCst);
        });
    }

    pub async fn run() -> Result<(), String> {
        let base =
            std::env::var("ALARMANT_PC_SERVER").map_err(|_| "ALARMANT_PC_SERVER is not set")?;
        let key = std::env::var("ALARMANT_PC_KEY").map_err(|_| "ALARMANT_PC_KEY is not set")?;
        let client = match std::env::var("ALARMANT_PC_SERVER_CERT_FILE") {
            Ok(path) if !path.trim().is_empty() => {
                let pem = std::fs::read(path.trim())
                    .map_err(|_| "ALARMANT_PC_SERVER_CERT_FILE cannot be read")?;
                PcClient::trusting(&base, key.trim(), &pem)
            }
            _ => PcClient::new(&base, key.trim()),
        }
        .map_err(|e| e.to_string())?;
        // SAFETY: once, at start, on the thread that runs everything (current-thread runtime).
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        let mut watcher = Watcher::new(WinSiren { wav: siren_wav() });
        let clicked = Arc::new(AtomicBool::new(false));
        let mut ids: Vec<String> = Vec::new();
        // Observer B: a check of the server every CHECK_SECS, three failures sound this PC's own siren.
        let mut health = HealthWatcher::new(WinSiren { wav: siren_wav() });
        let health_clicked = Arc::new(AtomicBool::new(false));
        let mut next_check = std::time::Instant::now();
        eprintln!("alarmant-pc: watching");
        loop {
            if clicked.swap(false, Ordering::SeqCst) {
                watcher.operator_stop();
                for id in &ids {
                    // Retried by the next poll if it fails: the alert is still listed then.
                    let _ = client.acknowledge(id).await;
                }
            }
            if health_clicked.swap(false, Ordering::SeqCst) {
                health.operator_stop();
            }
            if std::time::Instant::now() >= next_check {
                next_check += Duration::from_secs(CHECK_SECS);
                if let Some(n) = health.observe(client.healthy().await) {
                    eprintln!("alarmant-pc: {}", terminal_notice(n));
                    if n == Notice::ServerDown {
                        show_server_down_dialog(Arc::clone(&health_clicked));
                    }
                }
            }
            let polled = client.active().await;
            if let Err(e) = &polled {
                eprintln!("alarmant-pc: {e}");
            }
            let was = watcher.is_sounding();
            if let Ok(list) = &polled {
                ids.clone_from(list);
            }
            watcher.observe(&polled);
            if watcher.is_sounding() && !was {
                show_stop_dialog(Arc::clone(&clicked));
            }
            tokio::time::sleep(POLL).await;
        }
    }
}

#[cfg(windows)]
#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(m) = win::run().await {
        eprintln!("alarmant-pc: {m}");
        std::process::exit(1);
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::win::siren_wav;

    #[test]
    fn the_siren_is_a_well_formed_wav() {
        let w = siren_wav();
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..16], b"WAVEfmt ");
        let riff_len = u32::from_le_bytes(w[4..8].try_into().unwrap()) as usize;
        assert_eq!(riff_len, w.len() - 8);
        let data_len = u32::from_le_bytes(w[40..44].try_into().unwrap()) as usize;
        assert_eq!(data_len, w.len() - 44);
        assert!(w[44..].chunks(2).any(|c| c != [0, 0]), "not silent");
    }
}
