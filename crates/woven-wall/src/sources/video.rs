//! Video wallpaper source via ffmpeg subprocess.
//! Requires `ffmpeg` in PATH. Loops the video indefinitely.
//! Spawns a reader thread that pipes raw BGRA frames into a bounded channel;
//! the main thread drains the channel each tick for the latest frame.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use crossbeam_channel::{bounded, Receiver, TrySendError};
use super::{Source, expand_tilde};

pub struct VideoSource {
    path:      String,
    child:     Option<Child>,
    rx:        Option<Receiver<Vec<u8>>>,
    current:   Vec<u8>,
    last_size: (u32, u32),
}

impl VideoSource {
    pub fn new(path: &str) -> Self {
        Self {
            path:      path.to_string(),
            child:     None,
            rx:        None,
            current:   Vec::new(),
            last_size: (0, 0),
        }
    }

    fn spawn(&mut self, width: u32, height: u32) {
        // kill previous process if any
        if let Some(mut c) = self.child.take() { let _ = c.kill(); let _ = c.wait(); }
        drop(self.rx.take());

        let path       = expand_tilde(&self.path);
        let frame_size = (width * height * 4) as usize;

        let mut child = match Command::new("ffmpeg")
            .args([
                "-re",                          // read at native speed
                "-stream_loop", "-1",           // loop forever
                "-i", &path,
                "-f", "rawvideo",
                "-pix_fmt", "bgra",             // matches wl_shm Argb8888 byte order
                "-vf", &format!("scale={}:{}", width, height),
                "-an",                          // drop audio
                "pipe:1",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(c)  => c,
            Err(e) => {
                tracing::error!("wall: failed to spawn ffmpeg: {e} — is it installed?");
                self.last_size = (width, height);
                self.current   = vec![0u8; frame_size];
                return;
            }
        };

        let mut stdout = child.stdout.take().unwrap();
        let (tx, rx)   = bounded::<Vec<u8>>(2);

        std::thread::spawn(move || {
            let mut buf = vec![0u8; frame_size];
            loop {
                match stdout.read_exact(&mut buf) {
                    Ok(_) => match tx.try_send(buf.clone()) {
                        Err(TrySendError::Full(_))          => {}  // consumer is behind; drop frame
                        Err(TrySendError::Disconnected(_))  => break,
                        Ok(_)                               => {}
                    },
                    Err(_) => break, // pipe closed / EOF
                }
            }
        });

        self.child     = Some(child);
        self.rx        = Some(rx);
        self.last_size = (width, height);
        self.current   = vec![0u8; frame_size];
        tracing::info!("wall: ffmpeg spawned for {path} at {width}×{height}");
    }
}

impl Drop for VideoSource {
    fn drop(&mut self) {
        if let Some(mut c) = self.child.take() { let _ = c.kill(); let _ = c.wait(); }
    }
}

impl Source for VideoSource {
    fn frame(&mut self, width: u32, height: u32) -> Vec<u8> {
        if self.last_size != (width, height) {
            self.spawn(width, height);
        }
        // drain channel — keep only the latest frame
        if let Some(rx) = &self.rx {
            while let Ok(f) = rx.try_recv() {
                self.current = f;
            }
        }
        if self.current.is_empty() {
            vec![0u8; (width * height * 4) as usize]
        } else {
            self.current.clone()
        }
    }

    // ~60 fps poll; ffmpeg -re handles actual video pacing
    fn frame_delay_ms(&self) -> u64 { 16 }
}
